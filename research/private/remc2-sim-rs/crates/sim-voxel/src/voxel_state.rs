//! Deterministic private VoxelMode detail state.
//!
//! `VoxelStructure` is immutable contract data.  This module owns the small
//! mutable mirror derived from it: per-cell health/presence and source-event
//! metadata.  The source `HouseState` remains authoritative for gameplay;
//! this state never writes back to it and has no knowledge of Unity, C ABI,
//! fireball, physics, or Track S systems.

#![allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]

use std::{collections::HashMap, fmt};

use super::voxel_structure::{CellCoord, VoxelStructure};

/// Maximum source events accepted in one deterministic mirror batch.
pub const MAX_SOURCE_EVENTS_PER_BATCH: usize = 64;
/// Maximum cell IDs carried by one source event.  This equals the contract's
/// maximum cell budget and prevents unbounded target lists.
pub const MAX_TARGET_CELLS_PER_EVENT: usize = 4_096;

/// Event kinds emitted by the source lifecycle mirror.  These are metadata
/// inputs only; they do not alter source gameplay state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VoxelSourceEventKind {
    Damaged,
    DestructionStarted,
    RemoveFootprint,
    Possessed,
}

/// Monotonic source event position within one manifest generation.
///
/// `ordinal` is a source-provided tie-breaker for events sharing a tick.  A
/// new tick may start at any ordinal; within one tick ordinals must increase.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VoxelSourceCursor {
    pub tick: u64,
    pub sequence: u32,
}

impl VoxelSourceCursor {
    #[must_use]
    pub const fn new(tick: u64, sequence: u32) -> Self {
        Self { tick, sequence }
    }
}

/// A bounded, source-neutral lifecycle input.
///
/// For `Damaged`, `target_cell_ids` must contain one or more sorted unique
/// IDs and `damage_milli` must be positive.  For `DestructionStarted` and
/// `RemoveFootprint`, an empty target list means the complete footprint;
/// otherwise it must be sorted and unique.  `Possessed` carries owner
/// metadata and does not require targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelSourceEvent {
    /// Source entity identity from the gameplay stream.
    pub source_entity_id: String,
    /// Source VoxelMode profile identity (`VoxelStructure::structure_id`).
    pub profile_id: String,
    pub generation: u32,
    pub cursor: VoxelSourceCursor,
    pub kind: VoxelSourceEventKind,
    pub target_cell_ids: Vec<String>,
    pub damage_milli: u32,
    pub owner_entity_index: u16,
    pub payload: u32,
    /// Source lifecycle metadata copied alongside the event.
    pub source_life: i32,
    pub source_action: u8,
    pub source_active: bool,
    /// Optional authoritative impact position in micrometres in source world
    /// space.  It is metadata for later damage mapping, not a gameplay query.
    pub impact_position_um: Option<[i64; 3]>,
    /// Optional attacker position used only for deterministic facade fallback
    /// when no impact position was emitted by the source.
    pub attacker_position_um: Option<[i64; 3]>,
}

impl VoxelSourceEvent {
    fn base(
        source_entity_id: impl Into<String>,
        profile_id: impl Into<String>,
        generation: u32,
        cursor: VoxelSourceCursor,
        kind: VoxelSourceEventKind,
        target_cell_ids: Vec<String>,
    ) -> Self {
        Self {
            source_entity_id: source_entity_id.into(),
            profile_id: profile_id.into(),
            generation,
            cursor,
            kind,
            target_cell_ids,
            damage_milli: 0,
            owner_entity_index: 0,
            payload: 0,
            source_life: 0,
            source_action: 0,
            source_active: true,
            impact_position_um: None,
            attacker_position_um: None,
        }
    }

    #[must_use]
    pub fn damaged(
        source_entity_id: impl Into<String>,
        profile_id: impl Into<String>,
        generation: u32,
        cursor: VoxelSourceCursor,
        target_cell_ids: Vec<String>,
        damage_milli: u32,
    ) -> Self {
        let mut event = Self::base(
            source_entity_id,
            profile_id,
            generation,
            cursor,
            VoxelSourceEventKind::Damaged,
            target_cell_ids,
        );
        event.damage_milli = damage_milli;
        event
    }

    #[must_use]
    pub fn destruction_started(
        source_entity_id: impl Into<String>,
        profile_id: impl Into<String>,
        generation: u32,
        cursor: VoxelSourceCursor,
        target_cell_ids: Vec<String>,
    ) -> Self {
        Self::base(
            source_entity_id,
            profile_id,
            generation,
            cursor,
            VoxelSourceEventKind::DestructionStarted,
            target_cell_ids,
        )
    }

    #[must_use]
    pub fn remove_footprint(
        source_entity_id: impl Into<String>,
        profile_id: impl Into<String>,
        generation: u32,
        cursor: VoxelSourceCursor,
        target_cell_ids: Vec<String>,
    ) -> Self {
        Self::base(
            source_entity_id,
            profile_id,
            generation,
            cursor,
            VoxelSourceEventKind::RemoveFootprint,
            target_cell_ids,
        )
    }

    #[must_use]
    pub fn possessed(
        source_entity_id: impl Into<String>,
        profile_id: impl Into<String>,
        generation: u32,
        cursor: VoxelSourceCursor,
        owner_entity_index: u16,
        payload: u32,
    ) -> Self {
        let mut event = Self::base(
            source_entity_id,
            profile_id,
            generation,
            cursor,
            VoxelSourceEventKind::Possessed,
            Vec::new(),
        );
        event.owner_entity_index = owner_entity_index;
        event.payload = payload;
        event
    }

    /// Attach source life/action/active and optional impact metadata.
    #[must_use]
    pub fn with_source_metadata(
        mut self,
        source_life: i32,
        source_action: u8,
        source_active: bool,
        impact_position_um: Option<[i64; 3]>,
    ) -> Self {
        self.source_life = source_life;
        self.source_action = source_action;
        self.source_active = source_active;
        self.impact_position_um = impact_position_um;
        self
    }

    #[must_use]
    pub fn with_attacker_position_um(mut self, attacker_position_um: Option<[i64; 3]>) -> Self {
        self.attacker_position_um = attacker_position_um;
        self
    }
}

/// Fixed-point mutable state for one validated structural cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelCellState {
    pub id: String,
    pub coord: CellCoord,
    pub material_id: String,
    pub support_ids: Vec<String>,
    pub max_health_milli: u32,
    pub health_milli: u32,
    pub present: bool,
}

impl VoxelCellState {
    #[must_use]
    pub fn intact(
        id: String,
        coord: CellCoord,
        material_id: String,
        support_ids: Vec<String>,
        max_health_milli: u32,
    ) -> Self {
        Self {
            id,
            coord,
            material_id,
            support_ids,
            max_health_milli,
            health_milli: max_health_milli,
            present: true,
        }
    }
}

/// Stable error category for bounded source-event ingestion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelStateErrorKind {
    Cap,
    WrongIdentity,
    WrongGeneration,
    Stale,
    Duplicate,
    OutOfOrder,
    UnknownCell,
    InvalidEvent,
    TickRegression,
}

impl VoxelStateErrorKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Cap => "cap",
            Self::WrongIdentity => "wrong-identity",
            Self::WrongGeneration => "wrong-generation",
            Self::Stale => "stale",
            Self::Duplicate => "duplicate",
            Self::OutOfOrder => "out-of-order",
            Self::UnknownCell => "unknown-cell",
            Self::InvalidEvent => "invalid-event",
            Self::TickRegression => "tick-regression",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelStateError {
    pub kind: VoxelStateErrorKind,
    pub message: String,
}

impl VoxelStateError {
    fn new(kind: VoxelStateErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for VoxelStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.label(), self.message)
    }
}

impl std::error::Error for VoxelStateError {}

/// Mutable deterministic detail mirror for one validated `VoxelStructure`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelStructureState {
    source_entity_id: String,
    profile_id: String,
    generation: u32,
    tick: u64,
    source_cursor: Option<VoxelSourceCursor>,
    cells: Vec<VoxelCellState>,
    cell_indexes: HashMap<String, usize>,
    destruction_started: bool,
    possessed_owner: Option<u16>,
    possession_payload: u32,
    source_life: i32,
    source_action: u8,
    source_active: bool,
    impact_position_um: Option<[i64; 3]>,
    damage_hit_count: u32,
}

impl VoxelStructureState {
    /// Construct an intact state in the manifest's stable cell order.
    pub fn new(structure: &VoxelStructure) -> Result<Self, VoxelStateError> {
        if structure.cells.len() > MAX_TARGET_CELLS_PER_EVENT {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::Cap,
                "manifest cell count exceeds detail-state cap",
            ));
        }
        let mut material_health = HashMap::with_capacity(structure.materials.len());
        for material in &structure.materials {
            material_health.insert(material.id.as_str(), material.health_milli);
        }
        let mut cells = Vec::with_capacity(structure.cells.len());
        let mut cell_indexes = HashMap::with_capacity(structure.cells.len());
        let mut previous_id: Option<&str> = None;
        for (index, cell) in structure.cells.iter().enumerate() {
            if previous_id.is_some_and(|previous| previous >= cell.id.as_str()) {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::InvalidEvent,
                    "manifest cells are not in stable ID order",
                ));
            }
            let Some(&max_health_milli) = material_health.get(cell.material_id.as_str()) else {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::InvalidEvent,
                    format!("cell {} has no material health", cell.id),
                ));
            };
            if cell_indexes.insert(cell.id.clone(), index).is_some() {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::InvalidEvent,
                    format!("duplicate cell ID {}", cell.id),
                ));
            }
            previous_id = Some(cell.id.as_str());
            cells.push(VoxelCellState::intact(
                cell.id.clone(),
                cell.coord,
                cell.material_id.clone(),
                cell.support_ids.clone(),
                max_health_milli,
            ));
        }
        Ok(Self {
            source_entity_id: structure.source_entity_id.clone(),
            profile_id: structure.structure_id.clone(),
            generation: structure.generation,
            tick: 0,
            source_cursor: None,
            cells,
            cell_indexes,
            destruction_started: false,
            possessed_owner: None,
            possession_payload: 0,
            source_life: 0,
            source_action: 0,
            source_active: true,
            impact_position_um: None,
            damage_hit_count: 0,
        })
    }

    /// Replace the mirror with a newer manifest generation and clear all
    /// cursors, lifecycle metadata, and cell damage.  Older or equal
    /// generations are rejected so replay cannot silently rewind state.
    pub fn reset_generation(&mut self, structure: &VoxelStructure) -> Result<(), VoxelStateError> {
        if structure.generation <= self.generation {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::Stale,
                format!(
                    "reset generation {} is not newer than current generation {}",
                    structure.generation, self.generation
                ),
            ));
        }
        *self = Self::new(structure)?;
        Ok(())
    }

    /// Alias emphasizing that the generation transition also resets detail
    /// state.  It is intentionally not a generic rewind API.
    pub fn reset_for_generation(
        &mut self,
        structure: &VoxelStructure,
    ) -> Result<(), VoxelStateError> {
        self.reset_generation(structure)
    }

    /// Restore a validated private snapshot without routing it through source
    /// gameplay events.  The snapshot module owns canonical hash/identity
    /// validation; this narrow crate-private seam only applies already checked
    /// fixed-point fields to this detail mirror.
    pub(crate) fn restore_snapshot_state(
        &mut self,
        generation: u32,
        tick: u64,
        source_cursor: Option<VoxelSourceCursor>,
        cell_overrides: &[(String, u32, bool)],
        destruction_started: bool,
        possessed_owner: Option<u16>,
        possession_payload: u32,
        source_life: i32,
        source_action: u8,
        source_active: bool,
        impact_position_um: Option<[i64; 3]>,
        damage_hit_count: u32,
    ) -> Result<(), VoxelStateError> {
        self.check_generation(generation)?;
        if source_cursor.is_some_and(|cursor| cursor.tick > tick) {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::InvalidEvent,
                "snapshot cursor tick exceeds snapshot tick",
            ));
        }
        if cell_overrides
            .windows(2)
            .any(|window| window[0].0 >= window[1].0)
        {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::InvalidEvent,
                "snapshot cell overrides must be sorted and unique",
            ));
        }
        for (id, health_milli, present) in cell_overrides {
            let Some(&index) = self.cell_indexes.get(id) else {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::UnknownCell,
                    format!("snapshot references unknown cell {id}"),
                ));
            };
            let cell = &self.cells[index];
            if *health_milli > cell.max_health_milli
                || (*present && *health_milli == 0)
                || (!*present && *health_milli != 0)
            {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::InvalidEvent,
                    format!("snapshot health/presence is invalid for cell {id}"),
                ));
            }
        }

        for cell in &mut self.cells {
            cell.health_milli = cell.max_health_milli;
            cell.present = true;
        }
        for (id, health_milli, present) in cell_overrides {
            let index = self.cell_indexes[id];
            let cell = &mut self.cells[index];
            cell.health_milli = *health_milli;
            cell.present = *present;
        }
        self.tick = tick;
        self.source_cursor = source_cursor;
        self.destruction_started = destruction_started;
        self.possessed_owner = possessed_owner;
        self.possession_payload = possession_payload;
        self.source_life = source_life;
        self.source_action = source_action;
        self.source_active = source_active;
        self.impact_position_um = impact_position_um;
        self.damage_hit_count = damage_hit_count;
        Ok(())
    }

    #[must_use]
    pub fn source_entity_id(&self) -> &str {
        &self.source_entity_id
    }

    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn source_cursor(&self) -> Option<VoxelSourceCursor> {
        self.source_cursor
    }

    #[must_use]
    pub fn cells(&self) -> &[VoxelCellState] {
        &self.cells
    }

    #[must_use]
    pub fn cell(&self, id: &str) -> Option<&VoxelCellState> {
        self.cell_indexes
            .get(id)
            .and_then(|index| self.cells.get(*index))
    }

    #[must_use]
    pub const fn destruction_started(&self) -> bool {
        self.destruction_started
    }

    #[must_use]
    pub const fn possessed_owner(&self) -> Option<u16> {
        self.possessed_owner
    }

    #[must_use]
    pub const fn possession_payload(&self) -> u32 {
        self.possession_payload
    }

    #[must_use]
    pub const fn source_life(&self) -> i32 {
        self.source_life
    }

    #[must_use]
    pub const fn source_action(&self) -> u8 {
        self.source_action
    }

    #[must_use]
    pub const fn source_active(&self) -> bool {
        self.source_active
    }

    #[must_use]
    pub const fn impact_position_um(&self) -> Option<[i64; 3]> {
        self.impact_position_um
    }

    #[must_use]
    pub const fn damage_hit_count(&self) -> u32 {
        self.damage_hit_count
    }

    /// Advance the detail clock without inventing a source event cursor.
    pub fn advance_tick(&mut self, generation: u32, tick: u64) -> Result<(), VoxelStateError> {
        self.check_generation(generation)?;
        if tick < self.tick {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::TickRegression,
                format!("tick {tick} is older than current tick {}", self.tick),
            ));
        }
        self.tick = tick;
        Ok(())
    }

    /// Apply one source input through the same bounded ordered path as a
    /// batch.  A one-event batch is useful to callers that stream events.
    pub fn apply_source_event(&mut self, event: &VoxelSourceEvent) -> Result<(), VoxelStateError> {
        self.apply_source_events(std::slice::from_ref(event))
            .map(|_| ())
    }

    /// Apply an ordered batch atomically after validating all caps, cursors,
    /// references, and payloads.  Rejected batches leave this state unchanged.
    pub fn apply_source_events(
        &mut self,
        events: &[VoxelSourceEvent],
    ) -> Result<usize, VoxelStateError> {
        if events.len() > MAX_SOURCE_EVENTS_PER_BATCH {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::Cap,
                format!(
                    "event batch length {} exceeds cap {}",
                    events.len(),
                    MAX_SOURCE_EVENTS_PER_BATCH
                ),
            ));
        }
        let mut cursor = self.source_cursor;
        let mut tick = self.tick;
        for event in events {
            self.validate_event(event, cursor, tick)?;
            cursor = Some(event.cursor);
            tick = tick.max(event.cursor.tick);
        }
        for event in events {
            self.apply_validated_event(event);
        }
        self.source_cursor = cursor;
        self.tick = tick;
        Ok(events.len())
    }

    fn check_generation(&self, generation: u32) -> Result<(), VoxelStateError> {
        if generation != self.generation {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::WrongGeneration,
                format!(
                    "event generation {generation} does not match state generation {}",
                    self.generation
                ),
            ));
        }
        Ok(())
    }

    fn validate_event(
        &self,
        event: &VoxelSourceEvent,
        previous_cursor: Option<VoxelSourceCursor>,
        current_tick: u64,
    ) -> Result<(), VoxelStateError> {
        if event.source_entity_id != self.source_entity_id || event.profile_id != self.profile_id {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::WrongIdentity,
                format!(
                    "event identity {}/{} does not match state {}/{}",
                    event.source_entity_id,
                    event.profile_id,
                    self.source_entity_id,
                    self.profile_id
                ),
            ));
        }
        self.check_generation(event.generation)?;
        if event.cursor.tick < current_tick {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::Stale,
                format!(
                    "event tick {} is older than current tick {current_tick}",
                    event.cursor.tick
                ),
            ));
        }
        if let Some(previous) = previous_cursor {
            if event.cursor == previous {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::Duplicate,
                    format!("event cursor {:?} was already consumed", event.cursor),
                ));
            }
            if event.cursor < previous {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::OutOfOrder,
                    format!(
                        "event cursor {:?} follows {:?} out of order",
                        event.cursor, previous
                    ),
                ));
            }
        }
        if event.target_cell_ids.len() > MAX_TARGET_CELLS_PER_EVENT {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::Cap,
                format!(
                    "target list length {} exceeds cap {}",
                    event.target_cell_ids.len(),
                    MAX_TARGET_CELLS_PER_EVENT
                ),
            ));
        }
        if event
            .target_cell_ids
            .windows(2)
            .any(|window| window[0] >= window[1])
        {
            return Err(VoxelStateError::new(
                VoxelStateErrorKind::InvalidEvent,
                "target cell IDs must be sorted and unique",
            ));
        }
        if event.kind == VoxelSourceEventKind::Damaged {
            if event.target_cell_ids.is_empty() || event.damage_milli == 0 {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::InvalidEvent,
                    "damage events require targets and positive damage",
                ));
            }
        }
        for id in &event.target_cell_ids {
            if !self.cell_indexes.contains_key(id) {
                return Err(VoxelStateError::new(
                    VoxelStateErrorKind::UnknownCell,
                    format!("event references unknown cell {id}"),
                ));
            }
        }
        Ok(())
    }

    fn apply_validated_event(&mut self, event: &VoxelSourceEvent) {
        self.source_life = event.source_life;
        self.source_action = event.source_action;
        self.source_active = event.source_active;
        self.impact_position_um = event.impact_position_um;
        match event.kind {
            VoxelSourceEventKind::Damaged => {
                self.damage_hit_count = self.damage_hit_count.saturating_add(1);
                for id in &event.target_cell_ids {
                    let index = self.cell_indexes[id];
                    let cell = &mut self.cells[index];
                    cell.health_milli = cell.health_milli.saturating_sub(event.damage_milli);
                    if cell.health_milli == 0 {
                        cell.present = false;
                    }
                }
            }
            VoxelSourceEventKind::DestructionStarted => {
                self.destruction_started = true;
            }
            VoxelSourceEventKind::RemoveFootprint => {
                if event.target_cell_ids.is_empty() {
                    for cell in &mut self.cells {
                        cell.health_milli = 0;
                        cell.present = false;
                    }
                } else {
                    for id in &event.target_cell_ids {
                        let index = self.cell_indexes[id];
                        let cell = &mut self.cells[index];
                        cell.health_milli = 0;
                        cell.present = false;
                    }
                }
            }
            VoxelSourceEventKind::Possessed => {
                self.possessed_owner = Some(event.owner_entity_index);
                self.possession_payload = event.payload;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::voxel_structure::load_manifest;

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

    fn state() -> VoxelStructureState {
        VoxelStructureState::new(&fixture_structure()).unwrap()
    }

    const SOURCE_ID: &str = "synthetic-house-0";
    const PROFILE_ID: &str = "synthetic-pointy-house";

    #[test]
    fn construction_is_intact_and_stably_ordered() {
        let state = state();
        assert_eq!(state.generation(), 1);
        assert_eq!(state.tick(), 0);
        assert_eq!(state.source_cursor(), None);
        assert!(state.cells().iter().all(|cell| cell.present));
        assert!(
            state
                .cells()
                .windows(2)
                .all(|window| window[0].id < window[1].id)
        );
        assert_eq!(
            state.cell("cell-n1-p0-n1").unwrap().max_health_milli,
            100_000
        );
        assert_eq!(
            state.cell("cell-n1-p0-n1").unwrap().material_id,
            "material-masonry"
        );
        assert_eq!(
            state.cell("cell-n1-p0-n1").unwrap().support_ids,
            vec!["support-structure", "support-foundation"]
        );
    }

    #[test]
    fn damage_updates_fixed_point_health_and_presence() {
        let mut state = state();
        let first = "cell-n1-p0-n1".to_owned();
        let second = "cell-n1-p0-p0".to_owned();
        let event = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(4, 0),
            vec![first.clone(), second.clone()],
            40_000,
        );
        assert_eq!(state.apply_source_event(&event).unwrap(), ());
        assert_eq!(state.cell(&first).unwrap().health_milli, 60_000);
        assert_eq!(state.cell(&second).unwrap().health_milli, 60_000);
        let destroy = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(4, 1),
            vec![first.clone()],
            60_000,
        );
        state.apply_source_event(&destroy).unwrap();
        assert_eq!(state.cell(&first).unwrap().health_milli, 0);
        assert!(!state.cell(&first).unwrap().present);
        assert!(state.cell(&second).unwrap().present);
        assert_eq!(state.cell(&first).unwrap().material_id, "material-masonry");
        assert_eq!(
            state.cell(&first).unwrap().support_ids,
            vec!["support-structure", "support-foundation"]
        );
    }

    #[test]
    fn newer_generation_reset_clears_damage_cursor_and_metadata() {
        let mut state = state();
        let event = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(8, 0),
            vec!["cell-n1-p0-n1".to_owned()],
            10,
        )
        .with_source_metadata(123, 52, false, Some([1, 2, 3]));
        state.apply_source_event(&event).unwrap();
        let destruction = VoxelSourceEvent::destruction_started(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(8, 1),
            Vec::new(),
        );
        state.apply_source_event(&destruction).unwrap();
        let possession = VoxelSourceEvent::possessed(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(8, 2),
            7,
            99,
        );
        state.apply_source_event(&possession).unwrap();

        let mut next = fixture_structure();
        next.generation = 2;
        state.reset_generation(&next).unwrap();
        assert_eq!(state.generation(), 2);
        assert_eq!(state.tick(), 0);
        assert_eq!(state.source_cursor(), None);
        assert!(!state.destruction_started());
        assert_eq!(state.possessed_owner(), None);
        assert_eq!(state.possession_payload(), 0);
        assert_eq!(state.source_life(), 0);
        assert_eq!(state.source_action(), 0);
        assert!(state.source_active());
        assert_eq!(state.impact_position_um(), None);
        assert!(
            state
                .cells()
                .iter()
                .all(|cell| { cell.present && cell.health_milli == cell.max_health_milli })
        );
        assert_eq!(
            state.reset_generation(&next).unwrap_err().kind,
            VoxelStateErrorKind::Stale
        );
        let mut older = next;
        older.generation = 1;
        assert_eq!(
            state.reset_for_generation(&older).unwrap_err().kind,
            VoxelStateErrorKind::Stale
        );
    }

    #[test]
    fn ordered_cursor_and_generation_guards_are_deterministic() {
        let mut state = state();
        let event = VoxelSourceEvent::possessed(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(2, 4),
            7,
            99,
        );
        state.apply_source_event(&event).unwrap();
        assert_eq!(state.source_cursor(), Some(event.cursor));
        assert_eq!(state.tick(), 2);
        assert_eq!(state.possessed_owner(), Some(7));
        assert_eq!(state.possession_payload(), 99);

        let duplicate = state.apply_source_event(&event).unwrap_err();
        assert_eq!(duplicate.kind, VoxelStateErrorKind::Duplicate);
        let stale = state
            .apply_source_event(&VoxelSourceEvent::possessed(
                SOURCE_ID,
                PROFILE_ID,
                1,
                VoxelSourceCursor::new(1, 9),
                8,
                1,
            ))
            .unwrap_err();
        assert_eq!(stale.kind, VoxelStateErrorKind::Stale);
        let wrong_generation = state
            .apply_source_event(&VoxelSourceEvent::possessed(
                SOURCE_ID,
                PROFILE_ID,
                2,
                VoxelSourceCursor::new(3, 0),
                8,
                1,
            ))
            .unwrap_err();
        assert_eq!(wrong_generation.kind, VoxelStateErrorKind::WrongGeneration);
        let out_of_order = state
            .apply_source_event(&VoxelSourceEvent::possessed(
                SOURCE_ID,
                PROFILE_ID,
                1,
                VoxelSourceCursor::new(2, 3),
                8,
                1,
            ))
            .unwrap_err();
        assert_eq!(out_of_order.kind, VoxelStateErrorKind::OutOfOrder);
    }

    #[test]
    fn lifecycle_events_and_atomic_batch_behavior_are_bounded() {
        let mut lifecycle_state = state();
        let started = VoxelSourceEvent::destruction_started(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(10, 0),
            Vec::new(),
        );
        let removed = VoxelSourceEvent::remove_footprint(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(10, 1),
            vec![],
        );
        assert_eq!(
            lifecycle_state
                .apply_source_events(&[started, removed])
                .unwrap(),
            2
        );
        assert!(lifecycle_state.destruction_started());
        assert!(
            lifecycle_state
                .cells()
                .iter()
                .all(|cell| !cell.present && cell.health_milli == 0)
        );

        let mut state_after = state();
        let before = state_after.clone();
        let valid = VoxelSourceEvent::possessed(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(1, 0),
            1,
            1,
        );
        let invalid = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(1, 1),
            vec!["cell-not-in-manifest".to_owned()],
            1,
        );
        let error = state_after
            .apply_source_events(&[valid, invalid])
            .unwrap_err();
        assert_eq!(error.kind, VoxelStateErrorKind::UnknownCell);
        assert_eq!(state_after, before);

        let too_many = (0..=MAX_SOURCE_EVENTS_PER_BATCH)
            .map(|number| {
                VoxelSourceEvent::possessed(
                    SOURCE_ID,
                    PROFILE_ID,
                    1,
                    VoxelSourceCursor::new(2, number as u32),
                    0,
                    0,
                )
            })
            .collect::<Vec<_>>();
        let error = state_after.apply_source_events(&too_many).unwrap_err();
        assert_eq!(error.kind, VoxelStateErrorKind::Cap);
    }

    #[test]
    fn input_validation_rejects_unknown_unsorted_duplicate_and_tick_regression() {
        let mut state = state();
        let unknown = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(1, 0),
            vec!["missing".to_owned()],
            1,
        );
        assert_eq!(
            state.apply_source_event(&unknown).unwrap_err().kind,
            VoxelStateErrorKind::UnknownCell
        );
        let unsorted = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(1, 1),
            vec!["cell-n1-p0-p0".to_owned(), "cell-n1-p0-n1".to_owned()],
            1,
        );
        assert_eq!(
            state.apply_source_event(&unsorted).unwrap_err().kind,
            VoxelStateErrorKind::InvalidEvent
        );
        let duplicate = VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            1,
            VoxelSourceCursor::new(1, 2),
            vec!["cell-n1-p0-n1".to_owned(), "cell-n1-p0-n1".to_owned()],
            1,
        );
        assert_eq!(
            state.apply_source_event(&duplicate).unwrap_err().kind,
            VoxelStateErrorKind::InvalidEvent
        );
        state.advance_tick(1, 5).unwrap();
        assert_eq!(
            state.advance_tick(1, 4).unwrap_err().kind,
            VoxelStateErrorKind::TickRegression
        );
    }
}
