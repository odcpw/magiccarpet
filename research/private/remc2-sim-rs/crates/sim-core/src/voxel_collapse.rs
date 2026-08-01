//! Deterministic support, mass, and bounded collapse classification.
//!
//! This is the private VoxelMode authority's structural layer.  It consumes
//! an immutable [`VoxelStructure`] and the mutable occupancy mirror from
//! [`crate::voxel_state`], discovers face-connected components, and classifies
//! each component as supported, detached, or toppling.  It does not create
//! rigid bodies, meshes, rubble particles, gameplay events, or Unity state.
//! Coarse inertia, persistent snapshot encoding, and cluster handoff remain a
//! later private slice; this report exposes the support footprint and a stable
//! report hash so those additions have a deterministic input.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use sha2::{Digest, Sha256};

use super::{
    voxel_state::VoxelStructureState,
    voxel_structure::{CellCoord, VoxelStructure},
};

/// Maximum number of authoritative component/cluster summaries retained in a
/// report.  This mirrors the private manifest's `major_clusters` budget.
pub const MAX_MAJOR_CLUSTERS: usize = 12;

/// One deterministic component classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelComponentClassification {
    /// The component retains foundation contact and its centre of mass stays
    /// inside the foundation support footprint.
    Supported,
    /// No present cell in the component retains foundation contact.
    Detached,
    /// Foundation contact remains, but the centre-of-mass projection lies
    /// outside the remaining support footprint.
    Topple,
}

/// Whole-structure decision derived from the bounded component summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelCollapseDecision {
    Standing,
    /// A grounded component remains while another component detached.
    ShedDetached,
    Topple,
    /// No present component remains grounded.
    Collapse,
    /// The component budget was exceeded; the conservative decision is not a
    /// detailed rigid-body decomposition.
    CoarseFracture,
}

/// Fixed-point mass and centre-of-mass result for one component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelCenterOfMass {
    /// Whole micro-kilograms, rounded per cell with ties away from zero.  The
    /// same integer value is used for support tests, so replay never depends
    /// on floating-point rounding.
    pub mass_microkg: u128,
    /// World-space position in the manifest's fixed micrometre units, rounded
    /// to the nearest micrometre with ties away from zero.
    pub position_um: [i64; 3],
}

/// Stable summary for one connected component.  `component_id` also serves as
/// the stable key for a future bounded authoritative rubble cluster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelComponentReport {
    pub component_id: String,
    pub cell_ids: Vec<String>,
    pub foundation_cell_ids: Vec<String>,
    /// X/Z support ranges in world micrometres.  The ranges are inclusive
    /// centre positions used by the fixed-point projection test.
    pub support_footprint_x_um: Option<[i64; 2]>,
    pub support_footprint_z_um: Option<[i64; 2]>,
    pub center_of_mass: VoxelCenterOfMass,
    pub classification: VoxelComponentClassification,
}

/// Bounded structural classification for one state snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelCollapseReport {
    pub source_entity_id: String,
    pub profile_id: String,
    pub generation: u32,
    /// Total components discovered before the major-cluster cap.
    pub component_count: usize,
    /// Stable summaries retained for authoritative major clusters.
    pub components: Vec<VoxelComponentReport>,
    pub omitted_component_count: usize,
    pub decision: VoxelCollapseDecision,
    pub coarse_fracture: bool,
    /// SHA-256 over the canonical fixed-point report fields.  This is a
    /// replay/evidence identity, not a network or save-format contract.
    pub report_hash_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoxelCollapseErrorKind {
    WrongIdentity,
    WrongGeneration,
    InvalidState,
    UnknownMaterial,
    Overflow,
}

impl VoxelCollapseErrorKind {
    const fn label(self) -> &'static str {
        match self {
            Self::WrongIdentity => "wrong-identity",
            Self::WrongGeneration => "wrong-generation",
            Self::InvalidState => "invalid-state",
            Self::UnknownMaterial => "unknown-material",
            Self::Overflow => "overflow",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoxelCollapseError {
    pub kind: VoxelCollapseErrorKind,
    pub message: String,
}

impl VoxelCollapseError {
    fn new(kind: VoxelCollapseErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for VoxelCollapseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.label(), self.message)
    }
}

impl std::error::Error for VoxelCollapseError {}

/// Stateless support/mass analyzer.  It never mutates the detail mirror.
pub struct VoxelCollapseAnalyzer;

impl VoxelCollapseAnalyzer {
    /// Analyze present cells and return a deterministic bounded report.
    pub fn analyze(
        structure: &VoxelStructure,
        state: &VoxelStructureState,
    ) -> Result<VoxelCollapseReport, VoxelCollapseError> {
        validate_state_identity(structure, state)?;

        let material_density: HashMap<&str, u32> = structure
            .materials
            .iter()
            .map(|material| (material.id.as_str(), material.density_kg_m3))
            .collect();
        let cell_volume_um3 = cell_volume_um3(structure)?;

        let mut coord_to_index = HashMap::with_capacity(state.cells().len());
        for (index, cell) in state.cells().iter().enumerate() {
            if coord_to_index.insert(cell.coord, index).is_some() {
                return Err(VoxelCollapseError::new(
                    VoxelCollapseErrorKind::InvalidState,
                    format!("duplicate state coordinate {:?}", cell.coord),
                ));
            }
        }
        let foundation_coords: HashSet<CellCoord> =
            structure.foundation_cells.iter().copied().collect();
        let mut visited = HashSet::with_capacity(state.cells().len());
        let mut reports = Vec::new();

        // VoxelStructureState is constructed in stable cell-ID order.  Seeds
        // therefore have deterministic order; neighbours use a fixed axis
        // order and reports are sorted again by component ID below.
        for seed in state.cells() {
            if !seed.present || !visited.insert(seed.id.clone()) {
                continue;
            }
            let mut queue = VecDeque::from([seed.coord]);
            let mut indexes = Vec::new();
            while let Some(coord) = queue.pop_front() {
                let Some(&index) = coord_to_index.get(&coord) else {
                    return Err(VoxelCollapseError::new(
                        VoxelCollapseErrorKind::InvalidState,
                        format!("component references missing coordinate {coord:?}"),
                    ));
                };
                let cell = &state.cells()[index];
                if !cell.present {
                    continue;
                }
                indexes.push(index);
                for neighbor in neighbors(coord).into_iter().flatten() {
                    let Some(&neighbor_index) = coord_to_index.get(&neighbor) else {
                        continue;
                    };
                    let neighbor_cell = &state.cells()[neighbor_index];
                    if neighbor_cell.present && visited.insert(neighbor_cell.id.clone()) {
                        queue.push_back(neighbor);
                    }
                }
            }
            if indexes.is_empty() {
                continue;
            }
            reports.push(component_report(
                structure,
                state,
                &indexes,
                &foundation_coords,
                &material_density,
                cell_volume_um3,
            )?);
        }

        reports.sort_unstable_by(|left, right| left.component_id.cmp(&right.component_id));
        let component_count = reports.len();
        let coarse_fracture = component_count > MAX_MAJOR_CLUSTERS;
        let omitted_component_count = component_count.saturating_sub(MAX_MAJOR_CLUSTERS);
        reports.truncate(MAX_MAJOR_CLUSTERS);

        let decision = if coarse_fracture {
            VoxelCollapseDecision::CoarseFracture
        } else {
            whole_structure_decision(&reports)
        };
        let report_hash_sha256 = report_hash_sha256(
            structure,
            component_count,
            &reports,
            omitted_component_count,
            decision,
            coarse_fracture,
        );
        Ok(VoxelCollapseReport {
            source_entity_id: structure.source_entity_id.clone(),
            profile_id: structure.structure_id.clone(),
            generation: structure.generation,
            component_count,
            components: reports,
            omitted_component_count,
            decision,
            coarse_fracture,
            report_hash_sha256,
        })
    }

    /// Alias for callers that use classification terminology.
    pub fn classify(
        structure: &VoxelStructure,
        state: &VoxelStructureState,
    ) -> Result<VoxelCollapseReport, VoxelCollapseError> {
        Self::analyze(structure, state)
    }
}

fn validate_state_identity(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
) -> Result<(), VoxelCollapseError> {
    if state.source_entity_id() != structure.source_entity_id
        || state.profile_id() != structure.structure_id
    {
        return Err(VoxelCollapseError::new(
            VoxelCollapseErrorKind::WrongIdentity,
            "state identity does not match structure manifest",
        ));
    }
    if state.generation() != structure.generation {
        return Err(VoxelCollapseError::new(
            VoxelCollapseErrorKind::WrongGeneration,
            "state generation does not match structure manifest",
        ));
    }
    if state.cells().len() != structure.cells.len() {
        return Err(VoxelCollapseError::new(
            VoxelCollapseErrorKind::InvalidState,
            "state cell count does not match structure manifest",
        ));
    }
    for (manifest_cell, state_cell) in structure.cells.iter().zip(state.cells()) {
        if manifest_cell.id != state_cell.id
            || manifest_cell.coord != state_cell.coord
            || manifest_cell.material_id != state_cell.material_id
        {
            return Err(VoxelCollapseError::new(
                VoxelCollapseErrorKind::InvalidState,
                format!(
                    "state cell {} does not match structure manifest",
                    state_cell.id
                ),
            ));
        }
    }
    Ok(())
}

fn component_report(
    structure: &VoxelStructure,
    state: &VoxelStructureState,
    indexes: &[usize],
    foundation_coords: &HashSet<CellCoord>,
    material_density: &HashMap<&str, u32>,
    cell_volume_um3: u128,
) -> Result<VoxelComponentReport, VoxelCollapseError> {
    let mut cell_ids: Vec<String> = indexes
        .iter()
        .map(|&index| state.cells()[index].id.clone())
        .collect();
    cell_ids.sort_unstable();
    let component_id = format!("component-{}", cell_ids[0]);

    let mut foundation_cell_ids: Vec<String> = indexes
        .iter()
        .filter_map(|&index| {
            let cell = &state.cells()[index];
            foundation_coords
                .contains(&cell.coord)
                .then(|| cell.id.clone())
        })
        .collect();
    foundation_cell_ids.sort_unstable();

    let mut total_mass_microkg = 0_u128;
    let mut weighted_world = [0_i128; 3];
    let mut support_x = None::<(i64, i64)>;
    let mut support_z = None::<(i64, i64)>;
    for &index in indexes {
        let state_cell = &state.cells()[index];
        let density = material_density
            .get(state_cell.material_id.as_str())
            .copied()
            .ok_or_else(|| {
                VoxelCollapseError::new(
                    VoxelCollapseErrorKind::UnknownMaterial,
                    format!("cell {} references unknown material", state_cell.id),
                )
            })?;
        let cell_mass = cell_mass_microkg(density, cell_volume_um3)?;
        total_mass_microkg = total_mass_microkg.checked_add(cell_mass).ok_or_else(|| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "component mass overflows fixed-point accumulator",
            )
        })?;
        let center = cell_center_um(structure, state_cell.coord)?;
        let mass_i128 = i128::try_from(cell_mass).map_err(|_| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "cell mass does not fit signed fixed-point accumulator",
            )
        })?;
        for axis in 0..3 {
            let contribution =
                i128::from(center[axis])
                    .checked_mul(mass_i128)
                    .ok_or_else(|| {
                        VoxelCollapseError::new(
                            VoxelCollapseErrorKind::Overflow,
                            "weighted centre of mass overflows fixed-point accumulator",
                        )
                    })?;
            weighted_world[axis] =
                weighted_world[axis]
                    .checked_add(contribution)
                    .ok_or_else(|| {
                        VoxelCollapseError::new(
                            VoxelCollapseErrorKind::Overflow,
                            "centre of mass sum overflows fixed-point accumulator",
                        )
                    })?;
        }
        if foundation_coords.contains(&state_cell.coord) {
            update_range(&mut support_x, center[0]);
            update_range(&mut support_z, center[2]);
        }
    }
    if total_mass_microkg == 0 {
        return Err(VoxelCollapseError::new(
            VoxelCollapseErrorKind::Overflow,
            "component mass rounded to zero",
        ));
    }
    let position_um = weighted_world
        .into_iter()
        .map(|value| rounded_div_signed(value, total_mass_microkg))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "centre of mass does not contain three axes",
            )
        })?;
    let classification = if foundation_cell_ids.is_empty() {
        VoxelComponentClassification::Detached
    } else if support_projection_inside(
        weighted_world,
        total_mass_microkg,
        support_x.expect("foundation IDs imply support X range"),
        support_z.expect("foundation IDs imply support Z range"),
    ) {
        VoxelComponentClassification::Supported
    } else {
        VoxelComponentClassification::Topple
    };

    Ok(VoxelComponentReport {
        component_id,
        cell_ids,
        foundation_cell_ids,
        support_footprint_x_um: support_x.map(range_array),
        support_footprint_z_um: support_z.map(range_array),
        center_of_mass: VoxelCenterOfMass {
            mass_microkg: total_mass_microkg,
            position_um,
        },
        classification,
    })
}

fn whole_structure_decision(reports: &[VoxelComponentReport]) -> VoxelCollapseDecision {
    if reports.is_empty() {
        return VoxelCollapseDecision::Collapse;
    }
    let has_topple = reports
        .iter()
        .any(|report| report.classification == VoxelComponentClassification::Topple);
    let has_grounded = reports
        .iter()
        .any(|report| report.classification == VoxelComponentClassification::Supported);
    let has_detached = reports
        .iter()
        .any(|report| report.classification == VoxelComponentClassification::Detached);
    if has_topple {
        VoxelCollapseDecision::Topple
    } else if has_detached && has_grounded {
        VoxelCollapseDecision::ShedDetached
    } else if has_detached {
        VoxelCollapseDecision::Collapse
    } else {
        VoxelCollapseDecision::Standing
    }
}

fn update_range(range: &mut Option<(i64, i64)>, value: i64) {
    match range {
        Some((minimum, maximum)) => {
            *minimum = (*minimum).min(value);
            *maximum = (*maximum).max(value);
        }
        None => *range = Some((value, value)),
    }
}

const fn range_array(range: (i64, i64)) -> [i64; 2] {
    [range.0, range.1]
}

fn report_hash_sha256(
    structure: &VoxelStructure,
    component_count: usize,
    components: &[VoxelComponentReport],
    omitted_component_count: usize,
    decision: VoxelCollapseDecision,
    coarse_fracture: bool,
) -> String {
    let mut hasher = Sha256::new();
    hash_string(&mut hasher, &structure.source_entity_id);
    hash_string(&mut hasher, &structure.structure_id);
    hasher.update(structure.generation.to_le_bytes());
    hasher.update((component_count as u64).to_le_bytes());
    hasher.update((omitted_component_count as u64).to_le_bytes());
    hasher.update([decision_tag(decision)]);
    hasher.update([u8::from(coarse_fracture)]);
    for component in components {
        hash_string(&mut hasher, &component.component_id);
        hasher.update((component.cell_ids.len() as u64).to_le_bytes());
        for cell_id in &component.cell_ids {
            hash_string(&mut hasher, cell_id);
        }
        hasher.update((component.foundation_cell_ids.len() as u64).to_le_bytes());
        for cell_id in &component.foundation_cell_ids {
            hash_string(&mut hasher, cell_id);
        }
        hash_range(&mut hasher, component.support_footprint_x_um);
        hash_range(&mut hasher, component.support_footprint_z_um);
        hasher.update(component.center_of_mass.mass_microkg.to_le_bytes());
        for position in component.center_of_mass.position_um {
            hasher.update(position.to_le_bytes());
        }
        hasher.update([classification_tag(component.classification)]);
    }
    format!("{:x}", hasher.finalize())
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_range(hasher: &mut Sha256, range: Option<[i64; 2]>) {
    match range {
        Some(values) => {
            hasher.update([1]);
            hasher.update(values[0].to_le_bytes());
            hasher.update(values[1].to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

const fn classification_tag(classification: VoxelComponentClassification) -> u8 {
    match classification {
        VoxelComponentClassification::Supported => 0,
        VoxelComponentClassification::Detached => 1,
        VoxelComponentClassification::Topple => 2,
    }
}

const fn decision_tag(decision: VoxelCollapseDecision) -> u8 {
    match decision {
        VoxelCollapseDecision::Standing => 0,
        VoxelCollapseDecision::ShedDetached => 1,
        VoxelCollapseDecision::Topple => 2,
        VoxelCollapseDecision::Collapse => 3,
        VoxelCollapseDecision::CoarseFracture => 4,
    }
}

fn support_projection_inside(
    weighted_world: [i128; 3],
    mass_microkg: u128,
    support_x: (i64, i64),
    support_z: (i64, i64),
) -> bool {
    let Ok(mass) = i128::try_from(mass_microkg) else {
        return false;
    };
    let inside = |weighted: i128, range: (i64, i64)| {
        weighted >= i128::from(range.0) * mass && weighted <= i128::from(range.1) * mass
    };
    inside(weighted_world[0], support_x) && inside(weighted_world[2], support_z)
}

fn cell_volume_um3(structure: &VoxelStructure) -> Result<u128, VoxelCollapseError> {
    let size = u128::from(structure.cell_size_um);
    size.checked_mul(size)
        .and_then(|square| square.checked_mul(size))
        .ok_or_else(|| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "cell volume overflows fixed-point accumulator",
            )
        })
}

fn cell_mass_microkg(
    density_kg_m3: u32,
    cell_volume_um3: u128,
) -> Result<u128, VoxelCollapseError> {
    // density * (cell_size_um^3 / 1e18 m^3) kg, then kg * 1e6
    // micro-kg => divide by 1e12.  Round ties away from zero; a positive
    // material/cell always retains one unit even for an unusually tiny cell.
    let numerator = u128::from(density_kg_m3)
        .checked_mul(cell_volume_um3)
        .ok_or_else(|| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "cell mass overflows fixed-point accumulator",
            )
        })?;
    let denominator = 1_000_000_000_000_u128;
    let rounded = numerator.checked_add(denominator / 2).ok_or_else(|| {
        VoxelCollapseError::new(
            VoxelCollapseErrorKind::Overflow,
            "cell mass rounding overflows fixed-point accumulator",
        )
    })? / denominator;
    Ok(rounded.max(1))
}

fn cell_center_um(
    structure: &VoxelStructure,
    coord: CellCoord,
) -> Result<[i64; 3], VoxelCollapseError> {
    let cell_size = i128::from(structure.cell_size_um);
    let half = cell_size / 2;
    let mut center = [0_i64; 3];
    for axis in 0..3 {
        let delta = i128::from(coord[axis]) - i128::from(structure.anchor_cell[axis]);
        let offset = delta.checked_mul(cell_size).ok_or_else(|| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "cell centre multiplication overflows fixed-point arithmetic",
            )
        })?;
        let value = i128::from(structure.anchor_world_um[axis])
            .checked_add(offset)
            .and_then(|value| value.checked_add(half))
            .ok_or_else(|| {
                VoxelCollapseError::new(
                    VoxelCollapseErrorKind::Overflow,
                    "cell centre addition overflows fixed-point arithmetic",
                )
            })?;
        center[axis] = i64::try_from(value).map_err(|_| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "cell centre does not fit fixed-point position",
            )
        })?;
    }
    Ok(center)
}

fn rounded_div_signed(value: i128, denominator: u128) -> Result<i64, VoxelCollapseError> {
    let denominator = i128::try_from(denominator).map_err(|_| {
        VoxelCollapseError::new(
            VoxelCollapseErrorKind::Overflow,
            "mass denominator does not fit signed arithmetic",
        )
    })?;
    let magnitude = value.unsigned_abs();
    let rounded = magnitude
        .checked_add(u128::try_from(denominator / 2).unwrap_or(u128::MAX))
        .ok_or_else(|| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "centre rounding overflows fixed-point arithmetic",
            )
        })?
        / u128::try_from(denominator).map_err(|_| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "mass denominator is negative",
            )
        })?;
    let signed = if value.is_negative() {
        -(i128::try_from(rounded).map_err(|_| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "rounded centre does not fit signed arithmetic",
            )
        })?)
    } else {
        i128::try_from(rounded).map_err(|_| {
            VoxelCollapseError::new(
                VoxelCollapseErrorKind::Overflow,
                "rounded centre does not fit signed arithmetic",
            )
        })?
    };
    i64::try_from(signed).map_err(|_| {
        VoxelCollapseError::new(
            VoxelCollapseErrorKind::Overflow,
            "rounded centre does not fit fixed-point position",
        )
    })
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

    fn intact_state(structure: &VoxelStructure) -> VoxelStructureState {
        VoxelStructureState::new(structure).unwrap()
    }

    fn remove_cells(
        structure: &VoxelStructure,
        state: &mut VoxelStructureState,
        sequence: u32,
        mut ids: Vec<String>,
    ) {
        ids.sort_unstable();
        let event = VoxelSourceEvent::remove_footprint(
            SOURCE_ID,
            PROFILE_ID,
            structure.generation,
            crate::voxel_state::VoxelSourceCursor::new(u64::from(sequence), sequence),
            ids,
        );
        state.apply_source_event(&event).unwrap();
    }

    fn foundation_ids(structure: &VoxelStructure) -> Vec<String> {
        structure
            .cells
            .iter()
            .filter(|cell| structure.foundation_cells.contains(&cell.coord))
            .map(|cell| cell.id.clone())
            .collect()
    }

    #[test]
    fn intact_house_is_supported_and_mass_conserves() {
        let structure = fixture_structure();
        let state = intact_state(&structure);
        let report = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        assert_eq!(report.decision, VoxelCollapseDecision::Standing);
        assert_eq!(report.component_count, 1);
        assert_eq!(report.components.len(), 1);
        assert_eq!(
            report.components[0].classification,
            VoxelComponentClassification::Supported
        );
        assert_eq!(report.components[0].foundation_cell_ids.len(), 8);
        assert_eq!(
            report.components[0].support_footprint_x_um,
            Some([-250_000, 750_000])
        );
        assert_eq!(
            report.components[0].support_footprint_z_um,
            Some([-250_000, 750_000])
        );
        assert!(report.components[0].center_of_mass.mass_microkg > 0);

        let repeated = VoxelCollapseAnalyzer::classify(&structure, &state).unwrap();
        assert_eq!(report, repeated);
        assert_eq!(report.report_hash_sha256.len(), 64);
    }

    #[test]
    fn local_hole_keeps_component_supported() {
        let structure = fixture_structure();
        let mut state = intact_state(&structure);
        remove_cells(&structure, &mut state, 0, vec!["cell-p1-p2-p0".to_owned()]);
        let report = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        assert_eq!(report.decision, VoxelCollapseDecision::Standing);
        assert_eq!(report.component_count, 1);
        assert_eq!(
            report.components[0].classification,
            VoxelComponentClassification::Supported
        );
    }

    #[test]
    fn roof_shed_is_reported_without_toppling_grounded_body() {
        let structure = fixture_structure();
        let mut state = intact_state(&structure);
        remove_cells(&structure, &mut state, 0, vec!["cell-p0-p3-p0".to_owned()]);
        let report = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        assert_eq!(report.decision, VoxelCollapseDecision::ShedDetached);
        let roof = report
            .components
            .iter()
            .find(|component| component.component_id == "component-cell-p0-p4-p0")
            .unwrap();
        assert_eq!(roof.classification, VoxelComponentClassification::Detached);
        assert_eq!(roof.cell_ids, vec!["cell-p0-p4-p0"]);
    }

    #[test]
    fn removing_all_foundations_is_a_detached_collapse() {
        let structure = fixture_structure();
        let mut state = intact_state(&structure);
        remove_cells(&structure, &mut state, 0, foundation_ids(&structure));
        let report = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        assert_eq!(report.decision, VoxelCollapseDecision::Collapse);
        assert_eq!(report.component_count, 1);
        assert_eq!(
            report.components[0].classification,
            VoxelComponentClassification::Detached
        );
        assert!(report.components[0].foundation_cell_ids.is_empty());
    }

    #[test]
    fn single_side_support_topples_and_tie_stays_supported() {
        let structure = fixture_structure();
        let ids = foundation_ids(&structure);
        let right_side: Vec<_> = ids
            .iter()
            .filter(|id| id.starts_with("cell-p1-p0"))
            .cloned()
            .collect();
        let mut topple_state = intact_state(&structure);
        let removed: Vec<_> = ids
            .iter()
            .filter(|id| !right_side.contains(id))
            .cloned()
            .collect();
        remove_cells(&structure, &mut topple_state, 0, removed);
        let topple = VoxelCollapseAnalyzer::analyze(&structure, &topple_state).unwrap();
        assert_eq!(topple.decision, VoxelCollapseDecision::Topple);
        assert_eq!(
            topple.components[0].classification,
            VoxelComponentClassification::Topple
        );

        // Equal mass on both support edges puts the projection on the exact
        // boundary midpoint.  Inclusive support comparisons make this stable
        // rather than introducing a float epsilon tie-break.
        let mut tie_state = intact_state(&structure);
        let keep: Vec<_> = ids
            .iter()
            .filter(|id| id.starts_with("cell-n1-p0") || id.starts_with("cell-p1-p0"))
            .cloned()
            .collect();
        let removed: Vec<_> = ids
            .iter()
            .filter(|id| !keep.contains(id))
            .cloned()
            .collect();
        remove_cells(&structure, &mut tie_state, 0, removed);
        let tie = VoxelCollapseAnalyzer::analyze(&structure, &tie_state).unwrap();
        assert_eq!(tie.decision, VoxelCollapseDecision::Standing);
        assert_eq!(
            tie.components[0].classification,
            VoxelComponentClassification::Supported
        );
    }

    #[test]
    fn mixed_density_shifts_mass_and_center_deterministically() {
        let mut structure = fixture_structure();
        let masonry = structure
            .materials
            .iter()
            .find(|material| material.kind == "masonry")
            .unwrap()
            .id
            .clone();
        let wood = structure
            .materials
            .iter()
            .find(|material| material.kind == "wood")
            .unwrap()
            .id
            .clone();
        let wood_density = structure
            .materials
            .iter()
            .find(|material| material.id == wood)
            .unwrap()
            .density_kg_m3;
        assert_ne!(masonry, wood);
        let state = intact_state(&structure);
        let baseline = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        let baseline_x = baseline.components[0].center_of_mass.position_um[0];
        let heavy_density = wood_density.saturating_mul(4);
        structure
            .materials
            .iter_mut()
            .find(|material| material.id == wood)
            .unwrap()
            .density_kg_m3 = heavy_density;
        // The fixture has no wood cells, so create one on the positive-X edge
        // through the mutable mirror and keep the manifest identity intact.
        let positive_x_id = "cell-p1-p0-p0";
        state
            .cells()
            .iter()
            .find(|cell| cell.id == positive_x_id)
            .unwrap();
        // State cells are intentionally immutable through the public slice;
        // changing the manifest material is enough to prove the analyzer's
        // density lookup by using the same material on a cloned cell record.
        structure
            .cells
            .iter_mut()
            .find(|cell| cell.id == positive_x_id)
            .unwrap()
            .material_id = wood;
        let state = VoxelStructureState::new(&structure).unwrap();
        let changed = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        assert!(changed.components[0].center_of_mass.position_um[0] > baseline_x);
        assert!(
            changed.components[0].center_of_mass.mass_microkg
                > baseline.components[0].center_of_mass.mass_microkg
        );
    }

    #[test]
    fn major_cluster_cap_returns_stable_coarse_fracture() {
        let mut structure = fixture_structure();
        structure.cells.clear();
        structure.foundation_cells.clear();
        structure.opening_cells.clear();
        for index in 0..(MAX_MAJOR_CLUSTERS + 1) {
            let coordinate = [index as i32 * 2, 0, 0];
            structure.cells.push(crate::voxel_structure::VoxelCell {
                id: format!("cell-p{}-p0-p0", index * 2),
                coord: coordinate,
                material_id: "material-masonry".to_owned(),
                part_id: "part-shell".to_owned(),
                support_ids: Vec::new(),
                render_group_id: "render-masonry".to_owned(),
            });
        }
        structure
            .cells
            .sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let state = VoxelStructureState::new(&structure).unwrap();
        let report = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        assert!(report.coarse_fracture);
        assert_eq!(report.component_count, MAX_MAJOR_CLUSTERS + 1);
        assert_eq!(report.components.len(), MAX_MAJOR_CLUSTERS);
        assert_eq!(report.omitted_component_count, 1);
        assert_eq!(report.decision, VoxelCollapseDecision::CoarseFracture);
        assert!(
            report
                .components
                .windows(2)
                .all(|pair| pair[0].component_id < pair[1].component_id)
        );
    }

    #[test]
    fn mass_sum_is_conserved_across_detached_components_and_repeated_runs() {
        let structure = fixture_structure();
        let mut state = intact_state(&structure);
        remove_cells(&structure, &mut state, 0, vec!["cell-p0-p3-p0".to_owned()]);
        let first = VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap();
        let total: u128 = first
            .components
            .iter()
            .map(|component| component.center_of_mass.mass_microkg)
            .sum();
        let intact = VoxelCollapseAnalyzer::analyze(&structure, &intact_state(&structure)).unwrap();
        let removed_mass = cell_mass_microkg(
            structure
                .materials
                .iter()
                .find(|material| material.id == "material-masonry")
                .unwrap()
                .density_kg_m3,
            cell_volume_um3(&structure).unwrap(),
        )
        .unwrap();
        assert_eq!(
            total + removed_mass,
            intact.components[0].center_of_mass.mass_microkg
        );
        for _ in 0..10_000 {
            assert_eq!(
                first,
                VoxelCollapseAnalyzer::analyze(&structure, &state).unwrap()
            );
        }
    }
}
