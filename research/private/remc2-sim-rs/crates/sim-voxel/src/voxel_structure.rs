//! Private, renderer-free VoxelMode structure contract.
//!
//! This module deliberately stops at the structure/detail boundary.  The
//! existing `sim-core` house state remains the gameplay authority; this
//! loader only accepts a semantic house description and exposes immutable
//! detail data for later mirror/renderer work.  It does not know about
//! fireballs, Unity, C ABI handles, PhysX, or Track S city data.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::from_iter_instead_of_collect,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_lines
)]

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Maximum values frozen for the private one-house prototype.
pub const FROZEN_CAPS: &[(&str, f64)] = &[
    ("cells", 4_096.0),
    ("imported_triangles", 100_000.0),
    ("persistent_game_objects", 48.0),
    ("renderers", 24.0),
    ("materials", 16.0),
    ("collider_proxies", 48.0),
    ("major_clusters", 12.0),
    ("active_cosmetic_debris", 256.0),
    ("sidecar_snapshot_bytes", 524_288.0),
    ("added_memory_bytes", 134_217_728.0),
    ("p95_dirty_rebuild_ms", 8.0),
    ("worst_impact_frame_ms", 33.0),
    ("managed_allocation_per_impact_bytes", 262_144.0),
    ("steady_frame_allocation_bytes", 1_024.0),
    ("monotonic_memory_growth_percent", 5.0),
];

/// Integer cell coordinate in the contract's right-handed +Y-up space.
pub type CellCoord = [i32; 3];

/// An error category suitable for telemetry and tests without relying on
/// serde's changing parser wording.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelManifestErrorKind {
    Json,
    Schema,
    Contract,
    Reference,
    Topology,
    Budget,
    Hash,
    Path,
    Io,
}

impl VoxelManifestErrorKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Schema => "schema",
            Self::Contract => "contract",
            Self::Reference => "reference",
            Self::Topology => "topology",
            Self::Budget => "budget",
            Self::Hash => "hash",
            Self::Path => "path",
            Self::Io => "io",
        }
    }
}

/// A deterministic contract failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelManifestError {
    pub kind: VoxelManifestErrorKind,
    pub path: String,
    pub message: String,
}

impl VoxelManifestError {
    fn new(
        kind: VoxelManifestErrorKind,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            message: message.into(),
        }
    }

    fn missing(path: &str, field: &str) -> Self {
        Self::new(
            VoxelManifestErrorKind::Contract,
            path,
            format!("missing required field {field:?}"),
        )
    }

    fn expect(
        kind: VoxelManifestErrorKind,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(kind, path, message)
    }
}

impl fmt::Display for VoxelManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.kind.label(), self.path, self.message)
    }
}

impl std::error::Error for VoxelManifestError {}

/// Fixed-point source-independent material properties.
#[derive(Clone, Debug, PartialEq)]
pub struct VoxelMaterial {
    pub id: String,
    pub kind: String,
    /// Density in whole kg/m³.  The contract currently supplies integral
    /// values; the parser rounds only after requiring a finite positive value.
    pub density_kg_m3: u32,
    /// Strength and health use milli-units to avoid float state in future
    /// damage/replay code.
    pub strength_milli: u32,
    pub health_milli: u32,
}

/// Immutable semantic cell record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelCell {
    pub id: String,
    pub coord: CellCoord,
    pub material_id: String,
    pub part_id: String,
    pub support_ids: Vec<String>,
    pub render_group_id: String,
}

/// The declared structural relation for one support group.  `face_connected`
/// documents the ordinary voxel adjacency that the collapse analyser already
/// traverses; `brace` is an authored non-face connection and therefore must be
/// consumed explicitly rather than inferred from coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelSupportKind {
    Foundation,
    FaceConnected,
    Brace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSupportGroup {
    pub id: String,
    pub kind: VoxelSupportKind,
    pub cell_ids: Vec<String>,
}

/// A compact read-only summary of one validated private house.
#[derive(Clone, Debug, PartialEq)]
pub struct VoxelStructure {
    pub manifest_id: String,
    pub structure_id: String,
    pub generation: u32,
    pub source_entity_id: String,
    pub building_index: u32,
    pub cell_bounds_min: CellCoord,
    pub cell_bounds_max: CellCoord,
    pub anchor_cell: CellCoord,
    /// Anchor world position in micrometres, retained for fixed-point impact
    /// transforms and never used as gameplay position state.
    pub anchor_world_um: [i64; 3],
    /// Cell size in micrometres (rounded from the positive contract value).
    pub cell_size_um: u64,
    pub cells: Vec<VoxelCell>,
    pub materials: Vec<VoxelMaterial>,
    /// Declared support groups, retained so structural authority can consume
    /// authored brace links without reparsing the manifest document.
    pub support_groups: Vec<VoxelSupportGroup>,
    pub foundation_cells: Vec<CellCoord>,
    /// Explicit window/door/breach coordinates remain empty detail cells.
    pub opening_cells: Vec<CellCoord>,
    pub semantic_sha256: String,
    pub schema_sha256: String,
    pub observed_budgets: BTreeMap<String, f64>,
    /// Retained only so future mirror code can inspect semantic rooms and
    /// openings without reparsing.  It is never used as gameplay state.
    semantic_document: Value,
}

impl VoxelStructure {
    /// Return the original semantic document.  The returned value is a clone
    /// to preserve the structure's immutable validation result.
    #[must_use]
    pub fn semantic_document(&self) -> Value {
        self.semantic_document.clone()
    }

    /// Look up a cell by its coordinate-derived stable ID.
    #[must_use]
    pub fn cell(&self, id: &str) -> Option<&VoxelCell> {
        self.cells.iter().find(|cell| cell.id == id)
    }

    /// Look up the stable cell ID occupying a coordinate, if that coordinate
    /// is part of the occupied semantic structure.
    #[must_use]
    pub fn cell_id_at(&self, coord: CellCoord) -> Option<&str> {
        self.cells
            .iter()
            .find(|cell| cell.coord == coord)
            .map(|cell| cell.id.as_str())
    }
}

/// Parse and validate one VoxelHouseManifest v1 against the supplied schema
/// bytes.  The schema bytes are hashed and compared to `hashes.schema_sha256`.
pub fn load_manifest(
    manifest_bytes: &[u8],
    schema_bytes: &[u8],
) -> Result<VoxelStructure, VoxelManifestError> {
    let document: Value = serde_json::from_slice(manifest_bytes).map_err(|error| {
        VoxelManifestError::new(
            VoxelManifestErrorKind::Json,
            "manifest",
            format!("cannot decode JSON: {error}"),
        )
    })?;
    validate_value(document, schema_bytes)
}

/// Load a manifest and its schema from paths relative to a private root.
///
/// Absolute paths, `..` components, and symlinks that resolve outside the
/// supplied root are rejected before any bytes are read.  This is an explicit
/// private-lab boundary; no raw Blender state is accepted by this API.
pub fn load_manifest_from_private_root(
    private_root: impl AsRef<Path>,
    manifest_relative: impl AsRef<Path>,
    schema_relative: impl AsRef<Path>,
) -> Result<VoxelStructure, VoxelManifestError> {
    let root = fs::canonicalize(private_root.as_ref()).map_err(|error| {
        VoxelManifestError::new(
            VoxelManifestErrorKind::Io,
            "private_root",
            error.to_string(),
        )
    })?;
    let manifest_path = resolve_private_path(&root, manifest_relative.as_ref(), "manifest_path")?;
    let schema_path = resolve_private_path(&root, schema_relative.as_ref(), "schema_path")?;
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
        VoxelManifestError::new(
            VoxelManifestErrorKind::Io,
            "manifest_path",
            error.to_string(),
        )
    })?;
    let schema_bytes = fs::read(&schema_path).map_err(|error| {
        VoxelManifestError::new(VoxelManifestErrorKind::Io, "schema_path", error.to_string())
    })?;
    load_manifest(&manifest_bytes, &schema_bytes)
}

/// Alias with a shorter name for callers that already have a private root.
pub fn load_manifest_from_path(
    private_root: impl AsRef<Path>,
    manifest_relative: impl AsRef<Path>,
    schema_relative: impl AsRef<Path>,
) -> Result<VoxelStructure, VoxelManifestError> {
    load_manifest_from_private_root(private_root, manifest_relative, schema_relative)
}

fn resolve_private_path(
    root: &Path,
    relative: &Path,
    path_name: &str,
) -> Result<PathBuf, VoxelManifestError> {
    if relative.is_absolute() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Path,
            path_name,
            "absolute paths are not allowed",
        ));
    }
    for component in relative.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Path,
                path_name,
                "parent/traversal path components are not allowed",
            ));
        }
    }
    let candidate = root.join(relative);
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        VoxelManifestError::new(VoxelManifestErrorKind::Io, path_name, error.to_string())
    })?;
    if !canonical.starts_with(root) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Path,
            path_name,
            "resolved path escapes private root",
        ));
    }
    Ok(canonical)
}

fn validate_value(
    document: Value,
    schema_bytes: &[u8],
) -> Result<VoxelStructure, VoxelManifestError> {
    let root = object(&document, "manifest")?;
    require_top_level(root)?;

    let schema = object(required(root, "schema", "manifest")?, "manifest.schema")?;
    if string(
        required(schema, "id", "manifest.schema")?,
        "manifest.schema.id",
    )? != "voxel-house-manifest"
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Schema,
            "manifest.schema.id",
            "must be voxel-house-manifest",
        ));
    }
    if integer(
        required(schema, "version", "manifest.schema")?,
        "manifest.schema.version",
    )? != 1
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Schema,
            "manifest.schema.version",
            "must be version 1",
        ));
    }
    let manifest_id = manifest_id(
        required(root, "manifest_id", "manifest")?,
        "manifest.manifest_id",
    )?;

    let coordinates = object(
        required(root, "coordinate_system", "manifest")?,
        "manifest.coordinate_system",
    )?;
    if string(
        required(coordinates, "unit", "manifest.coordinate_system")?,
        "manifest.coordinate_system.unit",
    )? != "m"
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.coordinate_system.unit",
            "must be meters (m)",
        ));
    }
    let cell_size = number(
        required(coordinates, "cell_size_m", "manifest.coordinate_system")?,
        "manifest.coordinate_system.cell_size_m",
    )?;
    if cell_size <= 0.0 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.coordinate_system.cell_size_m",
            "must be positive",
        ));
    }
    for (key, expected) in [
        ("handedness", "+right"),
        ("up_axis", "+Y"),
        ("forward_axis", "+Z"),
        ("right_axis", "+X"),
    ] {
        let actual = string(
            required(coordinates, key, "manifest.coordinate_system")?,
            &format!("manifest.coordinate_system.{key}"),
        )?;
        if (key == "handedness" && actual != "right") || (key != "handedness" && actual != expected)
        {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("manifest.coordinate_system.{key}"),
                format!(
                    "must be {}",
                    if key == "handedness" {
                        "right"
                    } else {
                        expected
                    }
                ),
            ));
        }
    }
    let anchor = object(
        required(coordinates, "anchor", "manifest.coordinate_system")?,
        "manifest.coordinate_system.anchor",
    )?;
    if string(
        required(anchor, "kind", "manifest.coordinate_system.anchor")?,
        "manifest.coordinate_system.anchor.kind",
    )? != "source_aabb_center_bottom"
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.coordinate_system.anchor.kind",
            "must be source_aabb_center_bottom",
        ));
    }
    let anchor_cell = coord(
        required(anchor, "cell_coord", "manifest.coordinate_system.anchor")?,
        "manifest.coordinate_system.anchor.cell_coord",
    )?;
    let anchor_world = vec3(
        required(anchor, "world_m", "manifest.coordinate_system.anchor")?,
        "manifest.coordinate_system.anchor.world_m",
        false,
    )?;
    let anchor_world_um =
        fixed_signed_vec3(anchor_world, "manifest.coordinate_system.anchor.world_m")?;

    let authority = object(
        required(root, "authority", "manifest")?,
        "manifest.authority",
    )?;
    let expected_authority = [
        ("gameplay", Value::String("source_house_state".to_owned())),
        ("detail", Value::String("voxel_structure_mirror".to_owned())),
        (
            "targeting",
            Value::String("source_authoritative".to_owned()),
        ),
        ("collision", Value::String("source_aabb".to_owned())),
        ("feedback_to_gameplay", Value::Bool(false)),
        ("standalone_town", Value::Bool(false)),
    ];
    for (key, expected) in expected_authority {
        if required(authority, key, "manifest.authority")? != &expected {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("manifest.authority.{key}"),
                format!("must be {expected}"),
            ));
        }
    }

    let profile = object(required(root, "profile", "manifest")?, "manifest.profile")?;
    let structure_id = id(
        required(profile, "structure_id", "manifest.profile")?,
        "manifest.profile.structure_id",
    )?
    .to_owned();
    let generation = nonnegative_u32(
        required(profile, "generation", "manifest.profile")?,
        "manifest.profile.generation",
    )?;
    let (cell_bounds_min, cell_bounds_max) = cell_bounds(
        required(profile, "cell_bounds", "manifest.profile")?,
        "manifest.profile.cell_bounds",
    )?;
    let visual_bounds = object(
        required(profile, "visual_bounds_m", "manifest.profile")?,
        "manifest.profile.visual_bounds_m",
    )?;
    let visual_min = vec3(
        required(visual_bounds, "min_m", "manifest.profile.visual_bounds_m")?,
        "manifest.profile.visual_bounds_m.min_m",
        false,
    )?;
    let visual_max = vec3(
        required(visual_bounds, "max_m", "manifest.profile.visual_bounds_m")?,
        "manifest.profile.visual_bounds_m.max_m",
        false,
    )?;
    if visual_min
        .iter()
        .zip(visual_max.iter())
        .any(|(low, high)| low > high)
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.profile.visual_bounds_m",
            "minimum must not exceed maximum",
        ));
    }
    let source_units_per_cell = number(
        required(profile, "source_units_per_cell", "manifest.profile")?,
        "manifest.profile.source_units_per_cell",
    )?;
    if source_units_per_cell <= 0.0 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.profile.source_units_per_cell",
            "must be positive",
        ));
    }

    let source_aabb = object(
        required(root, "source_gameplay_aabb", "manifest")?,
        "manifest.source_gameplay_aabb",
    )?;
    let source_entity_id = id(
        required(
            source_aabb,
            "source_entity_id",
            "manifest.source_gameplay_aabb",
        )?,
        "manifest.source_gameplay_aabb.source_entity_id",
    )?
    .to_owned();
    let building_index = nonnegative_u32(
        required(
            source_aabb,
            "building_index",
            "manifest.source_gameplay_aabb",
        )?,
        "manifest.source_gameplay_aabb.building_index",
    )?;
    let _source_center = vec3(
        required(source_aabb, "center_m", "manifest.source_gameplay_aabb")?,
        "manifest.source_gameplay_aabb.center_m",
        false,
    )?;
    let _source_half_extents = vec3(
        required(
            source_aabb,
            "half_extents_m",
            "manifest.source_gameplay_aabb",
        )?,
        "manifest.source_gameplay_aabb.half_extents_m",
        true,
    )?;
    if string(
        required(source_aabb, "source_space", "manifest.source_gameplay_aabb")?,
        "manifest.source_gameplay_aabb.source_space",
    )? != "mc2_source_units"
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.source_gameplay_aabb.source_space",
            "must be mc2_source_units",
        ));
    }
    if string(
        required(
            source_aabb,
            "removal_authority",
            "manifest.source_gameplay_aabb",
        )?,
        "manifest.source_gameplay_aabb.removal_authority",
    )? != "source_house_action_53"
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.source_gameplay_aabb.removal_authority",
            "must be source_house_action_53",
        ));
    }

    let provenance = object(
        required(root, "provenance", "manifest")?,
        "manifest.provenance",
    )?;
    let provenance_class = string(
        required(provenance, "class", "manifest.provenance")?,
        "manifest.provenance.class",
    )?
    .to_owned();
    if !matches!(
        provenance_class.as_str(),
        "synthetic-test" | "owner-derived-private"
    ) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.provenance.class",
            "must be a recognized private provenance class",
        ));
    }
    for key in ["source_ref", "recipe_ref", "license_ref"] {
        if string(
            required(provenance, key, "manifest.provenance")?,
            &format!("manifest.provenance.{key}"),
        )?
        .len()
            < 3
        {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("manifest.provenance.{key}"),
                "must not be empty",
            ));
        }
    }
    let interior_authorship = string(
        required(provenance, "interior_authorship", "manifest.provenance")?,
        "manifest.provenance.interior_authorship",
    )?;
    if !matches!(interior_authorship, "newly_authored" | "not_applicable") {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.provenance.interior_authorship",
            "must be newly_authored or not_applicable",
        ));
    }

    let hashes = object(required(root, "hashes", "manifest")?, "manifest.hashes")?;
    let mut hash_values = BTreeMap::new();
    for key in [
        "source_sha256",
        "recipe_sha256",
        "schema_sha256",
        "asset_sha256",
        "semantic_sha256",
    ] {
        let value = sha256(
            required(hashes, key, "manifest.hashes")?,
            &format!("manifest.hashes.{key}"),
        )?;
        hash_values.insert(key, value);
    }
    let expected_schema_hash = sha256_bytes(schema_bytes);
    if hash_values["schema_sha256"] != expected_schema_hash {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Hash,
            "manifest.hashes.schema_sha256",
            "does not match the pinned schema file",
        ));
    }

    let materials_value = array(
        required(root, "materials", "manifest")?,
        "manifest.materials",
    )?;
    if materials_value.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.materials",
            "must not be empty",
        ));
    }
    let mut material_ids = Vec::with_capacity(materials_value.len());
    let mut material_map = HashMap::new();
    let mut materials = Vec::with_capacity(materials_value.len());
    check_sorted_unique_ids(materials_value, "manifest.materials", &mut material_ids)?;
    for (index, value) in materials_value.iter().enumerate() {
        let path = format!("manifest.materials[{index}]");
        let item = object(value, &path)?;
        let kind = string(required(item, "kind", &path)?, &format!("{path}.kind"))?.to_owned();
        if !matches!(
            kind.as_str(),
            "masonry" | "wood" | "glass" | "metal" | "debris"
        ) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.kind"),
                "unknown material kind",
            ));
        }
        let density = number(
            required(item, "density_kg_m3", &path)?,
            &format!("{path}.density_kg_m3"),
        )?;
        if !(0.0 < density && density <= 20_000.0) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.density_kg_m3"),
                "must be in (0, 20000]",
            ));
        }
        let strength = number(
            required(item, "strength", &path)?,
            &format!("{path}.strength"),
        )?;
        let health = number(required(item, "health", &path)?, &format!("{path}.health"))?;
        if strength <= 0.0 || health <= 0.0 {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                path,
                "strength and health must be positive",
            ));
        }
        let density_kg_m3 = u32::try_from(fixed_unsigned(
            density,
            1.0,
            &format!("{path}.density_kg_m3"),
        )?)
        .map_err(|_| {
            VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.density_kg_m3"),
                "fixed-point density exceeds u32",
            )
        })?;
        let parsed = VoxelMaterial {
            id: material_ids[index].clone(),
            kind,
            density_kg_m3,
            strength_milli: fixed_milli(strength, &format!("{path}.strength"))?,
            health_milli: fixed_milli(health, &format!("{path}.health"))?,
        };
        material_map.insert(parsed.id.clone(), index);
        materials.push(parsed);
    }

    let cells_value = array(required(root, "cells", "manifest")?, "manifest.cells")?;
    if cells_value.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.cells",
            "must not be empty",
        ));
    }
    let mut cell_ids = Vec::with_capacity(cells_value.len());
    check_sorted_unique_ids(cells_value, "manifest.cells", &mut cell_ids)?;
    if cells_value.len() > cap_as_usize("cells") {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Budget,
            "manifest.cells",
            "exceeds frozen private cap",
        ));
    }
    let mut cells = Vec::with_capacity(cells_value.len());
    let mut cell_map = HashMap::new();
    let mut coord_to_id = HashMap::new();
    for (index, value) in cells_value.iter().enumerate() {
        let path = format!("manifest.cells[{index}]");
        let item = object(value, &path)?;
        let id_value = &cell_ids[index];
        let cell_coord = coord(required(item, "coord", &path)?, &format!("{path}.coord"))?;
        if id_value != &stable_cell_id(cell_coord) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.id"),
                "must be the stable coordinate-derived cell ID",
            ));
        }
        if cell_coord
            .iter()
            .zip(cell_bounds_min.iter().zip(cell_bounds_max.iter()))
            .any(|(value, (low, high))| value < low || value > high)
        {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Topology,
                format!("{path}.coord"),
                "lies outside profile.cell_bounds",
            ));
        }
        if coord_to_id.contains_key(&cell_coord) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Topology,
                format!("{path}.coord"),
                "overlaps another occupied cell",
            ));
        }
        if required(item, "occupied", &path)? != &Value::Bool(true) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.occupied"),
                "must be true for an occupied cell record",
            ));
        }
        let material_id = id(
            required(item, "material_id", &path)?,
            &format!("{path}.material_id"),
        )?;
        let part_id = id(
            required(item, "part_id", &path)?,
            &format!("{path}.part_id"),
        )?;
        let support_ids = id_array(
            required(item, "support_ids", &path)?,
            &format!("{path}.support_ids"),
        )?;
        let render_group_id = id(
            required(item, "render_group_id", &path)?,
            &format!("{path}.render_group_id"),
        )?;
        if !material_map.contains_key(material_id) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.material_id"),
                "references an unknown material",
            ));
        }
        let parsed = VoxelCell {
            id: id_value.clone(),
            coord: cell_coord,
            material_id: material_id.to_owned(),
            part_id: part_id.to_owned(),
            support_ids,
            render_group_id: render_group_id.to_owned(),
        };
        cell_map.insert(id_value.clone(), index);
        coord_to_id.insert(cell_coord, id_value.clone());
        cells.push(parsed);
    }

    let parts_value = array(required(root, "parts", "manifest")?, "manifest.parts")?;
    if parts_value.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.parts",
            "must not be empty",
        ));
    }
    let mut part_ids = Vec::with_capacity(parts_value.len());
    check_sorted_unique_ids(parts_value, "manifest.parts", &mut part_ids)?;
    let mut part_membership = HashMap::new();
    for (index, value) in parts_value.iter().enumerate() {
        let path = format!("manifest.parts[{index}]");
        let item = object(value, &path)?;
        let kind = string(required(item, "kind", &path)?, &format!("{path}.kind"))?;
        if !matches!(kind, "shell" | "roof" | "partition" | "fixture") {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.kind"),
                "unknown part kind",
            ));
        }
        let material_id = id(
            required(item, "material_id", &path)?,
            &format!("{path}.material_id"),
        )?;
        if !material_map.contains_key(material_id) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.material_id"),
                "references an unknown material",
            ));
        }
        let thickness = integer(
            required(item, "thickness_cells", &path)?,
            &format!("{path}.thickness_cells"),
        )?;
        if thickness < 1 {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.thickness_cells"),
                "thin features are not allowed",
            ));
        }
        let members = id_array(
            required(item, "cell_ids", &path)?,
            &format!("{path}.cell_ids"),
        )?;
        ensure_sorted_unique(&members, &format!("{path}.cell_ids"))?;
        for member in members {
            if !cell_map.contains_key(&member) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("{path}.cell_ids"),
                    format!("references unknown cell {member}"),
                ));
            }
            if part_membership
                .insert(member.clone(), part_ids[index].clone())
                .is_some()
            {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("{path}.cell_ids"),
                    format!("cell {member} belongs to multiple parts"),
                ));
            }
        }
    }
    if part_membership.len() != cell_map.len() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Reference,
            "manifest.parts",
            "must cover every occupied cell exactly once",
        ));
    }

    let supports_value = array(required(root, "supports", "manifest")?, "manifest.supports")?;
    if supports_value.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.supports",
            "must not be empty",
        ));
    }
    let mut support_ids = Vec::with_capacity(supports_value.len());
    check_sorted_unique_ids(supports_value, "manifest.supports", &mut support_ids)?;
    let mut support_map: HashMap<String, HashSet<String>> = HashMap::new();
    let mut support_groups = Vec::with_capacity(supports_value.len());
    let mut foundation_ids = HashSet::new();
    for (index, value) in supports_value.iter().enumerate() {
        let path = format!("manifest.supports[{index}]");
        let item = object(value, &path)?;
        let kind = string(required(item, "kind", &path)?, &format!("{path}.kind"))?;
        let support_kind = match kind {
            "foundation" => VoxelSupportKind::Foundation,
            "face_connected" => VoxelSupportKind::FaceConnected,
            "brace" => VoxelSupportKind::Brace,
            _ => {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Contract,
                    format!("{path}.kind"),
                    "unknown support kind",
                ));
            }
        };
        let members = id_array(
            required(item, "cell_ids", &path)?,
            &format!("{path}.cell_ids"),
        )?;
        ensure_sorted_unique(&members, &format!("{path}.cell_ids"))?;
        for member in &members {
            if !cell_map.contains_key(member) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("{path}.cell_ids"),
                    "references an unknown cell",
                ));
            }
        }
        if kind == "foundation" {
            foundation_ids.extend(members.iter().cloned());
        }
        support_map.insert(
            support_ids[index].clone(),
            members.iter().cloned().collect(),
        );
        support_groups.push(VoxelSupportGroup {
            id: support_ids[index].clone(),
            kind: support_kind,
            cell_ids: members,
        });
    }
    if foundation_ids.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.supports",
            "must contain at least one foundation cell",
        ));
    }
    for (index, cell) in cells_value.iter().enumerate() {
        let support_values = id_array(
            required(
                object(cell, &format!("manifest.cells[{index}]"))?,
                "support_ids",
                &format!("manifest.cells[{index}]"),
            )?,
            &format!("manifest.cells[{index}].support_ids"),
        )?;
        for support_id in support_values {
            if !support_map
                .get(&support_id)
                .is_some_and(|members| members.contains(&cell_ids[index]))
            {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("manifest.cells[{index}].support_ids"),
                    format!("support {support_id} does not contain this cell"),
                ));
            }
        }
    }
    let foundation_coords: HashSet<CellCoord> = foundation_ids
        .iter()
        .map(|id| cells[cell_map[id]].coord)
        .collect();

    let topology = object(required(root, "topology", "manifest")?, "manifest.topology")?;
    let shell_required = coord_set(
        required(topology, "shell_required", "manifest.topology")?,
        "manifest.topology.shell_required",
    )?;
    let topology_foundation = coord_set(
        required(topology, "foundation_cells", "manifest.topology")?,
        "manifest.topology.foundation_cells",
    )?;
    let topology_openings = coord_set(
        required(topology, "opening_cells", "manifest.topology")?,
        "manifest.topology.opening_cells",
    )?;
    if shell_required.is_empty() || topology_foundation.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.topology",
            "shell_required and foundation_cells must not be empty",
        ));
    }
    if !topology_foundation
        .iter()
        .all(|coord| coord_to_id.contains_key(coord))
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.topology.foundation_cells",
            "every foundation coordinate must be occupied",
        ));
    }
    if topology_foundation != foundation_coords {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.topology.foundation_cells",
            "must match the foundation support group",
        ));
    }
    if !shell_required
        .iter()
        .all(|coord| coord_to_id.contains_key(coord) || topology_openings.contains(coord))
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.topology.shell_required",
            "contains an unrepresented shell coordinate",
        ));
    }
    if shell_required
        .intersection(&topology_openings)
        .next()
        .is_some()
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.topology",
            "an opening cannot also be a required shell cell",
        ));
    }
    ensure_face_connected(&coord_to_id, &foundation_coords)?;

    let render_groups_value = array(
        required(root, "render_groups", "manifest")?,
        "manifest.render_groups",
    )?;
    if render_groups_value.is_empty() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest.render_groups",
            "must not be empty",
        ));
    }
    let mut group_ids = Vec::with_capacity(render_groups_value.len());
    check_sorted_unique_ids(
        render_groups_value,
        "manifest.render_groups",
        &mut group_ids,
    )?;
    let mut group_membership = HashMap::new();
    for (index, value) in render_groups_value.iter().enumerate() {
        let path = format!("manifest.render_groups[{index}]");
        let item = object(value, &path)?;
        let kind = string(
            required(item, "surface_kind", &path)?,
            &format!("{path}.surface_kind"),
        )?;
        if !matches!(
            kind,
            "masonry" | "roof" | "interior" | "window_frame" | "furniture"
        ) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.surface_kind"),
                "unknown render surface kind",
            ));
        }
        let material_id = id(
            required(item, "material_id", &path)?,
            &format!("{path}.material_id"),
        )?;
        if !material_map.contains_key(material_id) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.material_id"),
                "references an unknown material",
            ));
        }
        let members = id_array(
            required(item, "cell_ids", &path)?,
            &format!("{path}.cell_ids"),
        )?;
        ensure_sorted_unique(&members, &format!("{path}.cell_ids"))?;
        for member in members {
            if !cell_map.contains_key(&member) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("{path}.cell_ids"),
                    format!("references unknown cell {member}"),
                ));
            }
            if group_membership
                .insert(member.clone(), group_ids[index].clone())
                .is_some()
            {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("{path}.cell_ids"),
                    format!("cell {member} belongs to multiple render groups"),
                ));
            }
        }
    }
    if group_membership.len() != cell_map.len() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Reference,
            "manifest.render_groups",
            "must cover every occupied cell exactly once",
        ));
    }

    let rooms_value = array(required(root, "rooms", "manifest")?, "manifest.rooms")?;
    let mut room_ids = Vec::with_capacity(rooms_value.len());
    check_sorted_unique_ids(rooms_value, "manifest.rooms", &mut room_ids)?;
    let mut room_bounds = HashMap::new();
    for (index, value) in rooms_value.iter().enumerate() {
        let path = format!("manifest.rooms[{index}]");
        let item = object(value, &path)?;
        let _layout = id(
            required(item, "layout_id", &path)?,
            &format!("{path}.layout_id"),
        )?;
        let bounds = cell_bounds(required(item, "bounds", &path)?, &format!("{path}.bounds"))?;
        if bounds
            .0
            .iter()
            .zip(cell_bounds_min.iter().zip(cell_bounds_max.iter()))
            .any(|(value, (low, high))| value < low || value > high)
            || bounds
                .1
                .iter()
                .zip(cell_bounds_min.iter().zip(cell_bounds_max.iter()))
                .any(|(value, (low, high))| value < low || value > high)
        {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Topology,
                format!("{path}.bounds"),
                "lies outside profile.cell_bounds",
            ));
        }
        let furniture_ids = id_array(
            required(item, "furniture_ids", &path)?,
            &format!("{path}.furniture_ids"),
        )?;
        ensure_sorted_unique(&furniture_ids, &format!("{path}.furniture_ids"))?;
        room_bounds.insert(room_ids[index].clone(), bounds);
    }

    let furniture_value = array(
        required(root, "furniture", "manifest")?,
        "manifest.furniture",
    )?;
    let mut furniture_ids = Vec::with_capacity(furniture_value.len());
    check_sorted_unique_ids(furniture_value, "manifest.furniture", &mut furniture_ids)?;
    let mut furniture_rooms = HashMap::new();
    for (index, value) in furniture_value.iter().enumerate() {
        let path = format!("manifest.furniture[{index}]");
        let item = object(value, &path)?;
        let room_id = id(
            required(item, "room_id", &path)?,
            &format!("{path}.room_id"),
        )?;
        let Some(room_bounds_value) = room_bounds.get(room_id) else {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.room_id"),
                "references an unknown room",
            ));
        };
        let kind = string(required(item, "kind", &path)?, &format!("{path}.kind"))?;
        if !matches!(kind, "table" | "bed" | "chest" | "stair" | "lamp") {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.kind"),
                "unknown furniture kind",
            ));
        }
        let material_id = id(
            required(item, "material_id", &path)?,
            &format!("{path}.material_id"),
        )?;
        if !material_map.contains_key(material_id) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.material_id"),
                "references an unknown material",
            ));
        }
        let anchor = coord(
            required(item, "anchor_coord", &path)?,
            &format!("{path}.anchor_coord"),
        )?;
        let footprint = coord_vec(
            required(item, "footprint_cells", &path)?,
            &format!("{path}.footprint_cells"),
        )?;
        if !in_bounds(anchor, room_bounds_value)
            || footprint
                .iter()
                .any(|item| !in_bounds(*item, room_bounds_value))
        {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Topology,
                format!("{path}.footprint_cells"),
                "furniture lies outside its room",
            ));
        }
        if footprint.iter().any(|item| coord_to_id.contains_key(item)) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Topology,
                format!("{path}.footprint_cells"),
                "furniture may not occupy a structural cell",
            ));
        }
        furniture_rooms.insert(furniture_ids[index].clone(), room_id.to_owned());
    }
    for (index, value) in rooms_value.iter().enumerate() {
        let item = object(value, &format!("manifest.rooms[{index}]"))?;
        let furniture = id_array(
            required(item, "furniture_ids", &format!("manifest.rooms[{index}]"))?,
            &format!("manifest.rooms[{index}].furniture_ids"),
        )?;
        for id_value in furniture {
            if furniture_rooms.get(&id_value) != Some(&room_ids[index]) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Reference,
                    format!("manifest.rooms[{index}].furniture_ids"),
                    format!("furniture {id_value} belongs to another room or is unknown"),
                ));
            }
        }
    }

    let openings_value = array(required(root, "openings", "manifest")?, "manifest.openings")?;
    let mut opening_ids = Vec::with_capacity(openings_value.len());
    check_sorted_unique_ids(openings_value, "manifest.openings", &mut opening_ids)?;
    let mut opening_coords = HashSet::new();
    for (index, value) in openings_value.iter().enumerate() {
        let path = format!("manifest.openings[{index}]");
        let item = object(value, &path)?;
        let kind = string(required(item, "kind", &path)?, &format!("{path}.kind"))?;
        if !matches!(kind, "window" | "door" | "breach") {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                format!("{path}.kind"),
                "unknown opening kind",
            ));
        }
        let host_part = id(
            required(item, "host_part_id", &path)?,
            &format!("{path}.host_part_id"),
        )?;
        if !part_ids.iter().any(|id_value| id_value == host_part) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.host_part_id"),
                "references an unknown part",
            ));
        }
        let room_id = id(
            required(item, "room_id", &path)?,
            &format!("{path}.room_id"),
        )?;
        if !room_bounds.contains_key(room_id) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Reference,
                format!("{path}.room_id"),
                "references an unknown room",
            ));
        }
        let coords = coord_vec(
            required(item, "cell_coords", &path)?,
            &format!("{path}.cell_coords"),
        )?;
        if coords.is_empty() {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Topology,
                format!("{path}.cell_coords"),
                "must contain at least one unique coordinate",
            ));
        }
        for item_coord in coords {
            if !opening_coords.insert(item_coord) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Topology,
                    format!("{path}.cell_coords"),
                    "must contain at least one unique coordinate",
                ));
            }
            if coord_to_id.contains_key(&item_coord) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Topology,
                    format!("{path}.cell_coords"),
                    "opening coordinate is occupied",
                ));
            }
            if !in_bounds(item_coord, &(cell_bounds_min, cell_bounds_max)) {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Topology,
                    format!("{path}.cell_coords"),
                    "opening lies outside profile.cell_bounds",
                ));
            }
        }
    }
    if opening_coords != topology_openings {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.topology.opening_cells",
            "must match opening coordinates",
        ));
    }

    let volume = object(required(root, "volume", "manifest")?, "manifest.volume")?;
    let envelope_cells = cell_bounds_volume(cell_bounds_min, cell_bounds_max)?;
    if nonnegative_u64(
        required(volume, "envelope_cells", "manifest.volume")?,
        "manifest.volume.envelope_cells",
    )? != envelope_cells
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.volume.envelope_cells",
            "does not match profile.cell_bounds",
        ));
    }
    if nonnegative_u64(
        required(volume, "occupied_cells", "manifest.volume")?,
        "manifest.volume.occupied_cells",
    )? != cells.len() as u64
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.volume.occupied_cells",
            "does not match cells",
        ));
    }
    if nonnegative_u64(
        required(volume, "empty_cells", "manifest.volume")?,
        "manifest.volume.empty_cells",
    )? != envelope_cells - cells.len() as u64
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.volume.empty_cells",
            "does not match occupied cells",
        ));
    }
    let expected_cell_volume = cell_size * cell_size * cell_size;
    if (number(
        required(volume, "cell_volume_m3", "manifest.volume")?,
        "manifest.volume.cell_volume_m3",
    )? - expected_cell_volume)
        .abs()
        > 1e-9
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.volume.cell_volume_m3",
            "does not match coordinate_system.cell_size_m3",
        ));
    }
    if (number(
        required(volume, "occupied_volume_m3", "manifest.volume")?,
        "manifest.volume.occupied_volume_m3",
    )? - cells.len() as f64 * expected_cell_volume)
        .abs()
        > 1e-9
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.volume.occupied_volume_m3",
            "does not match occupied cells",
        ));
    }

    let budgets = object(required(root, "budgets", "manifest")?, "manifest.budgets")?;
    let limits = object(
        required(budgets, "limits", "manifest.budgets")?,
        "manifest.budgets.limits",
    )?;
    let observed = object(
        required(budgets, "observed", "manifest.budgets")?,
        "manifest.budgets.observed",
    )?;
    let mut observed_budgets = BTreeMap::new();
    for (key, frozen) in FROZEN_CAPS {
        let limit = number(
            required(limits, key, "manifest.budgets.limits")?,
            &format!("manifest.budgets.limits.{key}"),
        )?;
        let value = number(
            required(observed, key, "manifest.budgets.observed")?,
            &format!("manifest.budgets.observed.{key}"),
        )?;
        if is_integer_budget(key) && (limit.fract() != 0.0 || value.fract() != 0.0) {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Budget,
                format!("manifest.budgets.{key}"),
                "count budget values must be integral",
            ));
        }
        if limit > *frozen {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Budget,
                format!("manifest.budgets.limits.{key}"),
                format!("exceeds frozen private cap {frozen}"),
            ));
        }
        if value > limit {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Budget,
                format!("manifest.budgets.observed.{key}"),
                "exceeds declared limit",
            ));
        }
        if value < 0.0 {
            return Err(VoxelManifestError::expect(
                VoxelManifestErrorKind::Budget,
                format!("manifest.budgets.observed.{key}"),
                "must not be negative",
            ));
        }
        observed_budgets.insert((*key).to_owned(), value);
    }
    if observed_budgets["cells"] as usize != cells.len() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Budget,
            "manifest.budgets.observed.cells",
            "must match cell count",
        ));
    }
    if observed_budgets["materials"] as usize != materials.len() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Budget,
            "manifest.budgets.observed.materials",
            "must match material count",
        ));
    }

    let semantic_digest = semantic_sha256(&document)?;
    if hash_values["semantic_sha256"] != semantic_digest {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Hash,
            "manifest.hashes.semantic_sha256",
            "does not match canonical semantic content",
        ));
    }

    let cell_size_um = fixed_unsigned(
        cell_size,
        1_000_000.0,
        "manifest.coordinate_system.cell_size_m",
    )?;
    let mut foundation_cells: Vec<_> = foundation_coords.into_iter().collect();
    foundation_cells.sort_unstable();
    let mut opening_cells: Vec<_> = topology_openings.into_iter().collect();
    opening_cells.sort_unstable();
    Ok(VoxelStructure {
        manifest_id,
        structure_id,
        generation,
        source_entity_id,
        building_index,
        cell_bounds_min,
        cell_bounds_max,
        anchor_cell,
        anchor_world_um,
        cell_size_um,
        cells,
        materials,
        support_groups,
        foundation_cells,
        opening_cells,
        semantic_sha256: semantic_digest,
        schema_sha256: hash_values["schema_sha256"].clone(),
        observed_budgets,
        semantic_document: document,
    })
}

fn require_top_level(root: &Map<String, Value>) -> Result<(), VoxelManifestError> {
    const REQUIRED: &[&str] = &[
        "schema",
        "manifest_id",
        "coordinate_system",
        "authority",
        "profile",
        "source_gameplay_aabb",
        "provenance",
        "hashes",
        "materials",
        "parts",
        "cells",
        "supports",
        "rooms",
        "furniture",
        "openings",
        "render_groups",
        "topology",
        "volume",
        "budgets",
    ];
    for key in REQUIRED {
        if !root.contains_key(*key) {
            return Err(VoxelManifestError::missing("manifest", key));
        }
    }
    Ok(())
}

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, VoxelManifestError> {
    object
        .get(key)
        .ok_or_else(|| VoxelManifestError::missing(path, key))
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, VoxelManifestError> {
    value.as_object().ok_or_else(|| {
        VoxelManifestError::expect(VoxelManifestErrorKind::Contract, path, "must be an object")
    })
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, VoxelManifestError> {
    value.as_array().ok_or_else(|| {
        VoxelManifestError::expect(VoxelManifestErrorKind::Contract, path, "must be an array")
    })
}

fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str, VoxelManifestError> {
    value.as_str().ok_or_else(|| {
        VoxelManifestError::expect(VoxelManifestErrorKind::Contract, path, "must be a string")
    })
}

fn integer(value: &Value, path: &str) -> Result<i64, VoxelManifestError> {
    value.as_i64().ok_or_else(|| {
        VoxelManifestError::expect(VoxelManifestErrorKind::Contract, path, "must be an integer")
    })
}

fn nonnegative_u32(value: &Value, path: &str) -> Result<u32, VoxelManifestError> {
    let value = integer(value, path)?;
    u32::try_from(value).map_err(|_| {
        VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must be a non-negative 32-bit integer",
        )
    })
}

fn nonnegative_u64(value: &Value, path: &str) -> Result<u64, VoxelManifestError> {
    let value = integer(value, path)?;
    u64::try_from(value).map_err(|_| {
        VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must be a non-negative integer",
        )
    })
}

fn number(value: &Value, path: &str) -> Result<f64, VoxelManifestError> {
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                path,
                "must be a finite number",
            )
        })
}

fn vec3(value: &Value, path: &str, positive: bool) -> Result<[f64; 3], VoxelManifestError> {
    let values = array(value, path)?;
    if values.len() != 3 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must contain exactly three numbers",
        ));
    }
    let result = [
        number(&values[0], &format!("{path}[0]"))?,
        number(&values[1], &format!("{path}[1]"))?,
        number(&values[2], &format!("{path}[2]"))?,
    ];
    if positive && result.iter().any(|value| *value <= 0.0) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "all components must be positive",
        ));
    }
    Ok(result)
}

fn coord(value: &Value, path: &str) -> Result<CellCoord, VoxelManifestError> {
    let values = array(value, path)?;
    if values.len() != 3 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must contain exactly three integer coordinates",
        ));
    }
    Ok([
        i32::try_from(integer(&values[0], &format!("{path}[0]"))?).map_err(|_| {
            VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                path,
                "coordinate exceeds i32",
            )
        })?,
        i32::try_from(integer(&values[1], &format!("{path}[1]"))?).map_err(|_| {
            VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                path,
                "coordinate exceeds i32",
            )
        })?,
        i32::try_from(integer(&values[2], &format!("{path}[2]"))?).map_err(|_| {
            VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                path,
                "coordinate exceeds i32",
            )
        })?,
    ])
}

fn cell_bounds(value: &Value, path: &str) -> Result<(CellCoord, CellCoord), VoxelManifestError> {
    let object = object(value, path)?;
    let min = coord(required(object, "min", path)?, &format!("{path}.min"))?;
    let max = coord(required(object, "max", path)?, &format!("{path}.max"))?;
    if min.iter().zip(max.iter()).any(|(low, high)| low > high) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            path,
            "minimum must not exceed maximum",
        ));
    }
    Ok((min, max))
}

fn in_bounds(coord: CellCoord, bounds: &(CellCoord, CellCoord)) -> bool {
    coord
        .iter()
        .zip(bounds.0.iter().zip(bounds.1.iter()))
        .all(|(value, (low, high))| value >= low && value <= high)
}

fn coord_set(value: &Value, path: &str) -> Result<HashSet<CellCoord>, VoxelManifestError> {
    let values = array(value, path)?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| coord(value, &format!("{path}[{index}]")))
        .collect()
}

fn coord_vec(value: &Value, path: &str) -> Result<Vec<CellCoord>, VoxelManifestError> {
    let values = array(value, path)?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| coord(value, &format!("{path}[{index}]")))
        .collect()
}

fn id<'a>(value: &'a Value, path: &str) -> Result<&'a str, VoxelManifestError> {
    let value = string(value, path)?;
    if !valid_id(value) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must be a lowercase stable hyphenated ID",
        ));
    }
    Ok(value)
}

fn manifest_id(value: &Value, path: &str) -> Result<String, VoxelManifestError> {
    let value = string(value, path)?;
    if value.len() < 3
        || value.len() > 96
        || !value.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
        || !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must be a lowercase stable manifest ID",
        ));
    }
    Ok(value.to_owned())
}

fn valid_id(value: &str) -> bool {
    value.len() >= 2
        && value.len() <= 96
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

fn sha256(value: &Value, path: &str) -> Result<String, VoxelManifestError> {
    let value = string(value, path)?;
    if value.len() != 64
        || value == "0".repeat(64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Hash,
            path,
            "must be a non-zero lowercase SHA-256 digest",
        ));
    }
    Ok(value.to_owned())
}

fn id_array(value: &Value, path: &str) -> Result<Vec<String>, VoxelManifestError> {
    let values = array(value, path)?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| id(value, &format!("{path}[{index}]")))
        .map(|result| result.map(str::to_owned))
        .collect()
}

fn check_sorted_unique_ids(
    values: &[Value],
    path: &str,
    output: &mut Vec<String>,
) -> Result<(), VoxelManifestError> {
    for (index, value) in values.iter().enumerate() {
        output.push(
            id(
                object(value, &format!("{path}[{index}]"))?
                    .get("id")
                    .ok_or_else(|| {
                        VoxelManifestError::missing(&format!("{path}[{index}]"), "id")
                    })?,
                &format!("{path}[{index}].id"),
            )?
            .to_owned(),
        );
    }
    ensure_sorted_unique(output, path)
}

fn ensure_sorted_unique(values: &[String], path: &str) -> Result<(), VoxelManifestError> {
    if values.windows(2).any(|window| window[0] > window[1]) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "items must be sorted by stable ID",
        ));
    }
    if values.windows(2).any(|window| window[0] == window[1]) {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "contains duplicate IDs",
        ));
    }
    Ok(())
}

fn stable_cell_id(coord: CellCoord) -> String {
    fn component(value: i32) -> String {
        format!(
            "{}{}",
            if value < 0 { 'n' } else { 'p' },
            value.unsigned_abs()
        )
    }
    format!(
        "cell-{}-{}-{}",
        component(coord[0]),
        component(coord[1]),
        component(coord[2])
    )
}

fn cell_bounds_volume(min: CellCoord, max: CellCoord) -> Result<u64, VoxelManifestError> {
    let mut volume = 1_u64;
    for (low, high) in min.into_iter().zip(max) {
        volume = volume
            .checked_mul(
                u64::try_from(i64::from(high) - i64::from(low) + 1).map_err(|_| {
                    VoxelManifestError::expect(
                        VoxelManifestErrorKind::Topology,
                        "manifest.profile.cell_bounds",
                        "volume overflow",
                    )
                })?,
            )
            .ok_or_else(|| {
                VoxelManifestError::expect(
                    VoxelManifestErrorKind::Budget,
                    "manifest.profile.cell_bounds",
                    "volume overflow",
                )
            })?;
    }
    Ok(volume)
}

fn neighbors(coord: CellCoord) -> [Option<CellCoord>; 6] {
    [
        coord[0].checked_sub(1).map(|x| [x, coord[1], coord[2]]),
        coord[0].checked_add(1).map(|x| [x, coord[1], coord[2]]),
        coord[1].checked_sub(1).map(|y| [coord[0], y, coord[2]]),
        coord[1].checked_add(1).map(|y| [coord[0], y, coord[2]]),
        coord[2].checked_sub(1).map(|z| [coord[0], coord[1], z]),
        coord[2].checked_add(1).map(|z| [coord[0], coord[1], z]),
    ]
}

fn ensure_face_connected(
    coord_to_id: &HashMap<CellCoord, String>,
    foundation: &HashSet<CellCoord>,
) -> Result<(), VoxelManifestError> {
    let mut reachable = foundation.clone();
    let mut queue = VecDeque::from_iter(foundation.iter().copied());
    while let Some(current) = queue.pop_front() {
        for neighbor in neighbors(current).into_iter().flatten() {
            if coord_to_id.contains_key(&neighbor) && reachable.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    if reachable.len() != coord_to_id.len() {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Topology,
            "manifest.cells",
            "contains an unsupported island disconnected from the foundation",
        ));
    }
    Ok(())
}

fn fixed_milli(value: f64, path: &str) -> Result<u32, VoxelManifestError> {
    fixed_unsigned(value, 1_000.0, path).and_then(|value| {
        u32::try_from(value).map_err(|_| {
            VoxelManifestError::expect(
                VoxelManifestErrorKind::Contract,
                path,
                "fixed-point value exceeds u32",
            )
        })
    })
}

fn fixed_unsigned(value: f64, scale: f64, path: &str) -> Result<u64, VoxelManifestError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "must be positive and finite",
        ));
    }
    let scaled = value * scale;
    if !scaled.is_finite() || scaled > u64::MAX as f64 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "fixed-point value overflows",
        ));
    }
    let rounded = scaled.round();
    if rounded < 1.0 {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            path,
            "value is below the fixed-point precision",
        ));
    }
    Ok(rounded as u64)
}

fn fixed_signed_vec3(value: [f64; 3], path: &str) -> Result<[i64; 3], VoxelManifestError> {
    value
        .into_iter()
        .enumerate()
        .map(|(index, component)| {
            let scaled = component * 1_000_000.0;
            if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
                return Err(VoxelManifestError::expect(
                    VoxelManifestErrorKind::Contract,
                    format!("{path}[{index}]"),
                    "fixed-point coordinate overflows",
                ));
            }
            Ok(scaled.round() as i64)
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(|components| {
            components.try_into().map_err(|_| {
                VoxelManifestError::expect(
                    VoxelManifestErrorKind::Contract,
                    path,
                    "fixed-point coordinate must contain three components",
                )
            })
        })
}

fn cap_as_usize(key: &str) -> usize {
    FROZEN_CAPS
        .iter()
        .find(|(name, _)| *name == key)
        .map_or(usize::MAX, |(_, value)| *value as usize)
}

fn is_integer_budget(key: &str) -> bool {
    matches!(
        key,
        "cells"
            | "imported_triangles"
            | "persistent_game_objects"
            | "renderers"
            | "materials"
            | "collider_proxies"
            | "major_clusters"
            | "active_cosmetic_debris"
            | "sidecar_snapshot_bytes"
            | "added_memory_bytes"
            | "managed_allocation_per_impact_bytes"
            | "steady_frame_allocation_bytes"
    )
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn semantic_sha256(document: &Value) -> Result<String, VoxelManifestError> {
    let mut semantic = document.clone();
    let Some(root) = semantic.as_object_mut() else {
        return Err(VoxelManifestError::expect(
            VoxelManifestErrorKind::Contract,
            "manifest",
            "must be an object",
        ));
    };
    let Some(hashes) = root.get_mut("hashes").and_then(Value::as_object_mut) else {
        return Err(VoxelManifestError::missing("manifest", "hashes"));
    };
    hashes.remove("semantic_sha256");
    Ok(sha256_bytes(&canonical_json(&semantic)?))
}

/// Canonical JSON matching the contract's Python validator: UTF-8 JSON,
/// sorted object keys, compact separators, and original JSON number lexemes.
fn canonical_json(value: &Value) -> Result<Vec<u8>, VoxelManifestError> {
    let mut output = String::new();
    write_canonical(value, &mut output)?;
    Ok(output.into_bytes())
}

fn write_canonical(value: &Value, output: &mut String) -> Result<(), VoxelManifestError> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => write_ascii_json_string(value, output),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_canonical(value, output)?;
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<&String> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_ascii_json_string(key, output);
                output.push(':');
                write_canonical(&values[key], output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

fn write_ascii_json_string(value: &str, output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_ascii() && character.is_control() => {
                let value = character as u8;
                output.push_str("\\u00");
                output.push(HEX[(value >> 4) as usize] as char);
                output.push(HEX[(value & 0x0f) as usize] as char);
            }
            character if character.is_ascii() => output.push(character),
            character => {
                let mut units = [0_u16; 2];
                for unit in character.encode_utf16(&mut units) {
                    output.push_str("\\u");
                    output.push(HEX[(((*unit) >> 12) & 0x0f) as usize] as char);
                    output.push(HEX[(((*unit) >> 8) & 0x0f) as usize] as char);
                    output.push(HEX[(((*unit) >> 4) & 0x0f) as usize] as char);
                    output.push(HEX[((*unit) & 0x0f) as usize] as char);
                }
            }
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn fixture_bytes() -> (Vec<u8>, Vec<u8>) {
        let root = fixture_root();
        (
            fs::read(root.join(
                "generated/voxel-house-lookdev/contract/fixtures/golden-synthetic-house.json",
            ))
            .unwrap(),
            fs::read(root.join(
                "generated/voxel-house-lookdev/contract/voxel-house-manifest-v1.schema.json",
            ))
            .unwrap(),
        )
    }

    #[test]
    fn golden_fixture_is_valid_and_hash_stable() {
        let (manifest, schema) = fixture_bytes();
        let structure = load_manifest(&manifest, &schema).unwrap();
        assert_eq!(structure.cells.len(), 32);
        assert_eq!(
            structure.semantic_sha256,
            "e179290bab0a9cd8a75d6c7a9356cee92cceba52471287b98d365da3a5873534"
        );
        assert_eq!(structure.cell_size_um, 500_000);
        assert_eq!(
            structure.semantic_document().as_object().unwrap()["authority"]["gameplay"],
            "source_house_state"
        );
    }

    #[test]
    fn object_key_order_does_not_change_semantic_hash() {
        let (manifest, schema) = fixture_bytes();
        let value: Value = serde_json::from_slice(&manifest).unwrap();
        let reordered = serde_json::to_vec(&reverse_object_keys(value)).unwrap();
        let first = load_manifest(&manifest, &schema).unwrap();
        let second = load_manifest(&reordered, &schema).unwrap();
        assert_eq!(first.semantic_sha256, second.semantic_sha256);
    }

    fn reverse_object_keys(value: Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .rev()
                    .map(|(key, value)| (key, reverse_object_keys(value)))
                    .collect(),
            ),
            Value::Array(values) => {
                Value::Array(values.into_iter().map(reverse_object_keys).collect())
            }
            value => value,
        }
    }

    #[test]
    fn contract_guards_reject_wrong_version_axis_anchor_density_and_hash() {
        let (manifest, schema) = fixture_bytes();
        let mutations: [(&str, fn(&mut Value)); 5] = [
            ("schema.version", |value: &mut Value| {
                value["schema"]["version"] = Value::from(2)
            }),
            ("coordinate_system.up_axis", |value: &mut Value| {
                value["coordinate_system"]["up_axis"] = Value::from("+Z")
            }),
            ("anchor.kind", |value: &mut Value| {
                value["coordinate_system"]["anchor"]["kind"] = Value::from("world_origin")
            }),
            ("density_kg_m3", |value: &mut Value| {
                value["materials"][1]["density_kg_m3"] = Value::from(0)
            }),
            ("asset_sha256", |value: &mut Value| {
                value["hashes"]
                    .as_object_mut()
                    .unwrap()
                    .remove("asset_sha256");
            }),
        ];
        for (needle, mutate) in mutations {
            let mut value: Value = serde_json::from_slice(&manifest).unwrap();
            mutate(&mut value);
            let bytes = serde_json::to_vec(&value).unwrap();
            let error = load_manifest(&bytes, &schema).unwrap_err().to_string();
            assert!(error.contains(needle), "{error} did not contain {needle}");
        }

        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        value["materials"][1]["density_kg_m3"] = Value::from(0.1);
        let error = load_manifest(&serde_json::to_vec(&value).unwrap(), &schema)
            .unwrap_err()
            .to_string();
        assert!(error.contains("fixed-point precision"), "{error}");

        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        value["budgets"]["limits"]["cells"] = Value::from(4_096.5);
        let error = load_manifest(&serde_json::to_vec(&value).unwrap(), &schema)
            .unwrap_err()
            .to_string();
        assert!(error.contains("integral"), "{error}");

        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        value["cells"][3]["support_ids"] = serde_json::json!(["support-foundation"]);
        let error = load_manifest(&serde_json::to_vec(&value).unwrap(), &schema)
            .unwrap_err()
            .to_string();
        assert!(error.contains("does not contain this cell"), "{error}");
    }

    #[test]
    fn topology_rejects_duplicate_overlap_and_islands() {
        let (manifest, schema) = fixture_bytes();
        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        value["cells"][1]["coord"] = value["cells"][0]["coord"].clone();
        let error = load_manifest(&serde_json::to_vec(&value).unwrap(), &schema)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("overlaps") || error.contains("stable coordinate"),
            "{error}"
        );

        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        let island = serde_json::json!({"id":"cell-p9-p9-p9","coord":[9,9,9],"occupied":true,"material_id":"material-masonry","part_id":"part-shell","support_ids":["support-structure"],"render_group_id":"render-shell"});
        value["cells"].as_array_mut().unwrap().push(island);
        let error = load_manifest(&serde_json::to_vec(&value).unwrap(), &schema)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("outside") || error.contains("stable coordinate"),
            "{error}"
        );
    }

    #[test]
    fn budgets_and_safe_paths_are_enforced() {
        let (manifest, schema) = fixture_bytes();
        let root = fixture_root();
        let good = load_manifest_from_private_root(
            &root,
            "generated/voxel-house-lookdev/contract/fixtures/golden-synthetic-house.json",
            "generated/voxel-house-lookdev/contract/voxel-house-manifest-v1.schema.json",
        )
        .unwrap();
        assert_eq!(good.cells.len(), 32);
        let error = load_manifest_from_private_root(
            &root,
            "../outside.json",
            "generated/voxel-house-lookdev/contract/voxel-house-manifest-v1.schema.json",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("path"), "{error}");

        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        value["budgets"]["limits"]["cells"] = Value::from(4_097);
        let error = load_manifest(&serde_json::to_vec(&value).unwrap(), &schema)
            .unwrap_err()
            .to_string();
        assert!(error.contains("budgets.limits.cells"), "{error}");
    }

    #[test]
    fn canonical_strings_match_ascii_escaped_python_form() {
        let value = serde_json::json!({"é": "😀", "line": "\n"});
        let bytes = canonical_json(&value).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"line":"\n","\u00e9":"\ud83d\ude00"}"#
        );
    }

    #[test]
    fn boundary_neighbors_are_checked_instead_of_wrapping_or_panicking() {
        let minimum = neighbors([i32::MIN, i32::MIN, i32::MIN]);
        assert!(minimum[0].is_none());
        assert!(minimum[2].is_none());
        assert!(minimum[4].is_none());
        let maximum = neighbors([i32::MAX, i32::MAX, i32::MAX]);
        assert!(maximum[1].is_none());
        assert!(maximum[3].is_none());
        assert!(maximum[5].is_none());
    }
}
