//! Source-order cave startup. Imported post-feature planes are comparison
//! oracles only: all writes start at the package's pre-feature boundary.

use std::collections::BTreeMap;

use super::{
    bind_definition, new_event, prepare_authored, reclaim, startup_position, synchronize_links,
};
use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    house::{HouseConstructionInput as Building, HouseState, decode_house_construction_inputs},
    legacy_terrain::LegacyTerrain,
    level_definition::{CaveCorridorDefinition, CaveFeatureDefinition},
    level1_mission::{AuthoredFeature, parse_authored_features},
    native_world::{
        EntitySnapshot, Level2EconomyState, MapType, SessionTerrain, level1_load_house_base,
        terrain_strip::cave_corridors,
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::disasters::{RingProfile, mc2 as rings},
};

/// A constructed world using the ordinary collision and presentation carrier.
/// No owner oracle rows or post-feature terrain are copied into this result.
#[derive(Clone, Debug)]
pub struct CaveStartupWorld {
    pub(crate) terrain: SessionTerrain,
    pub(crate) entities: Vec<EntitySnapshot>,
    pub(crate) global_rng: u32,
    pub(crate) free_slots: Vec<u16>,
    pub(crate) authored: Vec<AuthoredFeature>,
    pub(crate) allocation_count: usize,
}

impl CaveStartupWorld {
    /// Install constructed geometry at the feature/reset boundary. Map heads
    /// belong to the later ordinary actor allocations and remain with the
    /// reset arena when the campaign composes those independent families.
    pub(crate) fn write_geometry_into(
        &self,
        planes: &mut crate::native_world::TerrainPlanes,
    ) -> u16 {
        let generated = &self.terrain.planes;
        planes.terrain_types.clone_from(&generated.terrain_types);
        planes.ground_heights.clone_from(&generated.ground_heights);
        planes.shading.clone_from(&generated.shading);
        planes.angles.clone_from(&generated.angles);
        planes
            .secondary_heights
            .clone_from(&generated.secondary_heights);
        self.terrain.terrain_rng
    }

    /// The same geometry consumer used by normal campaign flight.
    pub fn terrain(&self) -> &SessionTerrain {
        &self.terrain
    }

    pub fn allocation_count(&self) -> usize {
        self.allocation_count
    }

    pub fn live_entities(&self) -> impl Iterator<Item = &EntitySnapshot> {
        self.entities.iter().filter(|source| source.active)
    }

    pub(crate) fn flight_world(&self) -> Result<LegacyTerrain, String> {
        LegacyTerrain::new(
            self.terrain.planes.terrain_types.to_vec(),
            self.terrain.planes.ground_heights.to_vec(),
            Some(self.terrain.planes.secondary_heights.to_vec()),
            self.terrain.planes.angles.to_vec(),
        )
    }
}

pub(crate) struct Inputs<'a> {
    pub(crate) authored: &'a [u8],
    pub(crate) pre_terrain: &'a [u8],
    pub(crate) boundary: &'a [u8],
    pub(crate) construction: &'a [u8],
    pub(crate) caves: &'a BTreeMap<u16, CaveFeatureDefinition>,
    pub(crate) corridors: &'a BTreeMap<u16, CaveCorridorDefinition>,
}

/// `GenerateEvents_49290` / `ApplyEvents_498A0`. This adapter admits a complete
/// supported startup family set before writing anything. Other worlds retain
/// their own owners; an absent constructor is never a successful no-op.
pub(crate) fn generate(inputs: Inputs<'_>) -> Result<CaveStartupWorld, String> {
    let mut features = parse_authored_features(inputs.authored)?;
    let buildings = decode_house_construction_inputs(inputs.construction)?;
    for row in features
        .iter()
        .skip(1)
        .filter(|row| row.activation_group == -1)
    {
        match (row.kind, row.subtype) {
            (10, 80..=85) | (10, 11) | (14, 2) | (10, 45) => {}
            // Other rows are constructed later by normal startup, outside
            // GenerateEvents' seven ordered preparation phases.
            (10, 9 | 15 | 27..=32 | 50 | 51 | 88) => {
                return Err(format!(
                    "cave generation needs constructor {}/{} at authored row {}",
                    row.kind, row.subtype, row.index
                ));
            }
            _ => {}
        }
    }
    let boundary = inputs.boundary;
    if boundary.len() != 16
        || &boundary[..4] != b"MFS3"
        || word(boundary, 4)? != 1
        || word(boundary, 6)? != 16
        || word(boundary, 10)? != 0
    {
        return Err("cave startup boundary is malformed".into());
    }
    let global_rng = dword(boundary, 12)?;
    let mut terrain = SessionTerrain {
        map_type: MapType::Cave,
        planes: super::super::parse_terrain(inputs.pre_terrain).map_err(|e| e.to_string())?,
        terrain_rng: word(boundary, 8)?,
        mutation_sequence: 0,
    };
    if terrain.planes.entity_indices.iter().any(|head| *head != 0) {
        return Err("cave pre-feature map heads are not empty".into());
    }
    let entities = (0..1000)
        .map(|slot| {
            let mut row = new_event(slot, 0, Position::default());
            row.reclaim_source_row();
            row
        })
        .collect::<Vec<_>>();
    let economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
        terrain.clone(),
        &entities,
        0,
        0,
        0,
        0,
        global_rng,
        BTreeMap::new(),
        None,
    )
    .map_err(|e| e.to_string())?;
    let mut arena = SourceArena::new(entities.into_boxed_slice(), economy);
    let mut allocations = 0;
    for phase in 0..7 {
        for index in 1..features.len() {
            let row = features[index];
            if row.activation_group != -1 {
                continue;
            }
            let selected = match phase {
                0 => (row.kind, row.subtype) == (10, 82),
                1 => row.kind == 10 && matches!(row.subtype, 11 | 83..=85),
                2 => row.kind == 10 && matches!(row.subtype, 80 | 81),
                3 => (row.kind, row.subtype) == (14, 2),
                5 | 6 if (row.kind, row.subtype) == (10, 45) => {
                    let building = buildings
                        .get(&(row.parameter_1 as u8))
                        .ok_or_else(|| format!("missing BUILD {}", row.parameter_1))?;
                    (building.parameter.flags & 0x10 != 0) == (phase == 5)
                }
                _ => false,
            };
            if !selected {
                continue;
            }
            let position = startup_position(row, terrain.ground_height(startup_position(row, 0)));
            match (row.kind, row.subtype) {
                (10, 80..=85) => {
                    // Linked path nodes read the complete mutable table. Apply
                    // their immutable width content before traversing any link.
                    for (&index, properties) in inputs.caves {
                        if let CaveFeatureDefinition::PathNode { half_width_tiles } = properties {
                            features[usize::from(index)].parameter_3 = u16::from(*half_width_tiles);
                        }
                    }
                    let born =
                        prepare_authored(&mut arena, &mut terrain, &mut features, row.index)?;
                    allocations += born.len();
                    if row.subtype != 80 {
                        let properties = inputs.caves.get(&row.index).ok_or_else(|| {
                            format!("cave row {} lacks typed construction content", row.index)
                        })?;
                        for allocation in born {
                            bind_definition(
                                &mut arena.entities[usize::from(allocation.source_slot)],
                                *properties,
                            );
                        }
                    }
                }
                (10, 11) => {
                    let tier = usize::from(row.parameter_1);
                    let damage = [50, 100, 300]
                        .get(tier)
                        .copied()
                        .ok_or_else(|| format!("terrain ring tier {tier} is invalid"))?;
                    let life = [31, 41, 51][tier];
                    if let Some(slot) =
                        rings::construct(&mut arena, position, RingProfile::default())
                    {
                        let source = &mut arena.entities[usize::from(slot)];
                        source.set_subspell_index(damage);
                        source.set_life(life);
                        allocations += 1;
                    }
                }
                (14, 2) => {
                    let properties = inputs.corridors.get(&row.index).ok_or_else(|| {
                        format!(
                            "corridor row {} lacks typed construction content",
                            row.index
                        )
                    })?;
                    if let Some(slot) =
                        cave_corridors::construct(&mut arena, &terrain, position, *properties)
                    {
                        synchronize_links(&mut arena, &mut terrain, slot);
                        allocations += 1;
                    }
                }
                (10, 45) => {
                    let building = &buildings[&(row.parameter_1 as u8)];
                    if construct_house(&mut arena, &mut terrain, row, building)?.is_some() {
                        allocations += 1;
                    }
                }
                _ => unreachable!("phase selection admits only supported constructors"),
            }
            features[index].kind = 0;
        }
        apply_events(&mut arena, &mut terrain, &buildings)?;
        #[cfg(test)]
        if let Some(prefix) = std::env::var_os("MC_CAVE_GENERATION_PHASE_DUMP") {
            let path = std::path::PathBuf::from(prefix).join(format!("phase-{}.bin", phase + 1));
            let mut bytes = Vec::new();
            for plane in [
                &terrain.planes.terrain_types,
                &terrain.planes.ground_heights,
                &terrain.planes.shading,
                &terrain.planes.angles,
                &terrain.planes.secondary_heights,
            ] {
                bytes.extend_from_slice(plane);
            }
            for head in &terrain.planes.entity_indices {
                bytes.extend_from_slice(&head.to_le_bytes());
            }
            bytes.extend_from_slice(&terrain.terrain_rng.to_le_bytes());
            bytes.extend_from_slice(&arena.economy.global_rng.to_le_bytes());
            std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(CaveStartupWorld {
        terrain,
        entities: arena.entities.into_vec(),
        global_rng: arena.economy.global_rng,
        free_slots: arena.economy.free_slots.clone(),
        authored: features,
        allocation_count: allocations,
    })
}

fn apply_events(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    buildings: &BTreeMap<u8, Building>,
) -> Result<(), String> {
    arena.economy.advance_global_rng();
    loop {
        let mut again = false;
        for slot in 1..arena.entities.len() {
            let source = &arena.entities[slot];
            if !source.active || source.class == 0 {
                continue;
            }
            let (class, model, action, life) =
                (source.class, source.model, source.action, source.life);
            let mut hide = false;
            if class == 14 {
                if model == 2 && life == 0 {
                    again = true;
                    cave_corridors::visit(arena, terrain, slot as u16);
                }
            } else if class == 10 {
                match (model, action) {
                    (80..=85, _) => {
                        again = true;
                        hide = super::step(terrain, &mut arena.entities[slot]).hide;
                    }
                    (11, 11) => {
                        again = true;
                        hide = rings::step(arena, terrain, slot as u16, |arena, damage| {
                            let source = &arena.entities[usize::from(damage.source_slot)];
                            let request = crate::combat::area::AreaDamage {
                                position: damage.position,
                                shape: crate::combat::area::Shape {
                                    half_width: damage.half_width,
                                    half_depth: damage.half_depth,
                                    vertical_offset: damage.vertical_offset,
                                    half_height: damage.half_height,
                                },
                                owner: damage.owner,
                                payload: damage.payload,
                                selector: (source.record[69], source.record[70]),
                                mode: 0,
                            };
                            // Startup has not allocated player spell holders;
                            // sub6D8B0 rejects this class10 self owner for XP.
                            crate::native_world::area_damage::apply(arena, None, None, request)
                        })?
                        .hide;
                    }
                    (45, 51) => {
                        again = true;
                        step_house(&mut arena.entities[slot], terrain, buildings)?;
                    }
                    (45, _) => {}
                    _ => hide = true,
                }
                let source = &mut arena.entities[slot];
                source.set_phase(source.phase.wrapping_add(1));
            } else {
                hide = true;
            }
            if hide {
                let source = &mut arena.entities[slot];
                source.set_control_flags(source.control_flags | 0x400);
            }
            if arena.entities[slot].active && arena.entities[slot].control_flags & 0x400 != 0 {
                reclaim(arena, terrain, slot as u16);
            }
        }
        if !again {
            break;
        }
    }
    Ok(())
}

fn construct_house(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    feature: AuthoredFeature,
    building: &Building,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot() else {
        return Ok(None);
    };
    let (x, y, z) = level1_load_house_base(
        &terrain.planes,
        feature.tile_x as u8,
        feature.tile_y as u8,
        &building.footprint,
    );
    let position = Position {
        x: u16::from(x) << 8,
        y: u16::from(y) << 8,
        z,
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    crate::house::bind_startup_construction(
        &mut source,
        feature.parameter_1 as u8,
        building.parameter,
        &building.footprint,
        building.raw_area,
    );
    arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    })?;
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

fn step_house(
    source: &mut EntitySnapshot,
    terrain: &mut SessionTerrain,
    buildings: &BTreeMap<u8, Building>,
) -> Result<(), String> {
    let building = &buildings[&source.record[74]];
    let mut house = HouseState::from_construction_snapshot(source, building.parameter)?;
    let remaining = house.life.wrapping_sub(1);
    let position = house.position;
    terrain.apply_house_construction_with_clearance(
        position,
        (remaining > 0).then_some(remaining),
        &building.footprint,
        building.parameter.flags & 4 == 0,
    )?;
    house.dispatch_build_entry(terrain);
    source.set_life(house.life);
    source.set_source_action(house.action);
    source.set_control_flags(house.control_flags);
    source.set_position(house.position);
    if remaining <= 0 {
        source.record[162..164].copy_from_slice(&position.z.to_le_bytes());
    }
    Ok(())
}

fn word(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let pair: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or("truncated startup word")?
        .try_into()
        .map_err(|_| "startup word extent")?;
    Ok(u16::from_le_bytes(pair))
}

fn dword(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or("truncated startup dword")?
        .try_into()
        .map_err(|_| "startup dword extent")?;
    Ok(u32::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::spec::{CampaignSelection, DecodedCampaignPackage};

    #[test]
    fn mission_four_ordinary_startup_constructs_source_terrain_and_retained_rows() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261024-b/level4");
        let package = DecodedCampaignPackage::decode(root, CampaignSelection::ordinary(4).unwrap())
            .expect("real Mission4 owner package");
        let generated = package
            .generate_cave_startup()
            .expect("ordinary source generation");
        let oracle = super::super::super::parse_terrain(
            package.member("terrain-post-features.bin").unwrap(),
        )
        .unwrap();
        let planes = generated.terrain.planes();
        let mut failures = Vec::new();
        for (name, actual, expected) in [
            (
                "material",
                planes.terrain_types.as_ref(),
                oracle.terrain_types.as_ref(),
            ),
            (
                "floor",
                planes.ground_heights.as_ref(),
                oracle.ground_heights.as_ref(),
            ),
            ("shading", planes.shading.as_ref(), oracle.shading.as_ref()),
            ("topology", planes.angles.as_ref(), oracle.angles.as_ref()),
            (
                "ceiling",
                planes.secondary_heights.as_ref(),
                oracle.secondary_heights.as_ref(),
            ),
        ] {
            let differences = actual
                .iter()
                .zip(expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (i, *a, *b))
                .collect::<Vec<_>>();
            if !differences.is_empty() {
                failures.push(format!(
                    "{name}: {} mismatches, first {:?}",
                    differences.len(),
                    &differences[..differences.len().min(5)]
                ));
            }
        }
        let head_differences = planes
            .entity_indices
            .iter()
            .zip(oracle.entity_indices.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i, *a, *b))
            .collect::<Vec<_>>();
        if !head_differences.is_empty() {
            failures.push(format!(
                "map heads: {} mismatches, first {:?}",
                head_differences.len(),
                head_differences.first()
            ));
        }
        let end = package.member("feature-end.bin").unwrap();
        if generated.global_rng != dword(end, 12).unwrap() {
            failures.push(format!(
                "global RNG: {:#x} vs {:#x}",
                generated.global_rng,
                dword(end, 12).unwrap()
            ));
        }
        if generated.terrain.terrain_rng != word(end, 8).unwrap() {
            failures.push(format!(
                "terrain RNG: {} vs {}",
                generated.terrain.terrain_rng,
                word(end, 8).unwrap()
            ));
        }
        let rows = package.member("entities-post-features.bin").unwrap();
        for row in &generated.entities {
            let expected =
                &rows[12 + usize::from(row.index) * 176..12 + (usize::from(row.index) + 1) * 176];
            if row.active != (expected[2] != 0) {
                failures.push(format!("slot {} activity mismatch", row.index));
            }
            if row.active {
                let differences = row
                    .record
                    .iter()
                    .zip(expected)
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .map(|(i, (a, b))| (i, *a, *b))
                    .collect::<Vec<_>>();
                if !differences.is_empty() {
                    failures.push(format!("slot {}: {:?}", row.index, differences));
                }
            }
        }
        if let Some(path) = std::env::var_os("MC_CAVE_GENERATION_DUMP") {
            let mut bytes = Vec::new();
            for plane in [
                &planes.terrain_types,
                &planes.ground_heights,
                &planes.shading,
                &planes.angles,
                &planes.secondary_heights,
            ] {
                bytes.extend_from_slice(plane);
            }
            for head in &planes.entity_indices {
                bytes.extend_from_slice(&head.to_le_bytes());
            }
            std::fs::write(path, bytes).expect("requested actual generation debug output");
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        let point = Position {
            x: 34 << 8,
            y: 40 << 8,
            z: 0,
        };
        let ordinary = package
            .terrain()
            .expect("ordinary constructed flight consumer");
        assert_eq!(
            ordinary.ground_height(point),
            generated.terrain.ground_height(point)
        );
        assert_eq!(
            ordinary.ceiling_height(point),
            generated.terrain.ceiling_height(point)
        );
        eprintln!(
            "Mission4 ordinary startup: {} allocations, {} retained rows, terrain RNG {}, global {:#010x}, collision floor/ceiling {}/{:?}",
            generated.allocation_count(),
            generated.live_entities().count(),
            generated.terrain.terrain_rng,
            generated.global_rng,
            ordinary.ground_height(point),
            ordinary.ceiling_height(point)
        );
    }
}
