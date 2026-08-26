//! Deterministic renderer-free damage mapping for private VoxelMode houses.
//!
//! The mapper consumes source-neutral event metadata, selects bounded detail
//! cells, and applies the resulting damage only to [`VoxelStructureState`].
//! It never mutates `HouseState`, source collision, gameplay ownership, or
//! any renderer/physics representation.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]

use std::fmt;

use super::{
    voxel_state::{
        VoxelSourceCursor, VoxelSourceEvent, VoxelSourceEventKind, VoxelStateError,
        VoxelStructureState,
    },
    voxel_structure::{CellCoord, VoxelStructure},
};

/// Maximum Manhattan-style cell radius used by one mapped impact.
pub const MAX_DAMAGE_RADIUS_CELLS: u32 = 4;
/// Maximum number of cells selected by one hit before coarse fracture mode is
/// reported and the stable nearest subset is retained.
pub const MAX_SELECTED_CELLS_PER_HIT: usize = 64;

/// One deterministic cell/damage pair returned by the mapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelDamageSelection {
    pub cell_id: String,
    pub damage_milli: u32,
}

/// Result of mapping and applying one source damage input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelDamageResult {
    pub cursor: VoxelSourceCursor,
    pub selected: Vec<VoxelDamageSelection>,
    pub local_impact_cell: CellCoord,
    pub radius_cells: u32,
    pub used_facade_fallback: bool,
    pub coarse_fracture: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelDamageErrorKind {
    InvalidEvent,
    NoTargets,
    TransformOverflow,
    State,
}

impl VoxelDamageErrorKind {
    const fn label(self) -> &'static str {
        match self {
            Self::InvalidEvent => "invalid-event",
            Self::NoTargets => "no-targets",
            Self::TransformOverflow => "transform-overflow",
            Self::State => "state",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelDamageError {
    pub kind: VoxelDamageErrorKind,
    pub message: String,
}

impl VoxelDamageError {
    fn new(kind: VoxelDamageErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for VoxelDamageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.label(), self.message)
    }
}

impl std::error::Error for VoxelDamageError {}

impl From<VoxelStateError> for VoxelDamageError {
    fn from(error: VoxelStateError) -> Self {
        Self::new(VoxelDamageErrorKind::State, error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    index: usize,
    distance_sq_um: u128,
}

/// Stateless deterministic mapper.  All mutable damage remains in the
/// supplied `VoxelStructureState`; constructing another mapper cannot change
/// replay results.
pub struct VoxelDamageMapper;

impl VoxelDamageMapper {
    /// Select and apply one damage event.
    ///
    /// A `Damaged` event may provide explicit targets, or leave targets empty
    /// when it carries an impact position or attacker position.  Empty target
    /// events are mapped against present structural cells; explicit opening
    /// coordinates are always filtered out.  The state cursor and generation
    /// guards are applied atomically by `VoxelStructureState`.
    pub fn map_and_apply(
        structure: &VoxelStructure,
        state: &mut VoxelStructureState,
        event: &VoxelSourceEvent,
    ) -> Result<VoxelDamageResult, VoxelDamageError> {
        if event.kind != VoxelSourceEventKind::Damaged || event.damage_milli == 0 {
            return Err(VoxelDamageError::new(
                VoxelDamageErrorKind::InvalidEvent,
                "damage mapper requires a positive Damaged event",
            ));
        }
        let (local_impact_cell, fallback_origin_um, used_facade_fallback) =
            if let Some(impact_position_um) = event.impact_position_um {
                (
                    local_cell_from_world(structure, impact_position_um)?,
                    impact_position_um,
                    false,
                )
            } else {
                let fallback_cell = fallback_facade_cell(structure, event);
                (
                    fallback_cell,
                    cell_center_um(structure, fallback_cell)?,
                    true,
                )
            };

        let radius_cells = damage_radius(structure, state, event, local_impact_cell);
        let mut candidates = if event.target_cell_ids.is_empty() {
            select_radius_candidates(
                structure,
                state,
                local_impact_cell,
                fallback_origin_um,
                radius_cells,
            )?
        } else {
            select_explicit_candidates(structure, state, event)?
        };
        if candidates.is_empty() {
            return Err(VoxelDamageError::new(
                VoxelDamageErrorKind::NoTargets,
                "damage event selected no present structural cells",
            ));
        }
        candidates.sort_by(|left, right| {
            left.distance_sq_um
                .cmp(&right.distance_sq_um)
                .then_with(|| {
                    structure.cells[left.index]
                        .id
                        .cmp(&structure.cells[right.index].id)
                })
        });
        let coarse_fracture = candidates.len() > MAX_SELECTED_CELLS_PER_HIT;
        candidates.truncate(MAX_SELECTED_CELLS_PER_HIT);

        // Keep the returned selection in exact-distance/stable-ID order so
        // callers can replay the mapper's bounded choice.  The state contract
        // requires target IDs in stable lexicographic order, so apply a sorted
        // copy without changing the authoritative selection order.
        let ordered_ids: Vec<String> = candidates
            .iter()
            .map(|candidate| structure.cells[candidate.index].id.clone())
            .collect();
        let mut selected_ids = ordered_ids.clone();
        selected_ids.sort_unstable();
        selected_ids.dedup();
        if selected_ids.is_empty() {
            return Err(VoxelDamageError::new(
                VoxelDamageErrorKind::NoTargets,
                "damage event selected no stable targets",
            ));
        }
        let mut mapped_event = event.clone();
        mapped_event.target_cell_ids = selected_ids.clone();
        state.apply_source_event(&mapped_event)?;

        Ok(VoxelDamageResult {
            cursor: event.cursor,
            selected: ordered_ids
                .into_iter()
                .map(|cell_id| VoxelDamageSelection {
                    cell_id,
                    damage_milli: event.damage_milli,
                })
                .collect(),
            local_impact_cell,
            radius_cells,
            used_facade_fallback,
            coarse_fracture,
        })
    }
}

fn local_cell_from_world(
    structure: &VoxelStructure,
    impact_position_um: [i64; 3],
) -> Result<CellCoord, VoxelDamageError> {
    let cell_size = i64::try_from(structure.cell_size_um).map_err(|_| {
        VoxelDamageError::new(
            VoxelDamageErrorKind::TransformOverflow,
            "cell size does not fit signed fixed-point arithmetic",
        )
    })?;
    if cell_size <= 0 {
        return Err(VoxelDamageError::new(
            VoxelDamageErrorKind::TransformOverflow,
            "cell size is not positive",
        ));
    }
    let mut local = [0_i32; 3];
    for axis in 0..3 {
        let relative = impact_position_um[axis]
            .checked_sub(structure.anchor_world_um[axis])
            .ok_or_else(|| {
                VoxelDamageError::new(
                    VoxelDamageErrorKind::TransformOverflow,
                    "impact minus anchor overflows fixed-point arithmetic",
                )
            })?;
        let coordinate = relative
            .div_euclid(cell_size)
            .checked_add(i64::from(structure.anchor_cell[axis]))
            .ok_or_else(|| {
                VoxelDamageError::new(
                    VoxelDamageErrorKind::TransformOverflow,
                    "local cell coordinate overflows",
                )
            })?;
        local[axis] = i32::try_from(coordinate).map_err(|_| {
            VoxelDamageError::new(
                VoxelDamageErrorKind::TransformOverflow,
                "local cell coordinate does not fit i32",
            )
        })?;
    }
    Ok(local)
}

fn cell_center_um(
    structure: &VoxelStructure,
    coord: CellCoord,
) -> Result<[i64; 3], VoxelDamageError> {
    let cell_size = i128::from(structure.cell_size_um);
    let half = cell_size / 2;
    let mut center = [0_i64; 3];
    for axis in 0..3 {
        let delta = i128::from(coord[axis]) - i128::from(structure.anchor_cell[axis]);
        let offset = delta.checked_mul(cell_size).ok_or_else(|| {
            VoxelDamageError::new(
                VoxelDamageErrorKind::TransformOverflow,
                "cell center multiplication overflows",
            )
        })?;
        let value = i128::from(structure.anchor_world_um[axis])
            .checked_add(offset)
            .and_then(|value| value.checked_add(half))
            .ok_or_else(|| {
                VoxelDamageError::new(
                    VoxelDamageErrorKind::TransformOverflow,
                    "cell center addition overflows",
                )
            })?;
        center[axis] = i64::try_from(value).map_err(|_| {
            VoxelDamageError::new(
                VoxelDamageErrorKind::TransformOverflow,
                "cell center does not fit fixed-point position",
            )
        })?;
    }
    Ok(center)
}

fn distance_sq_um(left: [i64; 3], right: [i64; 3]) -> Result<u128, VoxelDamageError> {
    let mut result = 0_u128;
    for axis in 0..3 {
        let delta = i128::from(left[axis]) - i128::from(right[axis]);
        let squared = delta.checked_mul(delta).ok_or_else(|| {
            VoxelDamageError::new(
                VoxelDamageErrorKind::TransformOverflow,
                "distance square overflows",
            )
        })?;
        result = result
            .checked_add(u128::try_from(squared).map_err(|_| {
                VoxelDamageError::new(
                    VoxelDamageErrorKind::TransformOverflow,
                    "distance square is negative",
                )
            })?)
            .ok_or_else(|| {
                VoxelDamageError::new(
                    VoxelDamageErrorKind::TransformOverflow,
                    "distance sum overflows",
                )
            })?;
    }
    Ok(result)
}

fn fallback_facade_cell(structure: &VoxelStructure, event: &VoxelSourceEvent) -> CellCoord {
    let min = structure.cell_bounds_min;
    let max = structure.cell_bounds_max;
    let height = i64::from(max[1]) - i64::from(min[1]) + 1;
    let y = i32::try_from(i64::from(min[1]) + i64::from(event.cursor.sequence) % height.max(1))
        .unwrap_or(min[1]);
    let side = if let Some(attacker) = event.attacker_position_um {
        let dx = i128::from(attacker[0]) - i128::from(structure.anchor_world_um[0]);
        let dz = i128::from(attacker[2]) - i128::from(structure.anchor_world_um[2]);
        if dx.unsigned_abs() >= dz.unsigned_abs() {
            if dx <= 0 { 0 } else { 1 }
        } else if dz <= 0 {
            2
        } else {
            3
        }
    } else {
        event.cursor.sequence % 4
    };
    let z_anchor = structure.anchor_cell[2].clamp(min[2], max[2]);
    let x_anchor = structure.anchor_cell[0].clamp(min[0], max[0]);
    match side {
        0 => [min[0], y, z_anchor],
        1 => [max[0], y, z_anchor],
        2 => [x_anchor, y, min[2]],
        _ => [x_anchor, y, max[2]],
    }
}

fn damage_radius(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
    event: &VoxelSourceEvent,
    local_impact_cell: CellCoord,
) -> u32 {
    let reference_strength = structure
        .cells
        .iter()
        .filter(|cell| cell.coord == local_impact_cell)
        .find_map(|cell| {
            structure
                .materials
                .iter()
                .find(|material| material.id == cell.material_id)
                .map(|material| material.strength_milli)
        })
        .or_else(|| {
            structure
                .materials
                .iter()
                .map(|material| material.strength_milli)
                .min()
        })
        .unwrap_or(1)
        .max(1);
    let material_response = (100_000_u32 / reference_strength).max(1);
    let damage_response = (event.damage_milli / 100_000).min(MAX_DAMAGE_RADIUS_CELLS);
    let repeated_response = state.damage_hit_count().min(MAX_DAMAGE_RADIUS_CELLS);
    1 + (material_response / 2 + damage_response + repeated_response)
        .min(MAX_DAMAGE_RADIUS_CELLS - 1)
}

fn select_radius_candidates(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
    local_impact_cell: CellCoord,
    impact_position_um: [i64; 3],
    radius_cells: u32,
) -> Result<Vec<Candidate>, VoxelDamageError> {
    let radius = i64::from(radius_cells);
    let mut candidates = Vec::new();
    for (index, cell) in structure.cells.iter().enumerate() {
        let Some(state_cell) = state.cell(&cell.id) else {
            continue;
        };
        if !state_cell.present || structure.opening_cells.binary_search(&cell.coord).is_ok() {
            continue;
        }
        let delta = [
            i64::from(cell.coord[0]) - i64::from(local_impact_cell[0]),
            i64::from(cell.coord[1]) - i64::from(local_impact_cell[1]),
            i64::from(cell.coord[2]) - i64::from(local_impact_cell[2]),
        ];
        if delta
            .iter()
            .any(|value| value.unsigned_abs() > radius as u64)
        {
            continue;
        }
        candidates.push(Candidate {
            index,
            distance_sq_um: distance_sq_um(
                cell_center_um(structure, cell.coord)?,
                impact_position_um,
            )?,
        });
    }
    Ok(candidates)
}

fn select_explicit_candidates(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
    event: &VoxelSourceEvent,
) -> Result<Vec<Candidate>, VoxelDamageError> {
    let mut candidates = Vec::new();
    for cell_id in &event.target_cell_ids {
        let Some(state_cell) = state.cell(cell_id) else {
            return Err(VoxelDamageError::new(
                VoxelDamageErrorKind::InvalidEvent,
                format!("explicit damage target {cell_id} is unknown"),
            ));
        };
        if !state_cell.present
            || structure
                .opening_cells
                .binary_search(&state_cell.coord)
                .is_ok()
        {
            continue;
        }
        let index = structure
            .cells
            .binary_search_by(|cell| cell.id.as_str().cmp(cell_id.as_str()))
            .map_err(|_| {
                VoxelDamageError::new(
                    VoxelDamageErrorKind::InvalidEvent,
                    format!("explicit damage target {cell_id} is not in structure"),
                )
            })?;
        candidates.push(Candidate {
            index,
            distance_sq_um: 0,
        });
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::{voxel_state::VoxelSourceEvent, voxel_structure::load_manifest};

    const SOURCE_ID: &str = "synthetic-house-0";
    const PROFILE_ID: &str = "synthetic-pointy-house";

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

    fn state(structure: &VoxelStructure) -> VoxelStructureState {
        VoxelStructureState::new(structure).unwrap()
    }

    fn damaged(
        structure: &VoxelStructure,
        sequence: u32,
        impact: Option<[i64; 3]>,
    ) -> VoxelSourceEvent {
        VoxelSourceEvent::damaged(
            SOURCE_ID,
            PROFILE_ID,
            structure.generation,
            VoxelSourceCursor::new(u64::from(sequence), sequence),
            Vec::new(),
            10_000,
        )
        .with_source_metadata(10_000, 52, true, impact)
    }

    #[test]
    fn edge_and_roof_hits_select_present_cells_without_collapse() {
        let structure = fixture_structure();
        let mut state = state(&structure);
        let edge = damaged(&structure, 0, Some([-750_000, 250_000, 0]));
        let result = VoxelDamageMapper::map_and_apply(&structure, &mut state, &edge).unwrap();
        assert!(!result.selected.is_empty());
        assert!(
            result.selected.iter().all(|selection| {
                state.cell(&selection.cell_id).unwrap().health_milli < 100_000
            })
        );
        assert!(state.cells().iter().any(|cell| cell.present));

        let roof = damaged(&structure, 1, Some([0, 2_500_000, 0]));
        let roof_result = VoxelDamageMapper::map_and_apply(&structure, &mut state, &roof).unwrap();
        assert!(!roof_result.selected.is_empty());
        assert_eq!(state.destruction_started(), false);
    }

    #[test]
    fn near_window_does_not_create_or_target_opening_cells() {
        let structure = fixture_structure();
        let mut state = state(&structure);
        let event = damaged(&structure, 0, Some([750_000, 750_000, 0]));
        let result = VoxelDamageMapper::map_and_apply(&structure, &mut state, &event).unwrap();
        assert!(result.selected.iter().all(|selection| {
            !structure
                .opening_cells
                .contains(&state.cell(&selection.cell_id).unwrap().coord)
        }));
        assert!(
            structure
                .opening_cells
                .iter()
                .all(|coord| { structure.cells.iter().all(|cell| cell.coord != *coord) })
        );
    }

    #[test]
    fn repeated_hits_expand_bounded_radius_and_reset_clears_history() {
        let structure = fixture_structure();
        let mut state = state(&structure);
        let first = damaged(&structure, 0, Some([0, 1_250_000, 0]));
        let first_result =
            VoxelDamageMapper::map_and_apply(&structure, &mut state, &first).unwrap();
        let second = damaged(&structure, 1, Some([0, 1_250_000, 0]));
        let second_result =
            VoxelDamageMapper::map_and_apply(&structure, &mut state, &second).unwrap();
        assert!(second_result.radius_cells >= first_result.radius_cells);
        assert_eq!(state.damage_hit_count(), 2);
        let mut next = structure.clone();
        next.generation = 2;
        state.reset_generation(&next).unwrap();
        assert_eq!(state.damage_hit_count(), 0);
        assert!(state.cells().iter().all(|cell| cell.present));
    }

    #[test]
    fn equal_distance_ties_use_stable_cell_id_order() {
        let structure = fixture_structure();
        let mut state = state(&structure);
        let event = damaged(&structure, 0, Some([0, 750_000, 0]));
        let result = VoxelDamageMapper::map_and_apply(&structure, &mut state, &event).unwrap();
        let ids: Vec<_> = result
            .selected
            .iter()
            .map(|selection| selection.cell_id.clone())
            .collect();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(deduped.len(), result.selected.len());

        // The mapper must be distance-first.  For equal exact distances it
        // must use the stable cell-ID order rather than manifest insertion
        // order (the fixture has several symmetric cells at this impact).
        let mut previous: Option<(u128, &str)> = None;
        for selection in &result.selected {
            let coord = structure
                .cells
                .iter()
                .find(|cell| cell.id == selection.cell_id)
                .unwrap()
                .coord;
            let distance =
                distance_sq_um(cell_center_um(&structure, coord).unwrap(), [0, 750_000, 0])
                    .unwrap();
            if let Some((previous_distance, previous_id)) = previous {
                if previous_distance == distance {
                    assert!(previous_id < selection.cell_id.as_str());
                } else {
                    assert!(previous_distance < distance);
                }
            }
            previous = Some((distance, selection.cell_id.as_str()));
        }
    }

    #[test]
    fn missing_position_uses_attacker_facade_then_sequence_fallback() {
        let structure = fixture_structure();
        let mut attacker_state = state(&structure);
        let event =
            damaged(&structure, 0, None).with_attacker_position_um(Some([-10_000_000, 0, 0]));
        let result =
            VoxelDamageMapper::map_and_apply(&structure, &mut attacker_state, &event).unwrap();
        assert!(result.used_facade_fallback);
        assert_eq!(result.local_impact_cell[0], structure.cell_bounds_min[0]);

        let mut fallback_state = state(&structure);
        let event = damaged(&structure, 3, None);
        let result =
            VoxelDamageMapper::map_and_apply(&structure, &mut fallback_state, &event).unwrap();
        assert!(result.used_facade_fallback);
        assert_eq!(result.local_impact_cell[2], structure.cell_bounds_max[2]);
    }

    #[test]
    fn cap_pressure_reports_stable_coarse_fracture_and_source_inputs_remain_untouched() {
        let mut structure = fixture_structure();
        for index in 0..64 {
            let mut extra = structure.cells[index % structure.cells.len()].clone();
            extra.id = format!("cell-extra-{index:03}");
            extra.coord = [0, 0, 0];
            structure.cells.push(extra);
        }
        structure
            .cells
            .sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let mut state = state(&structure);
        let event = damaged(&structure, 0, Some([0, 1_000_000, 0]));
        let before = event.clone();
        let result = VoxelDamageMapper::map_and_apply(&structure, &mut state, &event).unwrap();
        assert!(result.selected.len() <= MAX_SELECTED_CELLS_PER_HIT);
        assert!(result.coarse_fracture);
        assert_eq!(event, before);
        assert_eq!(structure.cells.len(), 96);
    }
}
