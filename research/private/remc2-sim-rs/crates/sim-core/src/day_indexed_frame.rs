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
    classic_hd::{ClassicHdFrameWriter, ClassicHdRenderProfile, byte_hash},
    classic_hd_raster::{
        rasterize_classic_hd_terrain_composed, rasterize_classic_hd_terrain_with_occlusion,
    },
    classic_hd_terrain::{
        ClassicHdProjection, ClassicHdSkyBank, ClassicHdTerrainRender, ClassicHdTerrainResources,
        build_classic_hd_terrain, build_classic_hd_terrain_projected,
    },
    mission_content::REMC2_SPEC_COMMIT,
    native_world::{MapType, Mc2Simulation, NativeWorldInspection},
    presentation::{
        cave_terrain::{
            CaveTerrainPlan, CaveTerrainResources, build_cave_terrain, build_cave_terrain_projected,
        },
        environment_materials::{CACHE_SCHEMA, EnvironmentResources},
        source_world::{SourceWorldSpriteStats, SourceWorldSprites},
    },
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
    /// World materials before exit history and map/HUD composition. Clients
    /// add their source-selected sprites before applying indexed GTD2 history.
    pub scene_indices: Vec<u8>,
    pub scene_revision: u64,
    pub scene_blur_mode: u8,
    pub scene_use_exit_tables: bool,
    /// This native scene includes the source-ordered terrain, reflected
    /// sprites, ground shadows and primary sprites. Clients must not redraw
    /// those sprites after water or apply their indexed operations twice.
    pub source_sprites_composited: bool,
    pub(crate) source_sprite_stats: SourceWorldSpriteStats,
    pub(crate) source_sprite_pixel_mode: u8,
    // Only actual primary DrawSprite completions enter this private carrier.
    // Bind immutable NewEvent identities before the renderer lends state back.
    primary_visibility: Vec<crate::campaign::source_arena::SourceEntityHandle>,
    visibility_package: String,
    visibility_camera: sim_session::CameraPose,
    visibility_sources: Vec<crate::native_world::EntitySnapshot>,
    pub palette_rgb8: Vec<u8>,
    /// Last source cell to paint each Cave terrain pixel; zero is unpainted.
    pub cave_terrain_orders: Option<Vec<u16>>,
    /// Source cell rank indexed by `(tile_y << 8) | tile_x`.
    pub cave_cell_orders: Option<Vec<u16>>,
    pub index_hash: u64,
    pub terrain_input_hash: u64,
    pub terrain_command_hash: u64,
    pub cache_content_sha256: String,
    pub package_sha256: String,
}

impl DayIndexedFrame {
    /// Apply source DrawSprite visibility feedback to the same authority that
    /// produced this frame. Pixel copying and cached reads invent no receipts.
    pub fn apply_primary_visibility(
        &self,
        session: &mut Mc2Simulation,
        generation: u64,
    ) -> Result<Vec<(u16, u32)>, DayIndexedFrameError> {
        if self.generation != generation || self.tick != session.state.tick {
            return Err(DayIndexedFrameError::new("primary sprite feedback belongs to a stale frame"));
        }
        if self.visibility_package != session.package_sha256()
            || self.visibility_camera != session.source_camera_pose()
        {
            return Err(DayIndexedFrameError::new("primary sprite feedback belongs to a different source view"));
        }
        for prior in &self.visibility_sources {
            let current = &session.entities()[usize::from(prior.index)];
            let mut acknowledged = prior.clone();
            acknowledged.set_subspell_index(prior.subspell_index | 0x40);
            if current != prior && current != &acknowledged {
                return Err(DayIndexedFrameError::new("primary sprite source changed after rendering"));
            }
        }
        session.apply_primary_sprite_visibility(self.tick, &self.primary_visibility)
            .map_err(DayIndexedFrameError::new)
    }

}

/// Original runtime graphics switches. These change presentation, never
/// authoritative gameplay or the selected mission's terrain/asset bindings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldRenderOptions {
    pub reflections: bool,
    pub sky: bool,
    pub shadows: bool,
}

impl WorldRenderOptions {
    #[must_use]
    pub const fn source_defaults() -> Self {
        Self {
            reflections: true,
            sky: true,
            shadows: true,
        }
    }
}

impl Default for WorldRenderOptions {
    fn default() -> Self {
        Self::source_defaults()
    }
}

/// Transactional generation/tick carrier for the frame producer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayIndexedFrameState {
    generation: u64,
    current_tick: Option<u64>,
    current_frame: Option<DayIndexedFrame>,
    current_view: Option<FrameView>,
    map_world: Option<MapWorldFrame>,
    last_sprite_pixel_mode: u8,
    render_options: WorldRenderOptions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MapWorldFrame {
    tick: u64,
    view: FrameView,
    cache_content_sha256: String,
    indices: Vec<u8>,
    palette_rgb8: Vec<u8>,
}

/// Exit action12 advances its camera and sequence without advancing the
/// flight tick. Palette-only paused updates do not advance image history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameView {
    camera: Level1TerrainFrameCamera,
    exit_phase: u8,
    exit_timer: i16,
    exit_blur: u8,
    exit_blank: bool,
    render_options: WorldRenderOptions,
}

impl FrameView {
    fn current(
        session: &Mc2Simulation,
        render_options: WorldRenderOptions,
    ) -> Result<Self, DayIndexedFrameError> {
        let exit = session.level_exit_sequence();
        Ok(Self {
            camera: camera_from_session(session)?,
            exit_phase: exit.phase,
            exit_timer: exit.timer,
            exit_blur: exit.blur_mode,
            exit_blank: exit.display_blank,
            render_options,
        })
    }
}

impl DayIndexedFrameState {
    #[must_use]
    pub const fn new(generation: u64) -> Self {
        Self {
            generation,
            current_tick: None,
            current_frame: None,
            current_view: None,
            map_world: None,
            last_sprite_pixel_mode: 0,
            render_options: WorldRenderOptions::source_defaults(),
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

    pub fn set_render_options(&mut self, options: WorldRenderOptions) {
        self.render_options = options;
    }

    /// Palette from the same committed narrow map view. Paused palette
    /// updates refresh it without re-rendering or advancing indexed history.
    #[must_use]
    pub fn map_world_palette(&self) -> Option<&[u8]> {
        self.map_world
            .as_ref()
            .map(|frame| frame.palette_rgb8.as_slice())
    }

    pub fn is_current_view(&self, session: &Mc2Simulation) -> Result<bool, DayIndexedFrameError> {
        Ok(self.current_frame.is_some()
            && self.current_tick == Some(session.state.tick)
            && self.current_view == Some(FrameView::current(session, self.render_options)?))
    }

    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.current_tick = None;
        self.current_frame = None;
        self.current_view = None;
        self.map_world = None;
        self.last_sprite_pixel_mode = 0;
    }

    /// Render the next frame without mutating this carrier until the complete
    /// qualified terrain pass succeeds.
    pub fn render(
        &mut self,
        inspection: &NativeWorldInspection,
        session: &Mc2Simulation,
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
        let view = FrameView::current(session, self.render_options)?;
        let frame = render_environment_indexed_frame(
            inspection,
            session,
            assets,
            generation,
            self.current_frame.as_ref(),
            false,
            self.last_sprite_pixel_mode,
            self.render_options,
        )?;
        self.last_sprite_pixel_mode = frame.source_sprite_pixel_mode;
        self.current_tick = Some(tick);
        self.current_view = Some(view);
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
        session: &Mc2Simulation,
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
        let view = FrameView::current(session, self.render_options)?;
        if self.current_tick.is_some_and(|current| tick < current) {
            return Err(DayIndexedFrameError::new(format!(
                "Day indexed frame tick {tick} predates the committed presentation revision"
            )));
        }
        if self.is_current_view(session)? {
            if let Some(frame) = self.current_frame.as_ref() {
                if frame.cache_content_sha256 != assets.cache_content_sha256() {
                    return Err(DayIndexedFrameError::new(
                        "environment cache changed within a live frame generation",
                    ));
                }
                // Paused outer frames advance PaletteChanges without a
                // gameplay tick. Reuse the indexed image/history, but read
                // the current palette from authority on every observation.
                let mut frame = frame.clone();
                frame.palette_rgb8 = assets.palette_rgb8().to_vec();
                session.field_palette().apply_rgb8(&mut frame.palette_rgb8);
                self.current_frame = Some(frame.clone());
                return Ok(frame);
            }
        }
        let frame = render_environment_indexed_frame(
            inspection,
            session,
            assets,
            generation,
            self.current_frame.as_ref(),
            false,
            self.last_sprite_pixel_mode,
            self.render_options,
        )?;
        self.last_sprite_pixel_mode = frame.source_sprite_pixel_mode;
        self.current_tick = Some(tick);
        self.current_view = Some(view);
        self.current_frame = Some(frame.clone());
        Ok(frame)
    }

    /// Actual narrow second DrawWorld viewport used by the full map. The
    /// source camera already contains its +128 eye-height bias. Repeated UI
    /// reads reuse one committed view and cannot advance sticky pixel mode.
    pub fn render_map_world(
        &mut self,
        inspection: &NativeWorldInspection,
        session: &Mc2Simulation,
        assets: &DayIndexedFrameAssets,
        generation: u64,
    ) -> Result<&[u8], DayIndexedFrameError> {
        if generation != self.generation {
            return Err(DayIndexedFrameError::new(
                "map world generation differs from its frame owner",
            ));
        }
        let view = FrameView::current(session, self.render_options)?;
        let tick = session.state.tick;
        if self
            .map_world
            .as_ref()
            .is_some_and(|frame| frame.tick == tick && frame.view == view)
        {
            let frame = self.map_world.as_mut().expect("map view checked");
            if frame.cache_content_sha256 != assets.cache_content_sha256() {
                return Err(DayIndexedFrameError::new(
                    "map world cache changed within a live generation",
                ));
            }
            frame.palette_rgb8 = assets.palette_rgb8().to_vec();
            session.field_palette().apply_rgb8(&mut frame.palette_rgb8);
            return Ok(&frame.indices);
        }
        let frame = render_environment_indexed_frame(
            inspection,
            session,
            assets,
            generation,
            None,
            true,
            self.last_sprite_pixel_mode,
            self.render_options,
        )?;
        let mut indices = Vec::with_capacity(128 * 200);
        for row in frame.scene_indices.chunks_exact(320) {
            indices.extend_from_slice(&row[192..320]);
        }
        // DrawWorld's LABEL_33 blends only its active viewport. The left
        // full-map terrain and its markers are outside that viewport. Retain
        // the previous narrow image, including the result of its last blend,
        // rather than advancing the ordinary world's independent history.
        let ordinary_prior;
        let prior = if let Some(prior) = self.map_world.as_ref() {
            Some(prior.indices.as_slice())
        } else if let Some(prior) = self.current_frame.as_ref() {
            ordinary_prior = prior
                .indices
                .chunks_exact(320)
                .flat_map(|row| row[192..320].iter().copied())
                .collect::<Vec<_>>();
            Some(ordinary_prior.as_slice())
        } else {
            None
        };
        if let Some(prior) = prior {
            assets
                .materials
                .composite_exit(&mut indices, prior, session.level_exit_sequence())
                .map_err(|error| DayIndexedFrameError::new(error.to_string()))?;
        }
        self.last_sprite_pixel_mode = frame.source_sprite_pixel_mode;
        self.map_world = Some(MapWorldFrame {
            tick,
            view,
            cache_content_sha256: assets.cache_content_sha256().to_owned(),
            indices,
            palette_rgb8: frame.palette_rgb8,
        });
        Ok(&self.map_world.as_ref().expect("map view committed").indices)
    }
}

/// Render one ordinary Level 2 frame from the public package/session route.
pub fn render_level2_day_indexed_frame(
    inspection: &NativeWorldInspection,
    session: &Mc2Simulation,
    assets: &DayIndexedFrameAssets,
    generation: u64,
) -> Result<DayIndexedFrame, DayIndexedFrameError> {
    render_environment_indexed_frame(
        inspection,
        session,
        assets,
        generation,
        None,
        false,
        0,
        WorldRenderOptions::source_defaults(),
    )
}

fn render_environment_indexed_frame(
    inspection: &NativeWorldInspection,
    session: &Mc2Simulation,
    assets: &DayIndexedFrameAssets,
    generation: u64,
    prior: Option<&DayIndexedFrame>,
    map_viewport: bool,
    previous_pixel_mode: u8,
    render_options: WorldRenderOptions,
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
    let mut camera = camera_from_session(session)?;
    camera.reflections = render_options.reflections;
    let profile = ClassicHdRenderProfile::compatibility_320x200();
    let sprite_projection = if map_viewport {
        crate::terrain_frame::source_map_projection(camera)
    } else {
        crate::terrain_frame::level1_projection(camera)
    };
    let mut terrain_projection = ClassicHdProjection::from_profile(&profile, camera);
    if map_viewport {
        terrain_projection.scale = sprite_projection.scale;
        terrain_projection.pitch_offset = sprite_projection.pitch_offset;
        terrain_projection.center_x = sprite_projection.center_x;
        terrain_projection.center_y = sprite_projection.center_y;
        terrain_projection.viewport_x = 192;
        terrain_projection.viewport_width = 128;
    }
    let planes = session.canonical_terrain().planes();
    validate_terrain_planes(
        planes.terrain_types.as_ref(),
        planes.ground_heights.as_ref(),
        planes.shading.as_ref(),
        planes.angles.as_ref(),
    )?;
    enum Terrain {
        Outdoor(ClassicHdTerrainRender),
        Cave(CaveTerrainPlan),
    }
    let terrain = if inspection.metadata().map_type == MapType::Cave {
        let clear_index = assets
            .materials
            .profile
            .as_ref()
            .ok_or_else(|| DayIndexedFrameError::new("Cave requires package-selected materials"))?
            .key_color_1;
        Terrain::Cave(
            build_cave_terrain_projected(
                &profile,
                camera,
                CaveTerrainResources {
                    terrain_types: &planes.terrain_types,
                    ground_heights: &planes.ground_heights,
                    ceiling_heights: &planes.secondary_heights,
                    shading: &planes.shading,
                    angles: &planes.angles,
                    clear_index,
                },
                terrain_projection,
            )
            .map_err(|error| {
                DayIndexedFrameError::new(format!("Cave terrain build failed: {error}"))
            })?,
        )
    } else {
        Terrain::Outdoor(
            if map_viewport || !render_options.sky {
                build_classic_hd_terrain_projected(
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
                    terrain_projection,
                    if render_options.sky {
                        crate::terrain_frame::draw_source_map_sky(camera, assets.sky())
                            .map_err(|error| DayIndexedFrameError::new(error.to_string()))?
                    } else {
                        let clear_index = assets
                            .materials
                            .profile
                            .as_ref()
                            .ok_or_else(|| {
                                DayIndexedFrameError::new(
                                    "Sky option requires source environment key colour",
                                )
                            })?
                            .key_color_1;
                        vec![clear_index; profile.extent()]
                    },
                )
            } else {
                build_classic_hd_terrain(
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
            }
            .map_err(|error| {
                DayIndexedFrameError::new(format!("Day terrain build failed: {error}"))
            })?,
        )
    };
    let extent = profile.extent();
    let (mut indices, terrain_input_hash, terrain_command_hash) = match &terrain {
        Terrain::Outdoor(plan) => (plan.sky.clone(), plan.input_hash, plan.command_hash),
        Terrain::Cave(plan) => (
            plan.initial_indices(&profile),
            plan.input_hash,
            plan.command_hash,
        ),
    };
    if indices.len() != extent {
        return Err(DayIndexedFrameError::new(format!(
            "Day sky extent {} does not match profile extent {extent}",
            indices.len()
        )));
    }
    let mut provenance = vec![0_u8; extent];
    // Retain the exact terrain write ranks on every map. The legacy field
    // names predate outdoor sprite occlusion; their ABI extents are unchanged.
    let mut cave_terrain_orders = Some(vec![0_u16; extent]);
    let (outdoor_owner_orders, outdoor_cell_orders) = match &terrain {
        Terrain::Outdoor(plan) => plan.source_painter_orders(camera.yaw),
        Terrain::Cave(_) => (Vec::new(), Vec::new()),
    };
    let cave_cell_orders = match &terrain {
        Terrain::Cave(plan) => {
            let orders = plan.cell_orders();
            if orders.len() != TERRAIN_PLANE_BYTES {
                return Err(DayIndexedFrameError::new(
                    "Cave cell order plane has an invalid extent",
                ));
            }
            Some(orders.to_vec())
        }
        Terrain::Outdoor(_) => Some(outdoor_cell_orders),
    };
    let mut source_sprites = assets
        .materials
        .source_tmaps()
        .map(|bank| {
            SourceWorldSprites::new(
                camera,
                sprite_projection,
                inspection.metadata().map_type,
                session
                    .campaign_entity_presentations(camera.yaw)
                    .map_err(|error| DayIndexedFrameError::new(error.to_string()))?,
                &session.session.definition.presentation.particles,
                bank,
                cave_cell_orders.as_deref().ok_or_else(|| {
                    DayIndexedFrameError::new("source sprite cell orders missing")
                })?,
                &planes.ground_heights,
                previous_pixel_mode,
                render_options.shadows,
            )
            .map_err(|error| DayIndexedFrameError::new(error.to_string()))
        })
        .transpose()?;
    let identity = {
        let mut writer = ClassicHdFrameWriter::from_planes(
            &profile,
            &mut indices,
            &mut provenance,
            generation,
            tick,
            terrain_input_hash,
            0,
        )
        .map_err(|error| {
            DayIndexedFrameError::new(format!("Day frame allocation failed: {error}"))
        })?;
        match &terrain {
            Terrain::Outdoor(plan) if source_sprites.is_some() => {
                let sprites = source_sprites
                    .as_mut()
                    .expect("source sprite owner checked");
                let reflections = sprites.reflections_enabled();
                rasterize_classic_hd_terrain_composed(
                    &mut writer,
                    plan,
                    assets.terrain_atlas(),
                    tables,
                    DAY_TERRAIN_PROVENANCE,
                    &outdoor_owner_orders,
                    cave_terrain_orders.as_deref_mut().ok_or_else(|| {
                        DayIndexedFrameError::new("Outdoor terrain order plane absent")
                    })?,
                    reflections,
                    &mut |pass, owner, writer| {
                        let texture =
                            plan.projected[owner].map_or(0, |vertex| vertex.texture_index);
                        let width = writer.width() as usize;
                        let height = writer.height() as usize;
                        sprites.draw_cell(
                            pass,
                            outdoor_owner_orders[owner],
                            texture,
                            width,
                            height,
                            writer.indices_mut(),
                            tables,
                        )
                    },
                )
                .map(|_| ())
            }
            Terrain::Outdoor(plan) => rasterize_classic_hd_terrain_with_occlusion(
                &mut writer,
                plan,
                assets.terrain_atlas(),
                tables,
                DAY_TERRAIN_PROVENANCE,
                &outdoor_owner_orders,
                cave_terrain_orders.as_deref_mut().ok_or_else(|| {
                    DayIndexedFrameError::new("Outdoor terrain order plane is absent")
                })?,
            )
            .map(|_| ()),
            Terrain::Cave(plan) => {
                let orders = cave_terrain_orders.as_deref_mut().ok_or_else(|| {
                    DayIndexedFrameError::new("Cave terrain order plane is absent")
                })?;
                if let Some(sprites) = source_sprites.as_mut() {
                    plan.rasterize_composed(
                        &mut writer,
                        assets.terrain_atlas(),
                        tables,
                        DAY_TERRAIN_PROVENANCE,
                        orders,
                        &mut |rank, texture, writer| {
                            let width = writer.width() as usize;
                            let height = writer.height() as usize;
                            sprites.draw_cell(
                                crate::terrain_reflection::TerrainPass::Primary,
                                rank,
                                texture,
                                width,
                                height,
                                writer.indices_mut(),
                                tables,
                            )
                        },
                    )
                    .map(|_| ())
                } else {
                    plan.rasterize_with_occlusion(
                        &mut writer,
                        assets.terrain_atlas(),
                        tables,
                        DAY_TERRAIN_PROVENANCE,
                        orders,
                    )
                    .map(|_| ())
                }
            }
        }
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
    let scene_indices = indices.clone();
    let scene_revision = prior
        .map_or(Some(1), |frame| frame.scene_revision.checked_add(1))
        .ok_or_else(|| DayIndexedFrameError::new("composed scene revision overflow"))?;
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
            &planes.angles,
            inspection.metadata().map_type == MapType::Cave,
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
        scene_indices,
        scene_revision,
        scene_blur_mode: exit.blur_mode,
        scene_use_exit_tables: exit.end_sequence
            && exit.phase >= 6
            && assets.materials.day_exit_tables().is_some(),
        source_sprites_composited: source_sprites.is_some(),
        source_sprite_stats: source_sprites
            .as_ref()
            .map_or(SourceWorldSpriteStats::default(), SourceWorldSprites::stats),
        source_sprite_pixel_mode: source_sprites
            .as_ref()
            .map_or(0, SourceWorldSprites::last_pixel_mode),
        primary_visibility: source_sprites.as_ref().map_or_else(Vec::new, |sprites| {
            sprites.primary_visibility_completions().iter()
                .filter_map(|&slot| session.source_arena().handle(slot)).collect()
        }),
        visibility_package: session.package_sha256().to_owned(),
        visibility_camera: session.source_camera_pose(),
        visibility_sources: source_sprites.as_ref().map_or_else(Vec::new, |sprites| {
            sprites.primary_visibility_completions().iter()
                .map(|&slot| session.entities()[usize::from(slot)].clone()).collect()
        }),
        palette_rgb8,
        cave_terrain_orders,
        cave_cell_orders,
        index_hash,
        terrain_input_hash,
        terrain_command_hash,
        cache_content_sha256: assets.cache_content_sha256.clone(),
        package_sha256: assets.package_sha256.clone(),
    })
}

/// Source `GameUI.cpp::sub_63670_draw_minimap_a`, selected opaque branch.
/// The 128-source-pixel map is sampled onto the fixed 320x200 presentation.
/// All world coordinates wrap through BYTE1 exactly as the source map index.
fn draw_minimap_terrain(
    target: &mut [u8],
    stride: usize,
    camera: Level1TerrainFrameCamera,
    terrain_types: &[u8],
    shading: &[u8],
    angles: &[u8],
    cave: bool,
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
            target[y * stride + x] = if cave && angles[tile] & 8 != 0 {
                0
            } else {
                let base = usize::from(tables[0x14000 + usize::from(terrain_types[tile])]);
                tables[(usize::from(shading[tile]) << 8) + base]
            };
        }
    }
}

fn validate_day_route(
    inspection: &NativeWorldInspection,
    session: &Mc2Simulation,
    assets: &DayIndexedFrameAssets,
) -> Result<(), DayIndexedFrameError> {
    let metadata = inspection.metadata();
    let material = assets.profile();
    if metadata.external_level != material.external_level
        || metadata.internal_level != material.internal_level
        || metadata.map_type as u8 != material.map_type_id
        || !matches!(
            metadata.map_type,
            MapType::Day | MapType::Night | MapType::Cave
        )
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
    if session.profile().cave_mode != (metadata.map_type == MapType::Cave) {
        return Err(DayIndexedFrameError::new(
            "environment frame and flight Cave profiles differ",
        ));
    }
    if (metadata.map_type == MapType::Cave) != assets.materials.sky().is_none() {
        return Err(DayIndexedFrameError::new(
            "environment sky presence differs from the actual map type",
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

fn camera_from_session(
    session: &Mc2Simulation,
) -> Result<Level1TerrainFrameCamera, DayIndexedFrameError> {
    let mut camera = camera_from_view(
        session.source_camera_pose(),
        session.shared_spells().local_player().turn,
    )?;
    let exit = session.level_exit_sequence();
    if exit.active || exit.complete {
        camera.fov = exit.camera_fov;
    }
    Ok(camera)
}

fn camera_from_view(
    view: sim_session::CameraPose,
    source_turn: i32,
) -> Result<Level1TerrainFrameCamera, DayIndexedFrameError> {
    Ok(Level1TerrainFrameCamera {
        render_x: view.x,
        render_y: view.y,
        eye_z: i32::from(view.z),
        yaw: view.yaw,
        pitch: view.pitch,
        roll: view.roll,
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
    use crate::FlightWorld;
    use crate::native_world::{NativeWorld, OwnerPackageProfile};
    use std::{
        fs,
        path::PathBuf,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn primary_visibility_feedback_binds_actual_frame_and_current_source_identity() {
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map(PathBuf::from)
            .unwrap_or_else(|| generated_path("mc-bhq10-owner-v3-level25-20261029"));
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map(PathBuf::from)
            .unwrap_or_else(|| generated_path("dragon-bee-20261105-integration/current-level25-environment"));
        let assets = DayIndexedFrameAssets::load(&cache).unwrap();
        let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            package, cache, crate::campaign::spec::CampaignSelection::ordinary(25).unwrap(),
        ).unwrap();
        campaign.step(crate::PlayerCommand::default(), 0);
        // Declared encounter progression and player positioning only. The
        // original boss constructor, clocks and renderer produce the receipt.
        campaign.simulation_mut().activate_authored_group_prerequisite_for_test(29).unwrap();
        let boss = campaign.simulation().entities().iter()
            .find(|row| row.active && (row.class, row.model) == (5, 10)).unwrap().index;
        let player = campaign.simulation().state.entity_index;
        let mut position = campaign.simulation().entities()[usize::from(boss)].position;
        crate::move_entity(&mut position, campaign.simulation().state.yaw.wrapping_add(1024) & 2047, 0, 2048);
        position.z = campaign.simulation().canonical_terrain().ground_height(position).wrapping_add(768);
        campaign.simulation_mut().set_level2_entity_position(player, position).unwrap();
        for _ in 0..256 {
            campaign.step(crate::PlayerCommand::default(), 0);
            let generation = campaign.generation();
            let frame = render_level2_day_indexed_frame(campaign.inspection(), campaign.simulation(), &assets, generation).unwrap();
            if frame.primary_visibility.is_empty() { continue; }
            let before = campaign.state_hash();
            let mut wrong = frame.clone();
            wrong.visibility_package.push('!');
            assert!(wrong.apply_primary_visibility(campaign.simulation_mut(), generation).is_err());
            wrong = frame.clone();
            wrong.visibility_camera.yaw ^= 1;
            assert!(wrong.apply_primary_visibility(campaign.simulation_mut(), generation).is_err());
            wrong = frame.clone();
            wrong.primary_visibility[0].generation = wrong.primary_visibility[0].generation.wrapping_add(1);
            assert!(wrong.apply_primary_visibility(campaign.simulation_mut(), generation).is_err());
            assert_eq!(campaign.state_hash(), before, "all invalid receipts reject before any write");
            let mut changed_source = campaign.clone();
            let slot = frame.primary_visibility[0].source_slot;
            let row = &mut changed_source.simulation_mut().source_arena_mut().entities[usize::from(slot)];
            row.set_life(row.life - 1);
            let changed_hash = changed_source.state_hash();
            assert!(frame.apply_primary_visibility(changed_source.simulation_mut(), generation).is_err());
            assert_eq!(changed_source.state_hash(), changed_hash);
            let changed = frame.apply_primary_visibility(campaign.simulation_mut(), generation).unwrap();
            assert!(changed.iter().any(|&(slot, _)| slot == boss));
            assert!(frame.apply_primary_visibility(campaign.simulation_mut(), generation).unwrap().is_empty(),
                "repeating an acknowledged carrier is idempotent");
            assert_eq!(campaign.simulation().entities()[usize::from(boss)].subspell_index & 0x40, 0x40);
            return;
        }
        panic!("actual boss primary rendering produced no visibility completion");
    }

    #[test]
    fn original_render_options_control_live_world_passes_and_invalidate_both_cached_views() {
        let package = generated_path("level3-day-block32-campaign-recipe-v2");
        let cache = generated_path("mc-level3-environment-exit-20261025");
        let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            package,
            &cache,
            crate::campaign::spec::CampaignSelection::LEVEL3,
        )
        .unwrap();
        let assets = DayIndexedFrameAssets::load(&cache).unwrap();
        // Captured actual Level3 water viewpoint toward live source actor201,
        // retained by the opening source-pass check. No entity/map mutation.
        campaign
            .simulation_mut()
            .session
            .state
            .scratch
            .source_camera_pose = Some(sim_session::CameraPose {
            x: 42645,
            y: 4971,
            z: 128,
            yaw: 1536,
            pitch: 11,
            roll: 0,
        });
        let before = campaign.state_hash();
        let mut carrier = DayIndexedFrameState::new(campaign.generation());
        let render = |carrier: &mut DayIndexedFrameState| {
            carrier
                .render_current(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap()
        };
        let enabled = render(&mut carrier);
        assert!(enabled.source_sprite_stats.primary_writes > 0);
        assert!(enabled.source_sprite_stats.reflection_writes > 0);
        assert!(enabled.source_sprite_stats.shadow_writes > 0);
        let original_map = carrier
            .render_map_world(
                campaign.inspection(),
                campaign.simulation(),
                &assets,
                campaign.generation(),
            )
            .unwrap()
            .to_vec();
        carrier.set_render_options(WorldRenderOptions {
            reflections: false,
            ..WorldRenderOptions::default()
        });
        assert!(!carrier.is_current_view(campaign.simulation()).unwrap());
        let no_reflections = render(&mut carrier);
        assert_eq!(no_reflections.tick, enabled.tick);
        assert!(no_reflections.scene_revision > enabled.scene_revision);
        assert_eq!(no_reflections.source_sprite_stats.reflection_calls, 0);
        assert!(no_reflections.source_sprite_stats.primary_writes > 0);
        assert!(no_reflections.source_sprite_stats.shadow_writes > 0);
        assert_ne!(
            no_reflections.terrain_command_hash, enabled.terrain_command_hash,
            "source reflection-off draws opaque mode5 water instead of mode26 against an empty reflection pass"
        );
        carrier.set_render_options(WorldRenderOptions {
            shadows: false,
            ..WorldRenderOptions::default()
        });
        let no_shadows = render(&mut carrier);
        assert_eq!(no_shadows.source_sprite_stats.shadow_calls, 0);
        assert!(no_shadows.source_sprite_stats.primary_writes > 0);
        assert!(no_shadows.source_sprite_stats.reflection_writes > 0);
        // The low water view proves pass admission, but its shadow samples
        // may be palette identities or covered by later source draws. Require
        // a final pixel difference at an actual ground-facing actor view too.
        let mut shadow_campaign = campaign.clone();
        let rows = shadow_campaign
            .simulation()
            .campaign_entity_presentations(0)
            .unwrap();
        let eligible: Vec<_> = rows
            .into_iter()
            .filter(|row| {
                row.draw_order != u32::MAX
                    && row.logical_height >= 100
                    && (row.control_flags >> 16) & 0x0808 == 0
                    && shadow_campaign
                        .simulation()
                        .session
                        .definition
                        .presentation
                        .particles[usize::from(row.render_parameter_index)]
                    .byte_10
                        == 0
            })
            .take(8)
            .collect();
        let mut visible_shadow = None;
        'actors: for row in eligible {
            let ground = crate::legacy_terrain::interpolate_ground(
                &shadow_campaign
                    .simulation()
                    .canonical_terrain()
                    .planes()
                    .ground_heights,
                row.position.x,
                row.position.y,
            );
            for distance in [1024_u16, 4096] {
                let pose = sim_session::CameraPose {
                    x: row.position.x,
                    y: row.position.y.wrapping_add(distance),
                    z: (ground + 512) as i16,
                    yaw: 0,
                    pitch: 64,
                    roll: 0,
                };
                shadow_campaign
                    .simulation_mut()
                    .session
                    .state
                    .scratch
                    .source_camera_pose = Some(pose);
                let unchanged = shadow_campaign.state_hash();
                let mut view = DayIndexedFrameState::new(shadow_campaign.generation());
                let on = view
                    .render_current(
                        shadow_campaign.inspection(),
                        shadow_campaign.simulation(),
                        &assets,
                        shadow_campaign.generation(),
                    )
                    .unwrap();
                view.set_render_options(WorldRenderOptions {
                    shadows: false,
                    ..WorldRenderOptions::default()
                });
                let off = view
                    .render_current(
                        shadow_campaign.inspection(),
                        shadow_campaign.simulation(),
                        &assets,
                        shadow_campaign.generation(),
                    )
                    .unwrap();
                assert_eq!(shadow_campaign.state_hash(), unchanged);
                assert_eq!(off.source_sprite_stats.shadow_calls, 0);
                if on.source_sprite_stats.shadow_writes > 0 && on.scene_indices != off.scene_indices
                {
                    visible_shadow = Some((row.entity_index, pose, on, off));
                    break 'actors;
                }
            }
        }
        let (shadow_actor, shadow_pose, shadow_on, shadow_off) = visible_shadow
            .expect("actual Level3 source actors must expose a final ground-shadow pixel change");
        eprintln!(
            "original shadow option actor={shadow_actor} camera={shadow_pose:?} passes={:?}",
            shadow_on.source_sprite_stats
        );
        assert_ne!(shadow_off.scene_indices, shadow_on.scene_indices);
        carrier.set_render_options(WorldRenderOptions {
            sky: false,
            ..WorldRenderOptions::default()
        });
        let no_sky = render(&mut carrier);
        let clear = assets.materials.profile.as_ref().unwrap().key_color_1;
        assert!(
            no_sky
                .scene_indices
                .iter()
                .zip(&enabled.scene_indices)
                .zip(no_sky.cave_terrain_orders.as_ref().unwrap())
                .any(|((&off, &on), &terrain)| terrain == 0 && off == clear && on != clear),
            "disabled sky exposes the source key-colour clear behind uncovered world pixels"
        );
        let map = carrier
            .render_map_world(
                campaign.inspection(),
                campaign.simulation(),
                &assets,
                campaign.generation(),
            )
            .unwrap()
            .to_vec();
        assert_ne!(
            map, original_map,
            "full-map viewport must not return pixels cached under prior options"
        );
        assert_eq!(
            map,
            carrier
                .render_map_world(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap(),
            "same options/view reads cannot advance image history"
        );
        carrier.set_render_options(WorldRenderOptions::default());
        let restored = render(&mut carrier);
        assert!(restored.source_sprite_stats.reflection_writes > 0);
        assert!(restored.source_sprite_stats.shadow_writes > 0);
        assert_eq!(restored, render(&mut carrier));
        assert_eq!(
            campaign.state_hash(),
            before,
            "graphics switches never advance gameplay"
        );
    }

    #[test]
    fn opening_native_world_frames_consume_real_source_banks_without_advancing_on_reads() {
        let cases = [
            (
                "mc-bhq8-level1-collision-v5-night",
                "mc-oi62-environment-level1-v2",
                crate::campaign::spec::CampaignSelection::LEVEL1,
            ),
            (
                "mc-level2-vault-successor-closure-20261025",
                "mc-level2-vault-successor-environment-20261025",
                crate::campaign::spec::CampaignSelection::LEVEL2,
            ),
            (
                "level3-day-block32-campaign-recipe-v2",
                "mc-level3-environment-exit-20261025",
                crate::campaign::spec::CampaignSelection::LEVEL3,
            ),
            (
                "mc-bhq6-ordinary-20261025-zclu5fjj/level4",
                "mc-oi62-environment-level4-audio-v3",
                crate::campaign::spec::CampaignSelection {
                    external_level: 4,
                    internal_level: 3,
                    map_type_id: 2,
                },
            ),
        ];
        let mut calls = 0;
        for (package, cache, selection) in cases {
            let cache = generated_path(cache);
            let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
                generated_path(package),
                &cache,
                selection,
            )
            .unwrap();
            let assets = DayIndexedFrameAssets::load(&cache).unwrap();
            let before = campaign.state_hash();
            let mut carrier = DayIndexedFrameState::new(campaign.generation());
            let first_started = Instant::now();
            let first = carrier
                .render_current(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap();
            let first_elapsed = first_started.elapsed();
            assert!(first.source_sprites_composited);
            calls += first.source_sprite_stats.primary_calls;
            eprintln!(
                "opening source world level={} passes={:?}",
                selection.external_level, first.source_sprite_stats
            );
            if selection.map_type_id == 2 {
                assert_eq!(
                    first.source_sprite_stats.reflection_calls, 0,
                    "Cave has no reflected world walk"
                );
            }
            let repeated = carrier
                .render_current(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap();
            assert_eq!(
                repeated, first,
                "presentation polling must not advance source pixel-operation state"
            );
            assert_eq!(
                campaign.state_hash(),
                before,
                "rendering does not advance gameplay"
            );
            // Each yaw changes the source view, so these samples cannot be
            // cache hits. Assets are already loaded even for the first frame;
            // this characterizes renderer CPU work, not end-to-end FPS.
            let saved_pose = campaign
                .simulation()
                .session
                .state
                .scratch
                .source_camera_pose;
            let initial_pose = campaign.simulation().source_camera_pose();
            let mut view_micros = Vec::new();
            for yaw_delta in [128_i16, 256, 384] {
                campaign
                    .simulation_mut()
                    .session
                    .state
                    .scratch
                    .source_camera_pose = Some(sim_session::CameraPose {
                    yaw: initial_pose.yaw.wrapping_add(yaw_delta) & 0x07ff,
                    ..initial_pose
                });
                let started = Instant::now();
                let rendered = carrier
                    .render_current(
                        campaign.inspection(),
                        campaign.simulation(),
                        &assets,
                        campaign.generation(),
                    )
                    .unwrap();
                view_micros.push(started.elapsed().as_micros());
                assert!(rendered.source_sprites_composited);
            }
            campaign
                .simulation_mut()
                .session
                .state
                .scratch
                .source_camera_pose = saved_pose;
            eprintln!(
                "opening native world CPU level={} profile=320x200 first_asset_loaded_us={} distinct_warm_yaw_us={view_micros:?}",
                selection.external_level,
                first_elapsed.as_micros(),
            );
            // Spawn views in Levels2–4 can legitimately contain no admitted
            // sprites. Inspect a bounded set of camera views near actual live
            // source rows instead of treating that empty view as coverage.
            let rows = campaign
                .simulation()
                .campaign_entity_presentations(0)
                .unwrap();
            let eligible: Vec<_> = rows
                .into_iter()
                .filter(|row| {
                    row.draw_order != u32::MAX
                        && row.logical_height >= 100
                        && (row.control_flags >> 16) & 0x808 == 0
                        && campaign
                            .simulation()
                            .session
                            .definition
                            .presentation
                            .particles[usize::from(row.render_parameter_index)]
                        .byte_10
                            == 0
                })
                .collect();
            let mut nonempty = false;
            let mut reflected = false;
            let mut shadowed = false;
            let mut checked_map = false;
            let mut horizontal_reflection_calls = 0;
            let mut horizontal_eye_rejections = 0;
            for row in eligible.iter().copied().take(16) {
                let ground = crate::legacy_terrain::interpolate_ground(
                    &campaign
                        .simulation()
                        .canonical_terrain()
                        .planes()
                        .ground_heights,
                    row.position.x,
                    row.position.y,
                );
                for depth in [1024_u16, 4096] {
                    horizontal_eye_rejections += usize::from(
                        ground + 128 >= crate::terrain_reflection::REFLECTION_EYE_Z_LIMIT,
                    );
                    campaign
                        .simulation_mut()
                        .session
                        .state
                        .scratch
                        .source_camera_pose = Some(sim_session::CameraPose {
                        x: row.position.x,
                        y: row.position.y.wrapping_add(depth),
                        z: (ground + 128) as i16,
                        yaw: 0,
                        pitch: 0,
                        roll: 0,
                    });
                    let view = carrier
                        .render_current(
                            campaign.inspection(),
                            campaign.simulation(),
                            &assets,
                            campaign.generation(),
                        )
                        .unwrap();
                    nonempty |= view.source_sprite_stats.primary_writes > 0;
                    shadowed |= view.source_sprite_stats.shadow_writes > 0;
                    reflected |= view.source_sprite_stats.reflection_writes > 0;
                    horizontal_reflection_calls += view.source_sprite_stats.reflection_calls;
                    if !checked_map && view.source_sprite_stats.primary_writes > 0 {
                        let map = carrier
                            .render_map_world(
                                campaign.inspection(),
                                campaign.simulation(),
                                &assets,
                                campaign.generation(),
                            )
                            .unwrap()
                            .to_vec();
                        assert_eq!(map.len(), 128 * 200);
                        let repeated = carrier
                            .render_map_world(
                                campaign.inspection(),
                                campaign.simulation(),
                                &assets,
                                campaign.generation(),
                            )
                            .unwrap();
                        assert_eq!(
                            map, repeated,
                            "map UI reads reuse their committed narrow viewport"
                        );
                        let ordinary_crop: Vec<_> = view
                            .scene_indices
                            .chunks_exact(320)
                            .flat_map(|line| line[192..320].iter().copied())
                            .collect();
                        assert_ne!(
                            map, ordinary_crop,
                            "the source map world uses its own 128-pixel projection, not a crop of the ordinary view"
                        );
                        if selection.external_level == 1 {
                            // Explicit source blur remains active in menu6;
                            // only automatic speed blur is disabled there.
                            campaign
                                .simulation_mut()
                                .session
                                .state
                                .live
                                .level_exit
                                .blur_mode = 2;
                            let raw = render_environment_indexed_frame(
                                campaign.inspection(),
                                campaign.simulation(),
                                &assets,
                                campaign.generation(),
                                None,
                                true,
                                carrier.last_sprite_pixel_mode,
                                carrier.render_options,
                            )
                            .unwrap();
                            let mut expected: Vec<_> = raw
                                .scene_indices
                                .chunks_exact(320)
                                .flat_map(|line| line[192..320].iter().copied())
                                .collect();
                            let tables = assets
                                .materials
                                .renderer_tables(campaign.simulation().level_exit_sequence());
                            for (pixel, &old) in expected.iter_mut().zip(&map) {
                                *pixel =
                                    tables[0x4000 + (usize::from(*pixel) << 8) + usize::from(old)];
                            }
                            let blurred = carrier
                                .render_map_world(
                                    campaign.inspection(),
                                    campaign.simulation(),
                                    &assets,
                                    campaign.generation(),
                                )
                                .unwrap()
                                .to_vec();
                            assert_eq!(
                                blurred, expected,
                                "source blur consumes narrow map history"
                            );
                            assert_eq!(
                                carrier
                                    .render_map_world(
                                        campaign.inspection(),
                                        campaign.simulation(),
                                        &assets,
                                        campaign.generation(),
                                    )
                                    .unwrap(),
                                blurred,
                                "map polling cannot advance blur history"
                            );
                            campaign
                                .simulation_mut()
                                .session
                                .state
                                .live
                                .level_exit
                                .blur_mode = 0;
                        }
                        checked_map = true;
                    }
                    if nonempty && shadowed && (selection.map_type_id == 2 || reflected) {
                        break;
                    }
                }
                if nonempty && shadowed && (selection.map_type_id == 2 || reflected) {
                    break;
                }
            }
            // A level camera at actor-ground+128 looking horizontally may
            // put every reflected sprite below the viewport. Find a real
            // nearby water viewpoint and aim at the actor's reflected
            // altitude; no entity, map or gameplay state is manufactured.
            if selection.map_type_id != 2 && !reflected {
                eprintln!(
                    "opening horizontal reflection search level={} calls={horizontal_reflection_calls} eye_limit_rejected_views={horizontal_eye_rejections}; searching actual water viewpoints",
                    selection.external_level
                );
                let planes = campaign.simulation().canonical_terrain().planes();
                let mut views = Vec::new();
                for row in &eligible {
                    for (dx, dy) in [(0_i16, 4096_i16), (4096, 0), (0, -4096), (-4096, 0)] {
                        let x = row.position.x.wrapping_add_signed(dx);
                        let y = row.position.y.wrapping_add_signed(dy);
                        let tile = usize::from(y >> 8) * 256 + usize::from(x >> 8);
                        if !crate::terrain_reflection::terrain_material_is_reflective(
                            planes.terrain_types[tile],
                        ) {
                            continue;
                        }
                        let eye_z =
                            crate::legacy_terrain::interpolate_ground(&planes.ground_heights, x, y)
                                + 128;
                        if eye_z >= crate::terrain_reflection::REFLECTION_EYE_Z_LIMIT {
                            continue;
                        }
                        let from = crate::Position {
                            x,
                            y,
                            z: eye_z as i16,
                        };
                        let yaw = crate::heading_for_positions(from, row.position) as i16;
                        let (_, depth, _) = crate::goat_sprite_raster::source_sprite_view(
                            x,
                            y,
                            yaw,
                            row.position.x,
                            row.position.y,
                            crate::goat_sprite_raster::Mode20SpriteProjectionLimits::exact()
                                .render_cutoff_squared,
                        )
                        .expect("bounded water viewpoint admits actual actor");
                        let scale = crate::terrain_frame::level1_projection(
                            camera_from_session(campaign.simulation()).unwrap(),
                        )
                        .scale;
                        let pitch = ((scale * (i32::from(row.position.z) + eye_z) / depth) * 256
                            / 320)
                            .clamp(-256, 256) as i16;
                        views.push((
                            row.entity_index,
                            sim_session::CameraPose {
                                x,
                                y,
                                z: eye_z as i16,
                                yaw,
                                pitch,
                                roll: 0,
                            },
                        ));
                        if views.len() == 8 {
                            break;
                        }
                    }
                    if views.len() == 8 {
                        break;
                    }
                }
                assert!(
                    !views.is_empty(),
                    "actual outdoor level{} needs an eligible water viewpoint",
                    selection.external_level
                );
                for (actor, pose) in views {
                    campaign
                        .simulation_mut()
                        .session
                        .state
                        .scratch
                        .source_camera_pose = Some(pose);
                    let view = carrier
                        .render_current(
                            campaign.inspection(),
                            campaign.simulation(),
                            &assets,
                            campaign.generation(),
                        )
                        .unwrap();
                    eprintln!(
                        "opening reflected water view level={} actor={actor} camera={pose:?} passes={:?}",
                        selection.external_level, view.source_sprite_stats
                    );
                    reflected |= view.source_sprite_stats.reflection_writes > 0;
                    if reflected {
                        break;
                    }
                }
            }
            eprintln!(
                "opening source world nonempty level={} primary={nonempty} shadow={shadowed} reflection={reflected}",
                selection.external_level
            );
            assert!(
                nonempty,
                "actual level{} source rows must reach nonempty native sprite pixels",
                selection.external_level
            );
            assert!(
                shadowed,
                "actual level{} eligible source rows must reach ground-shadow pixels",
                selection.external_level
            );
            if selection.map_type_id != 2 {
                assert!(
                    reflected,
                    "actual outdoor level{} source rows must reach reflected sprite pixels",
                    selection.external_level
                );
            }
        }
        assert!(
            calls > 0,
            "real opening world producers must reach the canonical source sprite consumer"
        );
    }

    #[test]
    fn exit_camera_and_palette_refresh_without_a_flight_tick() {
        let cache = generated_path("level3-day-spell-owner-cache-audio-v4/components/material");
        let mut campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            generated_path("level3-day-block32-campaign-recipe-v2"),
            &cache,
            crate::campaign::spec::CampaignSelection::LEVEL3,
        )
        .expect("installed Level3 package and materials");
        let assets = DayIndexedFrameAssets::load(&cache).unwrap();
        let mut carrier = DayIndexedFrameState::new(campaign.generation());
        let initial = carrier
            .render_current(
                campaign.inspection(),
                campaign.simulation(),
                &assets,
                campaign.generation(),
            )
            .unwrap();
        // Diagnose the action12 camera consumer directly. This neither earns
        // an exit trigger nor advances gameplay or campaign progression.
        {
            let session = campaign.simulation_mut();
            let profile = session.profile();
            let live = &mut session.session.state.live;
            live.level_exit = crate::level_exit::LevelExitSequence {
                active: true,
                phase: 5,
                timer: 12,
                ..crate::level_exit::LevelExitSequence::default()
            };
            live.level_exit.advance(
                &live.terrain,
                profile,
                &mut live.core.state,
                &mut live.core.context,
                None,
            );
        }
        assert_eq!(camera_from_session(campaign.simulation()).unwrap().fov, 127);
        let zoomed = carrier
            .render_current(
                campaign.inspection(),
                campaign.simulation(),
                &assets,
                campaign.generation(),
            )
            .unwrap();
        assert_eq!(zoomed.tick, initial.tick);
        assert_ne!(zoomed.terrain_input_hash, initial.terrain_input_hash);
        assert_ne!(zoomed.index_hash, initial.index_hash);
        for request in [6, 5] {
            {
                let live = &mut campaign.simulation_mut().session.state.live;
                let palette = &mut live.economy.fields.palette;
                palette.request(crate::world::fields::PaletteRequest {
                    mode: request,
                    count: 0,
                });
                assert!(palette.advance_frame());
                if request == 5 {
                    // Source setup, step0, then one actual black-fade step.
                    assert!(palette.advance_frame());
                    assert!(palette.advance_frame());
                }
                live.level_exit.palette_submode = palette.mode();
            }
            let frame = carrier
                .render_current(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap();
            assert_eq!(frame.tick, zoomed.tick);
            assert_eq!(frame.indices, zoomed.indices);
            assert_ne!(frame.palette_rgb8, zoomed.palette_rgb8);
            let mut expected = assets.palette_rgb8().to_vec();
            campaign
                .simulation()
                .field_palette()
                .apply_rgb8(&mut expected);
            assert_eq!(frame.palette_rgb8, expected);
            assert_eq!(
                carrier
                    .render_current(
                        campaign.inspection(),
                        campaign.simulation(),
                        &assets,
                        campaign.generation(),
                    )
                    .unwrap(),
                frame,
                "observing the same authority cannot advance palette or image history"
            );
        }
    }

    #[test]
    fn cave_minimap_angle8_selects_opaque_black_only_in_caves() {
        let mut tables = vec![0; TABLESD_BYTES];
        for phase in 0..64 {
            tables[0x14300 + phase * 4] = 255;
        }
        tables[0x14000] = 7;
        tables[7] = 99;
        let terrain = vec![0; TERRAIN_PLANE_BYTES];
        let shading = vec![0; TERRAIN_PLANE_BYTES];
        let camera = camera_from_view(sim_session::CameraPose::default(), 0).unwrap();
        for (cave, angle, expected) in [(true, 8, 0), (true, 0, 99), (false, 8, 99)] {
            let angles = vec![angle; TERRAIN_PLANE_BYTES];
            let mut indices =
                vec![42; usize::from(DAY_FRAME_WIDTH) * usize::from(DAY_FRAME_HEIGHT)];
            draw_minimap_terrain(
                &mut indices,
                usize::from(DAY_FRAME_WIDTH),
                camera,
                &terrain,
                &shading,
                &angles,
                cave,
                &tables,
            );
            assert_eq!(indices[32 * usize::from(DAY_FRAME_WIDTH) + 32], expected);
            assert_eq!(indices[100 * usize::from(DAY_FRAME_WIDTH) + 100], 42);
        }
    }

    #[test]
    fn real_level4_frame_retains_source_occlusion_planes() {
        let cache = generated_path("mc-oi62-environment-level4-audio-v3");
        let campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            generated_path("mc-bhq6-ordinary-20261025-zclu5fjj/level4"),
            &cache,
            crate::campaign::spec::CampaignSelection {
                external_level: 4,
                internal_level: 3,
                map_type_id: 2,
            },
        )
        .expect("installed Level4 package and owner assets");
        let assets = DayIndexedFrameAssets::load(&cache).unwrap();
        let before = campaign.state_hash();
        let mut carrier = DayIndexedFrameState::new(campaign.generation());
        let frame = carrier
            .render_current(
                campaign.inspection(),
                campaign.simulation(),
                &assets,
                campaign.generation(),
            )
            .unwrap();
        let terrain_orders = frame.cave_terrain_orders.as_ref().unwrap();
        let cell_orders = frame.cave_cell_orders.as_ref().unwrap();
        assert_eq!(terrain_orders.len(), frame.indices.len());
        assert_eq!(cell_orders.len(), TERRAIN_PLANE_BYTES);
        assert!(terrain_orders.iter().any(|&order| order != 0));
        assert!(cell_orders.iter().any(|&order| order != 0));
        assert_eq!(
            carrier
                .render_current(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .unwrap(),
            frame
        );
        assert_eq!(campaign.state_hash(), before);
        assert_eq!(frame.tick, 0);
    }

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
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .expect("actual runtime frame");
            assert!(first.indices.iter().any(|&pixel| pixel != 0));
            let orders = first
                .cave_terrain_orders
                .as_ref()
                .expect("outdoor pixel ranks");
            let cells = first
                .cave_cell_orders
                .as_ref()
                .expect("outdoor sprite buckets");
            assert_eq!(orders.len(), first.scene_indices.len());
            assert_eq!(cells.len(), TERRAIN_PLANE_BYTES);
            assert!(orders.iter().any(|&rank| rank != 0));
            assert!(cells.iter().any(|&rank| rank != 0));
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
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .expect("following runtime frame");
            assert_eq!(moved.tick, first.tick + 1);
            assert_ne!(moved.index_hash, first.index_hash);
            let moved_camera = camera_from_session(campaign.simulation()).unwrap();
            // Objective speech/flash progresses through real paused outer
            // frames while the terrain/image tick remains fixed.
            for _ in 0..8 {
                campaign.paused_frame();
            }
            let paused = state
                .render_current(
                    campaign.inspection(),
                    campaign.simulation(),
                    &assets,
                    campaign.generation(),
                )
                .expect("paused objective flash observes the current palette");
            assert_eq!(paused.tick, moved.tick);
            // PlayerEvents advances the outer turn while paused. The stored
            // view stays fixed, while source water/sky animation may change.
            let mut expected_camera = moved_camera;
            expected_camera.source_turn += 8;
            assert_eq!(
                camera_from_session(campaign.simulation()).unwrap(),
                expected_camera
            );
            assert_ne!(paused.terrain_input_hash, moved.terrain_input_hash);
            assert_ne!(paused.palette_rgb8, moved.palette_rgb8);
            let mut expected_palette = assets.palette_rgb8().to_vec();
            campaign
                .simulation()
                .field_palette()
                .apply_rgb8(&mut expected_palette);
            assert_eq!(paused.palette_rgb8, expected_palette);
        }
        assert!(
            DayIndexedFrameAssets::load_night(generated_path("mc-oi62-environment-level2-v2"))
                .is_err()
        );
    }

    #[test]
    fn real_cave12_reset_consumes_floor_ceiling_and_no_sky_materials() {
        let cache = generated_path("mc-oi62-environment-level12-v1");
        let campaign = crate::campaign::CampaignSession::open_with_owner_cache(
            generated_path("mc-bhq6-ordinary-20261025-zclu5fjj/level12"),
            &cache,
            crate::campaign::spec::CampaignSelection {
                external_level: 12,
                internal_level: 11,
                map_type_id: 2,
            },
        )
        .expect("genuine ordinary Cave12 opens at reset");
        let reset = campaign.simulation();
        assert!(reset.profile().cave_mode);
        assert_eq!(reset.state.tick, 0);
        let assets = DayIndexedFrameAssets::load(&cache).expect("actual Cave12 materials");
        assert!(assets.environment_resources().sky().is_none());
        let mut carrier = DayIndexedFrameState::new(campaign.generation());
        let frame = carrier
            .render_current(campaign.inspection(), reset, &assets, campaign.generation())
            .expect("genuine Cave reset renders without taking an unsupported gameplay turn");
        assert_eq!(frame.profile.map_type_id, 2);
        assert_eq!(frame.palette_rgb8, assets.palette_rgb8());
        let profile = ClassicHdRenderProfile::compatibility_320x200();
        let planes = reset.canonical_terrain().planes();
        let plan = build_cave_terrain(
            &profile,
            camera_from_session(reset).unwrap(),
            CaveTerrainResources {
                terrain_types: &planes.terrain_types,
                ground_heights: &planes.ground_heights,
                ceiling_heights: &planes.secondary_heights,
                shading: &planes.shading,
                angles: &planes.angles,
                clear_index: 254,
            },
        )
        .unwrap();
        let mut pixels = plan.initial_indices(&profile);
        let mut provenance = vec![0; profile.extent()];
        let mut writer = ClassicHdFrameWriter::from_planes(
            &profile,
            &mut pixels,
            &mut provenance,
            campaign.generation(),
            0,
            plan.input_hash,
            0,
        )
        .unwrap();
        let receipt = plan
            .rasterize(
                &mut writer,
                assets.terrain_atlas(),
                assets.tablesd(),
                DAY_TERRAIN_PROVENANCE,
            )
            .unwrap();
        let _ = writer.finish();
        let floor = provenance.iter().filter(|&&value| value == 3).count();
        let ceiling = provenance.iter().filter(|&&value| value == 5).count();
        eprintln!(
            "genuine Cave12 reset: visible_floor_pixels={floor}, visible_ceiling_pixels={ceiling}, writes={receipt:?}, camera={:?}, frame_hash={:016x}",
            camera_from_session(reset).unwrap(),
            frame.index_hash
        );
        assert!(floor > 0, "actual reset must write floor pixels");
        // Both original renderers draw each ceiling before its floor. This
        // high-vault reset overpaints the roof; material consumption must not
        // require a final visible roof pixel at this particular camera.
        let consumes_both = |writes: &crate::presentation::cave_terrain::CaveRasterReceipt| {
            writes.floor_pixels_written > 0 && writes.ceiling_pixels_written > 0
        };
        assert!(
            consumes_both(&receipt),
            "actual reset must rasterize both owner materials"
        );
        // Negative control uses the same real camera, geometry and resources,
        // with only the ceiling submissions omitted. It is not a game state
        // or a second runtime route and must fail the same consumption gate.
        let omitted = plan.without_ceiling_submissions_for_test();
        let mut omitted_pixels = omitted.initial_indices(&profile);
        let mut omitted_provenance = vec![0; profile.extent()];
        let mut writer = ClassicHdFrameWriter::from_planes(
            &profile,
            &mut omitted_pixels,
            &mut omitted_provenance,
            campaign.generation(),
            0,
            omitted.input_hash,
            0,
        )
        .unwrap();
        let omitted_receipt = omitted
            .rasterize(
                &mut writer,
                assets.terrain_atlas(),
                assets.tablesd(),
                DAY_TERRAIN_PROVENANCE,
            )
            .unwrap();
        let _ = writer.finish();
        assert!(omitted_receipt.floor_pixels_written > 0);
        assert!(
            !consumes_both(&omitted_receipt),
            "omitting the roof must fail material consumption"
        );
        assert_ne!(omitted.command_hash, plan.command_hash);
        let repeated = carrier
            .render_current(campaign.inspection(), reset, &assets, campaign.generation())
            .unwrap();
        assert_eq!(repeated, frame);
        let wrong =
            DayIndexedFrameAssets::load(generated_path("mc-oi62-environment-level2-v2")).unwrap();
        assert!(
            carrier
                .render_current(campaign.inspection(), reset, &wrong, campaign.generation())
                .is_err()
        );
        assert_eq!(carrier.current_frame(), Some(&frame));
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
                campaign.simulation(),
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
            render_level2_day_indexed_frame(day.inspection(), day.simulation(), &assets, 0)
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
            campaign.simulation(),
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
            render_level2_day_indexed_frame(campaign.inspection(), campaign.simulation(), assets, 0)
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
            campaign.simulation(),
            &assets,
            0,
        )
        .expect("corrected Day assets render the real campaign terrain");
        assert_eq!(frame.indices.len(), 320 * 200);
        assert!(frame.indices.iter().any(|index| *index != 0));
        let session = campaign.simulation();
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
