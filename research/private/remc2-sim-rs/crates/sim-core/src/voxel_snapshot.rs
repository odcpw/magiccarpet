//! Versioned private VoxelMode snapshots and ordered sparse deltas.
//!
//! Snapshots are an internal renderer-free detail artifact.  They preserve
//! the identity/hash boundary of one synthetic structure, sparse cell damage,
//! lifecycle metadata, derived opening reveals, and the deterministic support
//! report.  The source `HouseState`, C ABI, Unity presentation, and Track S
//! runtime never consume this module.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use sha2::{Digest, Sha256};

use super::{
    voxel_collapse::{VoxelCollapseAnalyzer, VoxelCollapseReport},
    voxel_state::{VoxelSourceCursor, VoxelStateError, VoxelStructureState},
    voxel_structure::{CellCoord, VoxelStructure},
};

/// Current private snapshot wire/version contract.
pub const VOXEL_SNAPSHOT_VERSION: u16 = 1;
/// Sparse snapshot/delta cell cap.  This is deliberately no larger than the
/// validated private structure cell budget.
pub const MAX_SNAPSHOT_CELLS: usize = 4_096;
/// Maximum derived opening coordinates retained by one snapshot.
pub const MAX_REVEALED_OPENINGS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSnapshotHeader {
    pub version: u16,
    pub generation: u32,
    pub tick: u64,
    pub source_cursor: Option<VoxelSourceCursor>,
    pub source_entity_id: String,
    pub profile_id: String,
    pub manifest_id: String,
    /// Semantic manifest identity (the validated `semantic_sha256` field).
    pub manifest_sha256: String,
    pub recipe_sha256: String,
    pub schema_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSnapshotCell {
    pub id: String,
    pub coord: CellCoord,
    pub material_id: String,
    pub health_milli: u32,
    pub present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSnapshotLifecycle {
    pub destruction_started: bool,
    pub possessed_owner: Option<u16>,
    pub possession_payload: u32,
    pub source_life: i32,
    pub source_action: u8,
    pub source_active: bool,
    pub impact_position_um: Option<[i64; 3]>,
    pub damage_hit_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSnapshot {
    pub header: VoxelSnapshotHeader,
    /// Cells whose health/presence differs from the intact manifest baseline.
    /// Missing cells are represented with `present = false` and zero health.
    pub changed_cells: Vec<VoxelSnapshotCell>,
    /// Explicit sorted index of the missing subset for cheap destruction
    /// queries and stable delta comparison.
    pub missing_cell_ids: Vec<String>,
    /// Opening coordinates made visible by an adjacent missing shell cell.
    pub revealed_opening_cells: Vec<CellCoord>,
    pub lifecycle: VoxelSnapshotLifecycle,
    pub components: VoxelCollapseReport,
    pub snapshot_hash_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSparseDelta {
    pub version: u16,
    pub from_snapshot_hash_sha256: String,
    pub to_snapshot_hash_sha256: String,
    pub from_tick: u64,
    pub from_cursor: Option<VoxelSourceCursor>,
    pub target_header: VoxelSnapshotHeader,
    /// Final target values for cells that differ from the source snapshot.
    pub changed_cells: Vec<VoxelSnapshotCell>,
    pub missing_cell_ids: Vec<String>,
    pub revealed_opening_cells: Vec<CellCoord>,
    pub lifecycle: VoxelSnapshotLifecycle,
    pub components: VoxelCollapseReport,
    pub delta_hash_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelDeltaApplyResult {
    Applied,
    AlreadyApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelSnapshotErrorKind {
    Version,
    Identity,
    Generation,
    TickOrder,
    CursorOrder,
    Hash,
    Corrupt,
    UnknownCell,
    InvalidSparseState,
    Cap,
    State,
}

impl VoxelSnapshotErrorKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Identity => "identity",
            Self::Generation => "generation",
            Self::TickOrder => "tick-order",
            Self::CursorOrder => "cursor-order",
            Self::Hash => "hash",
            Self::Corrupt => "corrupt",
            Self::UnknownCell => "unknown-cell",
            Self::InvalidSparseState => "invalid-sparse-state",
            Self::Cap => "cap",
            Self::State => "state",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSnapshotError {
    pub kind: VoxelSnapshotErrorKind,
    pub message: String,
}

impl VoxelSnapshotError {
    fn new(kind: VoxelSnapshotErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for VoxelSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.label(), self.message)
    }
}

impl std::error::Error for VoxelSnapshotError {}

impl From<VoxelStateError> for VoxelSnapshotError {
    fn from(error: VoxelStateError) -> Self {
        Self::new(VoxelSnapshotErrorKind::State, error.to_string())
    }
}

impl VoxelSnapshot {
    /// Capture the current detail mirror as a canonical sparse snapshot.
    pub fn capture(
        structure: &VoxelStructure,
        state: &VoxelStructureState,
    ) -> Result<Self, VoxelSnapshotError> {
        validate_state_identity(structure, state)?;
        let components = VoxelCollapseAnalyzer::analyze(structure, state).map_err(|error| {
            VoxelSnapshotError::new(VoxelSnapshotErrorKind::State, error.to_string())
        })?;
        let mut changed_cells = Vec::new();
        let mut missing_cell_ids = Vec::new();
        for cell in state.cells() {
            if cell.health_milli != cell.max_health_milli || !cell.present {
                changed_cells.push(VoxelSnapshotCell {
                    id: cell.id.clone(),
                    coord: cell.coord,
                    material_id: cell.material_id.clone(),
                    health_milli: cell.health_milli,
                    present: cell.present,
                });
            }
            if !cell.present {
                missing_cell_ids.push(cell.id.clone());
            }
        }
        let revealed_opening_cells = revealed_openings(structure, state)?;
        let header = snapshot_header(structure, state)?;
        let lifecycle = snapshot_lifecycle(state);
        let snapshot = Self {
            header,
            changed_cells,
            missing_cell_ids,
            revealed_opening_cells,
            lifecycle,
            components,
            snapshot_hash_sha256: String::new(),
        };
        let mut snapshot = snapshot;
        snapshot.validate_sparse(structure)?;
        snapshot.snapshot_hash_sha256 = snapshot_hash_sha256(&snapshot);
        Ok(snapshot)
    }

    /// Validate structure identity, sparse canonical form, and the snapshot
    /// hash without mutating a state mirror.
    pub fn validate(&self, structure: &VoxelStructure) -> Result<(), VoxelSnapshotError> {
        self.validate_sparse(structure)?;
        let expected = snapshot_hash_sha256(self);
        if self.snapshot_hash_sha256 != expected {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Hash,
                "snapshot hash does not match canonical fields",
            ));
        }
        Ok(())
    }

    /// Restore a validated snapshot into a same-generation detail mirror.
    pub fn restore_into(
        &self,
        structure: &VoxelStructure,
        state: &mut VoxelStructureState,
    ) -> Result<(), VoxelSnapshotError> {
        self.validate(structure)?;
        validate_state_identity(structure, state)?;
        let overrides = override_tuples(&self.changed_cells);
        state.restore_snapshot_state(
            self.header.generation,
            self.header.tick,
            self.header.source_cursor,
            &overrides,
            self.lifecycle.destruction_started,
            self.lifecycle.possessed_owner,
            self.lifecycle.possession_payload,
            self.lifecycle.source_life,
            self.lifecycle.source_action,
            self.lifecycle.source_active,
            self.lifecycle.impact_position_um,
            self.lifecycle.damage_hit_count,
        )?;
        let replayed = Self::capture(structure, state)?;
        if replayed.snapshot_hash_sha256 != self.snapshot_hash_sha256 {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Hash,
                "restored state does not reproduce snapshot hash",
            ));
        }
        Ok(())
    }

    /// Reset the supplied state to a newer manifest generation and capture its
    /// empty/intact snapshot.  Older or equal generations are rejected.
    pub fn reset_generation(
        previous: &Self,
        structure: &VoxelStructure,
        state: &mut VoxelStructureState,
    ) -> Result<Self, VoxelSnapshotError> {
        previous.validate_header_only(structure)?;
        if structure.generation <= previous.header.generation {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Generation,
                "reset generation is not newer than snapshot generation",
            ));
        }
        state.reset_generation(structure)?;
        Self::capture(structure, state)
    }

    fn validate_header_only(&self, structure: &VoxelStructure) -> Result<(), VoxelSnapshotError> {
        validate_header_static(&self.header, structure)?;
        if self.header.generation >= structure.generation {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Generation,
                "previous snapshot generation is not older than reset target",
            ));
        }
        Ok(())
    }

    fn validate_sparse(&self, structure: &VoxelStructure) -> Result<(), VoxelSnapshotError> {
        validate_header_against_structure(&self.header, structure)?;
        validate_changed_cells(structure, &self.changed_cells, false)?;
        validate_missing_ids(structure, &self.changed_cells, &self.missing_cell_ids, true)?;
        validate_revealed_openings(structure, &self.revealed_opening_cells)?;
        validate_component_report(structure, &self.components)?;
        if self.changed_cells.len() > MAX_SNAPSHOT_CELLS
            || self.missing_cell_ids.len() > MAX_SNAPSHOT_CELLS
        {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Cap,
                "snapshot sparse cell count exceeds cap",
            ));
        }
        Ok(())
    }
}

impl VoxelSparseDelta {
    /// Compute a bounded ordered delta from one valid snapshot to another.
    pub fn between(
        structure: &VoxelStructure,
        from: &VoxelSnapshot,
        to: &VoxelSnapshot,
    ) -> Result<Self, VoxelSnapshotError> {
        from.validate(structure)?;
        to.validate(structure)?;
        validate_header_identity(&from.header, &to.header)?;
        if to.header.tick < from.header.tick {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::TickOrder,
                "delta target tick precedes source tick",
            ));
        }
        if to.header.tick == from.header.tick
            && to.header.source_cursor <= from.header.source_cursor
            && to.snapshot_hash_sha256 != from.snapshot_hash_sha256
        {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::CursorOrder,
                "changed snapshots require a newer tick or source cursor",
            ));
        }
        let from_cells = full_cells(structure, from)?;
        let to_cells = full_cells(structure, to)?;
        let changed_cells: Vec<_> = structure
            .cells
            .iter()
            .filter_map(|cell| {
                let before = from_cells.get(&cell.id)?;
                let after = to_cells.get(&cell.id)?;
                (before != after).then(|| after.clone())
            })
            .collect();
        if changed_cells.len() > MAX_SNAPSHOT_CELLS {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Cap,
                "sparse delta changed-cell count exceeds cap",
            ));
        }
        let delta = Self {
            version: VOXEL_SNAPSHOT_VERSION,
            from_snapshot_hash_sha256: from.snapshot_hash_sha256.clone(),
            to_snapshot_hash_sha256: to.snapshot_hash_sha256.clone(),
            from_tick: from.header.tick,
            from_cursor: from.header.source_cursor,
            target_header: to.header.clone(),
            changed_cells,
            missing_cell_ids: to.missing_cell_ids.clone(),
            revealed_opening_cells: to.revealed_opening_cells.clone(),
            lifecycle: to.lifecycle.clone(),
            components: to.components.clone(),
            delta_hash_sha256: String::new(),
        };
        let mut delta = delta;
        delta.delta_hash_sha256 = delta_hash_sha256(&delta);
        delta.validate(structure)?;
        Ok(delta)
    }

    /// Apply a delta in source order.  Reapplying the exact already-applied
    /// delta is an idempotent no-op; another base/hash or older cursor fails.
    pub fn apply_to(
        &self,
        structure: &VoxelStructure,
        base: &mut VoxelSnapshot,
    ) -> Result<VoxelDeltaApplyResult, VoxelSnapshotError> {
        self.validate(structure)?;
        base.validate(structure)?;
        if base.snapshot_hash_sha256 == self.to_snapshot_hash_sha256 {
            return Ok(VoxelDeltaApplyResult::AlreadyApplied);
        }
        if base.snapshot_hash_sha256 != self.from_snapshot_hash_sha256 {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Hash,
                "delta source hash does not match base snapshot",
            ));
        }
        if base.header.tick != self.from_tick || base.header.source_cursor != self.from_cursor {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::CursorOrder,
                "delta source cursor does not match base snapshot",
            ));
        }
        validate_cursor_advance(base.header.source_cursor, self.target_header.source_cursor)?;
        let mut target = base.clone();
        let mut full = full_cells(structure, base)?;
        for cell in &self.changed_cells {
            full.insert(cell.id.clone(), cell.clone());
        }
        target.header = self.target_header.clone();
        target.changed_cells = sparse_cells(structure, &full)?;
        target.missing_cell_ids = self.missing_cell_ids.clone();
        target.revealed_opening_cells = self.revealed_opening_cells.clone();
        target.lifecycle = self.lifecycle.clone();
        target.components = self.components.clone();
        target.snapshot_hash_sha256 = snapshot_hash_sha256(&target);
        if target.snapshot_hash_sha256 != self.to_snapshot_hash_sha256 {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Hash,
                "delta result does not reproduce target snapshot hash",
            ));
        }
        target.validate(structure)?;
        *base = target;
        Ok(VoxelDeltaApplyResult::Applied)
    }

    /// Validate version, source/target identity, canonical sparse fields, and
    /// the delta hash before applying it.
    pub fn validate(&self, structure: &VoxelStructure) -> Result<(), VoxelSnapshotError> {
        if self.version != VOXEL_SNAPSHOT_VERSION {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Version,
                "unsupported sparse delta version",
            ));
        }
        validate_header_against_structure(&self.target_header, structure)?;
        if self
            .from_cursor
            .is_some_and(|cursor| cursor.tick > self.from_tick)
        {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::CursorOrder,
                "delta source cursor tick exceeds source tick",
            ));
        }
        if self.target_header.tick < self.from_tick {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::TickOrder,
                "delta target tick precedes source tick",
            ));
        }
        validate_cursor_advance(self.from_cursor, self.target_header.source_cursor)?;
        validate_changed_cells(structure, &self.changed_cells, true)?;
        validate_missing_ids(
            structure,
            &self.changed_cells,
            &self.missing_cell_ids,
            false,
        )?;
        validate_revealed_openings(structure, &self.revealed_opening_cells)?;
        validate_component_report(structure, &self.components)?;
        if !is_sha256(&self.from_snapshot_hash_sha256)
            || !is_sha256(&self.to_snapshot_hash_sha256)
            || !is_sha256(&self.delta_hash_sha256)
        {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Hash,
                "delta hash fields must be lowercase SHA-256 strings",
            ));
        }
        let expected = delta_hash_sha256(self);
        if self.delta_hash_sha256 != expected {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Hash,
                "delta hash does not match canonical fields",
            ));
        }
        Ok(())
    }
}

fn validate_state_identity(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
) -> Result<(), VoxelSnapshotError> {
    if state.source_entity_id() != structure.source_entity_id
        || state.profile_id() != structure.structure_id
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Identity,
            "state identity does not match structure",
        ));
    }
    if state.generation() != structure.generation {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Generation,
            "state generation does not match structure",
        ));
    }
    if state.cells().len() != structure.cells.len()
        || structure
            .cells
            .iter()
            .zip(state.cells())
            .any(|(manifest, mirror)| {
                manifest.id != mirror.id
                    || manifest.coord != mirror.coord
                    || manifest.material_id != mirror.material_id
            })
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Corrupt,
            "state cells do not match structure",
        ));
    }
    Ok(())
}

fn snapshot_header(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
) -> Result<VoxelSnapshotHeader, VoxelSnapshotError> {
    Ok(VoxelSnapshotHeader {
        version: VOXEL_SNAPSHOT_VERSION,
        generation: state.generation(),
        tick: state.tick(),
        source_cursor: state.source_cursor(),
        source_entity_id: structure.source_entity_id.clone(),
        profile_id: structure.structure_id.clone(),
        manifest_id: structure.manifest_id.clone(),
        manifest_sha256: structure.semantic_sha256.clone(),
        recipe_sha256: recipe_sha256(structure)?,
        schema_sha256: structure.schema_sha256.clone(),
    })
}

fn recipe_sha256(structure: &VoxelStructure) -> Result<String, VoxelSnapshotError> {
    let document = structure.semantic_document();
    let value = document
        .get("hashes")
        .and_then(|hashes| hashes.get("recipe_sha256"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Identity,
                "manifest does not expose recipe_sha256",
            )
        })?;
    if !is_sha256(value) {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Identity,
            "manifest recipe_sha256 is not canonical",
        ));
    }
    Ok(value.to_owned())
}

fn validate_header_against_structure(
    header: &VoxelSnapshotHeader,
    structure: &VoxelStructure,
) -> Result<(), VoxelSnapshotError> {
    if header.version != VOXEL_SNAPSHOT_VERSION {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Version,
            "unsupported snapshot version",
        ));
    }
    validate_header_static(header, structure)?;
    if header.generation != structure.generation {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Generation,
            "snapshot generation does not match structure",
        ));
    }
    Ok(())
}

fn validate_header_static(
    header: &VoxelSnapshotHeader,
    structure: &VoxelStructure,
) -> Result<(), VoxelSnapshotError> {
    if header.version != VOXEL_SNAPSHOT_VERSION {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Version,
            "unsupported snapshot version",
        ));
    }
    if header.source_entity_id != structure.source_entity_id
        || header.profile_id != structure.structure_id
        || header.manifest_id != structure.manifest_id
        || header.manifest_sha256 != structure.semantic_sha256
        || header.schema_sha256 != structure.schema_sha256
        || header.recipe_sha256 != recipe_sha256(structure)?
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Identity,
            "snapshot header identity/hash does not match structure",
        ));
    }
    if header
        .source_cursor
        .is_some_and(|cursor| cursor.tick > header.tick)
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::CursorOrder,
            "snapshot source cursor tick exceeds header tick",
        ));
    }
    Ok(())
}

fn validate_header_identity(
    left: &VoxelSnapshotHeader,
    right: &VoxelSnapshotHeader,
) -> Result<(), VoxelSnapshotError> {
    if left.version != right.version
        || left.generation != right.generation
        || left.source_entity_id != right.source_entity_id
        || left.profile_id != right.profile_id
        || left.manifest_id != right.manifest_id
        || left.manifest_sha256 != right.manifest_sha256
        || left.recipe_sha256 != right.recipe_sha256
        || left.schema_sha256 != right.schema_sha256
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Identity,
            "snapshot headers do not share one structure identity",
        ));
    }
    Ok(())
}

fn validate_changed_cells(
    structure: &VoxelStructure,
    changed_cells: &[VoxelSnapshotCell],
    allow_intact: bool,
) -> Result<(), VoxelSnapshotError> {
    if changed_cells.len() > MAX_SNAPSHOT_CELLS {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Cap,
            "changed-cell list exceeds cap",
        ));
    }
    if changed_cells
        .windows(2)
        .any(|window| window[0].id >= window[1].id)
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::InvalidSparseState,
            "changed cells must be sorted and unique",
        ));
    }
    let material_health: HashMap<&str, u32> = structure
        .materials
        .iter()
        .map(|material| (material.id.as_str(), material.health_milli))
        .collect();
    for cell in changed_cells {
        let Some(manifest_cell) = structure.cell(&cell.id) else {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::UnknownCell,
                format!("changed cell {} is not in structure", cell.id),
            ));
        };
        if manifest_cell.coord != cell.coord || manifest_cell.material_id != cell.material_id {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::Corrupt,
                format!("changed cell {} metadata differs from structure", cell.id),
            ));
        }
        let Some(&max_health) = material_health.get(cell.material_id.as_str()) else {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::UnknownCell,
                format!("changed cell {} material is not in structure", cell.id),
            ));
        };
        if cell.health_milli > max_health
            || (cell.present && cell.health_milli == 0)
            || (!cell.present && cell.health_milli != 0)
            || (!allow_intact && cell.present && cell.health_milli == max_health)
        {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::InvalidSparseState,
                format!("changed cell {} health/presence is not canonical", cell.id),
            ));
        }
    }
    Ok(())
}

fn validate_missing_ids(
    structure: &VoxelStructure,
    changed_cells: &[VoxelSnapshotCell],
    missing_cell_ids: &[String],
    require_exact: bool,
) -> Result<(), VoxelSnapshotError> {
    if missing_cell_ids.len() > MAX_SNAPSHOT_CELLS
        || missing_cell_ids
            .windows(2)
            .any(|window| window[0] >= window[1])
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::InvalidSparseState,
            "missing cell IDs must be sorted, unique, and bounded",
        ));
    }
    let missing: HashSet<&str> = missing_cell_ids.iter().map(String::as_str).collect();
    let changed_missing: HashSet<&str> = changed_cells
        .iter()
        .filter(|cell| !cell.present)
        .map(|cell| cell.id.as_str())
        .collect();
    if !changed_missing.is_subset(&missing)
        || (require_exact && missing.len() != changed_missing.len())
        || (require_exact && missing != changed_missing)
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::InvalidSparseState,
            "missing IDs do not match sparse cell presence",
        ));
    }
    for id in missing_cell_ids {
        if structure.cell(id).is_none() {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::UnknownCell,
                format!("missing cell {id} is not in structure"),
            ));
        }
    }
    Ok(())
}

fn validate_revealed_openings(
    structure: &VoxelStructure,
    revealed: &[CellCoord],
) -> Result<(), VoxelSnapshotError> {
    if revealed.len() > MAX_REVEALED_OPENINGS
        || revealed.windows(2).any(|window| window[0] >= window[1])
        || revealed
            .iter()
            .any(|coord| structure.opening_cells.binary_search(coord).is_err())
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::InvalidSparseState,
            "revealed openings are not a sorted subset of manifest openings",
        ));
    }
    Ok(())
}

fn validate_component_report(
    structure: &VoxelStructure,
    report: &VoxelCollapseReport,
) -> Result<(), VoxelSnapshotError> {
    if report.source_entity_id != structure.source_entity_id
        || report.profile_id != structure.structure_id
        || report.generation != structure.generation
        || report.components.len() > super::voxel_collapse::MAX_MAJOR_CLUSTERS
        || report.component_count < report.components.len()
        || report.omitted_component_count
            != report
                .component_count
                .saturating_sub(report.components.len())
        || !is_sha256(&report.report_hash_sha256)
    {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Corrupt,
            "component report identity or cap is invalid",
        ));
    }
    Ok(())
}

fn override_tuples(changed_cells: &[VoxelSnapshotCell]) -> Vec<(String, u32, bool)> {
    changed_cells
        .iter()
        .map(|cell| (cell.id.clone(), cell.health_milli, cell.present))
        .collect()
}

fn full_cells(
    structure: &VoxelStructure,
    snapshot: &VoxelSnapshot,
) -> Result<BTreeMap<String, VoxelSnapshotCell>, VoxelSnapshotError> {
    snapshot.validate_sparse(structure)?;
    let material_health: HashMap<&str, u32> = structure
        .materials
        .iter()
        .map(|material| (material.id.as_str(), material.health_milli))
        .collect();
    let changed: HashMap<&str, &VoxelSnapshotCell> = snapshot
        .changed_cells
        .iter()
        .map(|cell| (cell.id.as_str(), cell))
        .collect();
    let mut cells = BTreeMap::new();
    for manifest_cell in &structure.cells {
        let max_health = *material_health
            .get(manifest_cell.material_id.as_str())
            .ok_or_else(|| {
                VoxelSnapshotError::new(
                    VoxelSnapshotErrorKind::UnknownCell,
                    format!("manifest cell {} material is unknown", manifest_cell.id),
                )
            })?;
        let cell = changed
            .get(manifest_cell.id.as_str())
            .map(|cell| (*cell).clone())
            .unwrap_or_else(|| VoxelSnapshotCell {
                id: manifest_cell.id.clone(),
                coord: manifest_cell.coord,
                material_id: manifest_cell.material_id.clone(),
                health_milli: max_health,
                present: true,
            });
        cells.insert(manifest_cell.id.clone(), cell);
    }
    Ok(cells)
}

fn sparse_cells(
    structure: &VoxelStructure,
    full: &BTreeMap<String, VoxelSnapshotCell>,
) -> Result<Vec<VoxelSnapshotCell>, VoxelSnapshotError> {
    let mut changed = Vec::new();
    for manifest_cell in &structure.cells {
        let Some(cell) = full.get(&manifest_cell.id) else {
            return Err(VoxelSnapshotError::new(
                VoxelSnapshotErrorKind::UnknownCell,
                format!("sparse state omits manifest cell {}", manifest_cell.id),
            ));
        };
        let max_health = structure
            .materials
            .iter()
            .find(|material| material.id == manifest_cell.material_id)
            .map(|material| material.health_milli)
            .ok_or_else(|| {
                VoxelSnapshotError::new(
                    VoxelSnapshotErrorKind::UnknownCell,
                    format!("manifest cell {} material is unknown", manifest_cell.id),
                )
            })?;
        if cell.health_milli != max_health || !cell.present {
            changed.push(cell.clone());
        }
    }
    if changed.len() > MAX_SNAPSHOT_CELLS {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Cap,
            "sparse state exceeds changed-cell cap",
        ));
    }
    Ok(changed)
}

fn revealed_openings(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
) -> Result<Vec<CellCoord>, VoxelSnapshotError> {
    let mut revealed = Vec::new();
    for &opening in &structure.opening_cells {
        let is_revealed = neighbors(opening).into_iter().flatten().any(|neighbor| {
            structure
                .cell_id_at(neighbor)
                .and_then(|id| state.cell(id))
                .is_some_and(|cell| !cell.present)
        });
        if is_revealed {
            revealed.push(opening);
        }
    }
    if revealed.len() > MAX_REVEALED_OPENINGS {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::Cap,
            "revealed opening count exceeds cap",
        ));
    }
    Ok(revealed)
}

fn snapshot_lifecycle(state: &VoxelStructureState) -> VoxelSnapshotLifecycle {
    VoxelSnapshotLifecycle {
        destruction_started: state.destruction_started(),
        possessed_owner: state.possessed_owner(),
        possession_payload: state.possession_payload(),
        source_life: state.source_life(),
        source_action: state.source_action(),
        source_active: state.source_active(),
        impact_position_um: state.impact_position_um(),
        damage_hit_count: state.damage_hit_count(),
    }
}

fn validate_cursor_advance(
    from: Option<VoxelSourceCursor>,
    to: Option<VoxelSourceCursor>,
) -> Result<(), VoxelSnapshotError> {
    if to < from {
        return Err(VoxelSnapshotError::new(
            VoxelSnapshotErrorKind::CursorOrder,
            "target cursor precedes source cursor",
        ));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
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

fn snapshot_hash_sha256(snapshot: &VoxelSnapshot) -> String {
    let mut hasher = Sha256::new();
    hash_header(&mut hasher, &snapshot.header);
    hash_cells(&mut hasher, &snapshot.changed_cells);
    hash_strings(&mut hasher, &snapshot.missing_cell_ids);
    hash_coords(&mut hasher, &snapshot.revealed_opening_cells);
    hash_lifecycle(&mut hasher, &snapshot.lifecycle);
    hash_component_report(&mut hasher, &snapshot.components);
    format!("{:x}", hasher.finalize())
}

fn delta_hash_sha256(delta: &VoxelSparseDelta) -> String {
    let mut hasher = Sha256::new();
    hasher.update(delta.version.to_le_bytes());
    hash_string(&mut hasher, &delta.from_snapshot_hash_sha256);
    hash_string(&mut hasher, &delta.to_snapshot_hash_sha256);
    hasher.update(delta.from_tick.to_le_bytes());
    hash_cursor(&mut hasher, delta.from_cursor);
    hash_header(&mut hasher, &delta.target_header);
    hash_cells(&mut hasher, &delta.changed_cells);
    hash_strings(&mut hasher, &delta.missing_cell_ids);
    hash_coords(&mut hasher, &delta.revealed_opening_cells);
    hash_lifecycle(&mut hasher, &delta.lifecycle);
    hash_component_report(&mut hasher, &delta.components);
    format!("{:x}", hasher.finalize())
}

fn hash_header(hasher: &mut Sha256, header: &VoxelSnapshotHeader) {
    hasher.update(header.version.to_le_bytes());
    hasher.update(header.generation.to_le_bytes());
    hasher.update(header.tick.to_le_bytes());
    hash_cursor(hasher, header.source_cursor);
    hash_string(hasher, &header.source_entity_id);
    hash_string(hasher, &header.profile_id);
    hash_string(hasher, &header.manifest_id);
    hash_string(hasher, &header.manifest_sha256);
    hash_string(hasher, &header.recipe_sha256);
    hash_string(hasher, &header.schema_sha256);
}

fn hash_cells(hasher: &mut Sha256, cells: &[VoxelSnapshotCell]) {
    hasher.update((cells.len() as u64).to_le_bytes());
    for cell in cells {
        hash_string(hasher, &cell.id);
        for axis in cell.coord {
            hasher.update(axis.to_le_bytes());
        }
        hash_string(hasher, &cell.material_id);
        hasher.update(cell.health_milli.to_le_bytes());
        hasher.update([u8::from(cell.present)]);
    }
}

fn hash_strings(hasher: &mut Sha256, values: &[String]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hash_string(hasher, value);
    }
}

fn hash_coords(hasher: &mut Sha256, values: &[CellCoord]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        for axis in *value {
            hasher.update(axis.to_le_bytes());
        }
    }
}

fn hash_cursor(hasher: &mut Sha256, cursor: Option<VoxelSourceCursor>) {
    match cursor {
        Some(cursor) => {
            hasher.update([1]);
            hasher.update(cursor.tick.to_le_bytes());
            hasher.update(cursor.sequence.to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_lifecycle(hasher: &mut Sha256, lifecycle: &VoxelSnapshotLifecycle) {
    hasher.update([u8::from(lifecycle.destruction_started)]);
    match lifecycle.possessed_owner {
        Some(owner) => {
            hasher.update([1]);
            hasher.update(owner.to_le_bytes());
        }
        None => hasher.update([0]),
    }
    hasher.update(lifecycle.possession_payload.to_le_bytes());
    hasher.update(lifecycle.source_life.to_le_bytes());
    hasher.update([lifecycle.source_action, u8::from(lifecycle.source_active)]);
    hash_position(hasher, lifecycle.impact_position_um);
    hasher.update(lifecycle.damage_hit_count.to_le_bytes());
}

fn hash_position(hasher: &mut Sha256, position: Option<[i64; 3]>) {
    match position {
        Some(position) => {
            hasher.update([1]);
            for axis in position {
                hasher.update(axis.to_le_bytes());
            }
        }
        None => hasher.update([0]),
    }
}

fn hash_component_report(hasher: &mut Sha256, report: &VoxelCollapseReport) {
    hash_string(hasher, &report.source_entity_id);
    hash_string(hasher, &report.profile_id);
    hasher.update(report.generation.to_le_bytes());
    hasher.update((report.component_count as u64).to_le_bytes());
    hasher.update((report.omitted_component_count as u64).to_le_bytes());
    hasher.update([
        u8::from(report.coarse_fracture),
        collapse_decision_tag(report.decision),
    ]);
    hash_string(hasher, &report.report_hash_sha256);
    hasher.update((report.components.len() as u64).to_le_bytes());
    for component in &report.components {
        hash_string(hasher, &component.component_id);
        hash_strings(hasher, &component.cell_ids);
        hash_strings(hasher, &component.foundation_cell_ids);
        hash_range(hasher, component.support_footprint_x_um);
        hash_range(hasher, component.support_footprint_z_um);
        hasher.update(component.center_of_mass.mass_microkg.to_le_bytes());
        for axis in component.center_of_mass.position_um {
            hasher.update(axis.to_le_bytes());
        }
        for inertia in component.inertia_diag_um2_microkg {
            hasher.update(inertia.to_le_bytes());
        }
        hasher.update([component_classification_tag(component.classification)]);
    }
}

fn hash_range(hasher: &mut Sha256, range: Option<[i64; 2]>) {
    match range {
        Some(range) => {
            hasher.update([1]);
            hasher.update(range[0].to_le_bytes());
            hasher.update(range[1].to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

const fn collapse_decision_tag(decision: super::voxel_collapse::VoxelCollapseDecision) -> u8 {
    match decision {
        super::voxel_collapse::VoxelCollapseDecision::Standing => 0,
        super::voxel_collapse::VoxelCollapseDecision::ShedDetached => 1,
        super::voxel_collapse::VoxelCollapseDecision::Topple => 2,
        super::voxel_collapse::VoxelCollapseDecision::Collapse => 3,
        super::voxel_collapse::VoxelCollapseDecision::CoarseFracture => 4,
    }
}

const fn component_classification_tag(
    classification: super::voxel_collapse::VoxelComponentClassification,
) -> u8 {
    match classification {
        super::voxel_collapse::VoxelComponentClassification::Supported => 0,
        super::voxel_collapse::VoxelComponentClassification::Detached => 1,
        super::voxel_collapse::VoxelComponentClassification::Topple => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::{voxel_state::VoxelSourceEvent, voxel_structure::load_manifest};

    const SOURCE_ID: &str = "synthetic-house-0";
    const PROFILE_ID: &str = "synthetic-pointy-house";
    const REMOVED_CELL: &str = "cell-n1-p1-n1";

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn fixture_structure() -> VoxelStructure {
        let root = fixture_root();
        let manifest =
            fs::read(root.join(
                "generated/voxel-house-lookdev/contract/fixtures/golden-synthetic-house.json",
            ))
            .unwrap();
        let schema = fs::read(
            root.join("generated/voxel-house-lookdev/contract/voxel-house-manifest-v1.schema.json"),
        )
        .unwrap();
        load_manifest(&manifest, &schema).unwrap()
    }

    fn intact_state(structure: &VoxelStructure) -> VoxelStructureState {
        VoxelStructureState::new(structure).unwrap()
    }

    fn damage_event(structure: &VoxelStructure, tick: u64, damage_milli: u32) -> VoxelSourceEvent {
        VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            structure.generation,
            VoxelSourceCursor::new(tick, tick as u32),
            vec![REMOVED_CELL.to_owned()],
            damage_milli,
        )
        .with_source_metadata(33, 52, true, Some([-750_000, 750_000, 0]))
    }

    fn damaged_snapshot(
        structure: &VoxelStructure,
        tick: u64,
        damage_milli: u32,
    ) -> (VoxelStructureState, VoxelSnapshot) {
        let mut state = intact_state(structure);
        state
            .apply_source_event(&damage_event(structure, tick, damage_milli))
            .unwrap();
        let snapshot = VoxelSnapshot::capture(structure, &state).unwrap();
        (state, snapshot)
    }

    #[test]
    fn capture_round_trip_restores_sparse_lifecycle_and_opening_reveal() {
        let structure = fixture_structure();
        let (state, snapshot) = damaged_snapshot(&structure, 5, 100_000);
        assert_eq!(snapshot.header.version, VOXEL_SNAPSHOT_VERSION);
        assert_eq!(snapshot.header.tick, 5);
        assert_eq!(
            snapshot.header.source_cursor,
            Some(VoxelSourceCursor::new(5, 5))
        );
        assert_eq!(snapshot.changed_cells.len(), 1);
        assert_eq!(snapshot.missing_cell_ids, vec![REMOVED_CELL.to_owned()]);
        assert!(snapshot.revealed_opening_cells.contains(&[-1, 1, 0]));
        assert_eq!(snapshot.lifecycle.source_action, 52);
        assert_eq!(snapshot.snapshot_hash_sha256.len(), 64);

        let mut restored = intact_state(&structure);
        snapshot.restore_into(&structure, &mut restored).unwrap();
        let replayed = VoxelSnapshot::capture(&structure, &restored).unwrap();
        assert_eq!(replayed, snapshot);
        assert!(!restored.cell(REMOVED_CELL).unwrap().present);
        assert_eq!(state.cell(REMOVED_CELL).unwrap().health_milli, 0);
    }

    #[test]
    fn sparse_delta_round_trip_is_ordered_and_idempotent() {
        let structure = fixture_structure();
        let base = VoxelSnapshot::capture(&structure, &intact_state(&structure)).unwrap();
        let (_state, target) = damaged_snapshot(&structure, 1, 10_000);
        let delta = VoxelSparseDelta::between(&structure, &base, &target).unwrap();
        assert_eq!(delta.changed_cells.len(), 1);
        assert_eq!(delta.from_tick, 0);
        assert_eq!(delta.target_header.tick, 1);

        let mut applied = base.clone();
        assert_eq!(
            delta.apply_to(&structure, &mut applied).unwrap(),
            VoxelDeltaApplyResult::Applied
        );
        assert_eq!(applied, target);
        assert_eq!(
            delta.apply_to(&structure, &mut applied).unwrap(),
            VoxelDeltaApplyResult::AlreadyApplied
        );
    }

    #[test]
    fn reset_generation_clears_sparse_state_and_rejects_stale_reset() {
        let structure = fixture_structure();
        let mut state = damaged_snapshot(&structure, 1, 100_000).0;
        let previous = VoxelSnapshot::capture(&structure, &state).unwrap();
        let mut next = structure.clone();
        next.generation = structure.generation + 1;
        let reset = VoxelSnapshot::reset_generation(&previous, &next, &mut state).unwrap();
        assert_eq!(reset.header.generation, next.generation);
        assert_eq!(reset.header.tick, 0);
        assert!(reset.changed_cells.is_empty());
        assert!(reset.missing_cell_ids.is_empty());
        assert_eq!(state.generation(), next.generation);
        assert!(state.cells().iter().all(|cell| cell.present));

        let mut stale_state = intact_state(&structure);
        let error =
            VoxelSnapshot::reset_generation(&previous, &structure, &mut stale_state).unwrap_err();
        assert_eq!(error.kind, VoxelSnapshotErrorKind::Generation);
    }

    #[test]
    fn wrong_generation_and_identity_are_rejected() {
        let structure = fixture_structure();
        let snapshot = VoxelSnapshot::capture(&structure, &intact_state(&structure)).unwrap();
        let mut newer = structure.clone();
        newer.generation += 1;
        assert_eq!(
            snapshot.validate(&newer).unwrap_err().kind,
            VoxelSnapshotErrorKind::Generation
        );
        let mut wrong_profile = structure.clone();
        wrong_profile.structure_id = "other-profile".to_owned();
        assert_eq!(
            snapshot.validate(&wrong_profile).unwrap_err().kind,
            VoxelSnapshotErrorKind::Identity
        );
    }

    #[test]
    fn corrupt_snapshot_hash_and_header_fail_closed() {
        let structure = fixture_structure();
        let snapshot = VoxelSnapshot::capture(&structure, &intact_state(&structure)).unwrap();
        let mut corrupt_hash = snapshot.clone();
        corrupt_hash.snapshot_hash_sha256 = "0".repeat(64);
        assert_eq!(
            corrupt_hash.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::Hash
        );
        let mut corrupt_header = snapshot;
        corrupt_header.header.recipe_sha256 = "f".repeat(64);
        assert_eq!(
            corrupt_header.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::Identity
        );
    }

    #[test]
    fn sparse_caps_and_canonical_presence_are_enforced() {
        let structure = fixture_structure();
        let snapshot = VoxelSnapshot::capture(&structure, &intact_state(&structure)).unwrap();
        let mut capped = snapshot.clone();
        capped.changed_cells = (0..=MAX_SNAPSHOT_CELLS)
            .map(|_| VoxelSnapshotCell {
                id: REMOVED_CELL.to_owned(),
                coord: [-1, 1, -1],
                material_id: "material-masonry".to_owned(),
                health_milli: 0,
                present: false,
            })
            .collect();
        assert_eq!(
            capped.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::Cap
        );

        let (_state, changed) = damaged_snapshot(&structure, 1, 10_000);
        let mut invalid = changed;
        invalid.changed_cells[0].health_milli = 100_000;
        invalid.changed_cells[0].present = true;
        assert_eq!(
            invalid.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::InvalidSparseState
        );

        let mut unknown = snapshot;
        unknown.changed_cells = vec![VoxelSnapshotCell {
            id: "cell-unknown".to_owned(),
            coord: [0, 0, 0],
            material_id: "material-masonry".to_owned(),
            health_milli: 0,
            present: false,
        }];
        assert_eq!(
            unknown.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::UnknownCell
        );
    }

    #[test]
    fn delta_hash_and_wrong_base_are_rejected() {
        let structure = fixture_structure();
        let base = VoxelSnapshot::capture(&structure, &intact_state(&structure)).unwrap();
        let (_state, target) = damaged_snapshot(&structure, 1, 10_000);
        let delta = VoxelSparseDelta::between(&structure, &base, &target).unwrap();
        let mut corrupt_hash = delta.clone();
        corrupt_hash.delta_hash_sha256 = "0".repeat(64);
        assert_eq!(
            corrupt_hash.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::Hash
        );

        let mut corrupt = delta.clone();
        corrupt.from_tick = 99;
        assert_eq!(
            corrupt.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::TickOrder
        );

        let mut stale_cursor = delta.clone();
        stale_cursor.from_cursor = Some(VoxelSourceCursor::new(9, 9));
        stale_cursor.delta_hash_sha256 = delta_hash_sha256(&stale_cursor);
        assert_eq!(
            stale_cursor.validate(&structure).unwrap_err().kind,
            VoxelSnapshotErrorKind::CursorOrder
        );

        let later = {
            let mut state = intact_state(&structure);
            state.advance_tick(structure.generation, 2).unwrap();
            VoxelSnapshot::capture(&structure, &state).unwrap()
        };
        let mut wrong_base = later;
        assert_eq!(
            delta
                .apply_to(&structure, &mut wrong_base)
                .unwrap_err()
                .kind,
            VoxelSnapshotErrorKind::Hash
        );
    }

    #[test]
    fn deterministic_replay_hash_repeats_for_ten_thousand_captures() {
        let structure = fixture_structure();
        let (state, expected) = damaged_snapshot(&structure, 3, 10_000);
        for _ in 0..10_000 {
            let replayed = VoxelSnapshot::capture(&structure, &state).unwrap();
            assert_eq!(replayed.snapshot_hash_sha256, expected.snapshot_hash_sha256);
        }
    }
}
