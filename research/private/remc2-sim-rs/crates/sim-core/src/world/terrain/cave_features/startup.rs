//! Source-order cave startup. Imported post-feature planes are comparison
//! oracles only: all writes start at the package's pre-feature boundary.

use std::collections::BTreeMap;

use super::{
    bind_definition, new_event, prepare_authored, reclaim, startup_position, synchronize_links,
};
use crate::{
    FlightWorld, Position,
    campaign::source_arena::SourceArena,
    dynamic_lighting::{DynamicLightLinkedEntity, DynamicLightingState},
    house::{HouseConstructionInput as Building, HouseState, decode_house_construction_inputs},
    legacy_terrain::LegacyTerrain,
    level_definition::{
        CaveCorridorDefinition, CaveFeatureDefinition, EffectDefinition, TerrainDisasterDefinition,
    },
    level1_mission::{AuthoredFeature, parse_authored_features},
    native_world::{
        EntitySnapshot, Level2EconomyState, MapType, ParticleParameter, SessionTerrain,
        cave_corridors, level1_load_house_base,
    },
    renderable_map_chain::RenderableMapSeed,
    world::terrain::disasters::{RingProfile, mc2 as rings},
};

/// A constructed world using the ordinary collision and presentation carrier.
/// No owner oracle rows or post-feature terrain are copied into this result.
#[derive(Clone, Debug)]
pub struct CaveStartupWorld {
    pub(crate) terrain: SessionTerrain,
    arena: SourceArena,
    pub(crate) authored: Vec<AuthoredFeature>,
    pub(crate) allocation_count: usize,
    buildings: BTreeMap<u8, Building>,
    effects: BTreeMap<u16, EffectDefinition>,
    particles: Box<[ParticleParameter]>,
}

impl CaveStartupWorld {
    /// Install geometry after the ordinary startup terrain callbacks. Other
    /// actor families retain their reset arena, RNG and map heads; this result
    /// supplies only the terrain constructed by the world owners.
    pub(crate) fn write_geometry_into(
        &self,
        planes: &mut crate::native_world::TerrainPlanes,
    ) -> Result<u16, String> {
        let terrain = self.reset_terrain()?;
        let generated = terrain.planes();
        planes.terrain_types.clone_from(&generated.terrain_types);
        planes.ground_heights.clone_from(&generated.ground_heights);
        planes.shading.clone_from(&generated.shading);
        planes.angles.clone_from(&generated.angles);
        planes
            .secondary_heights
            .clone_from(&generated.secondary_heights);
        Ok(terrain.terrain_rng())
    }

    /// Compose the terrain part of group-zero construction and the first
    /// source callback pass, following GenerateEvents. No imported reset or
    /// post-feature geometry participates in these writes. The temporary
    /// arena is deliberately not published as a complete actor startup:
    /// unrelated actors have their own ordinary construction/reset owners.
    pub fn reset_terrain(&self) -> Result<SessionTerrain, String> {
        let mut terrain = self.terrain.clone();
        let mut arena = self.arena.clone();
        let mut lighting = DynamicLightingState::reset();
        let mut effect_state = crate::world::effects::State::default();
        // sub_4A1E0 invokes sub_49F90 before group-zero construction. The
        // previous generation pass leaves a LIFO reclaim stack; rebuilding
        // descending slots restores the actual numeric callback order.
        arena.rebuild_source_arena_with_terrain(&mut terrain, &[])?;
        for &row in self.authored.iter().skip(1) {
            if row.activation_group != 0 || row.kind != 10 {
                continue;
            }
            match row.subtype {
                0 => {
                    let definition = *self.effects.get(&row.index).ok_or_else(|| {
                        format!("missing ordinary impact content at row{}", row.index)
                    })?;
                    let position = row.world_position(terrain.ground_height(row.world_position(0)));
                    if let Some(slot) = crate::world::effects::allocate(
                        &mut arena,
                        &mut terrain,
                        definition,
                        position,
                        &self.particles,
                    )? {
                        // NewAdd0A00 -> AddEvent2_847D0. The light service owns
                        // its RNG and footprint; the actual effect row owns life.
                        if lighting
                            .admit(true, true, slot, position, 128, 7, true)
                            .is_some()
                        {
                            let source = &mut arena.entities[usize::from(slot)];
                            source.set_control_flags(source.control_flags | 0x0008_0000);
                        }
                    }
                }
                45 => {
                    let building = self
                        .buildings
                        .get(&(row.parameter_1 as u8))
                        .ok_or_else(|| format!("missing ordinary BUILD {}", row.parameter_1))?;
                    construct_house(&mut arena, &mut terrain, row, building)?;
                }
                63 | 64 => {
                    let position = Position {
                        x: row.tile_x.wrapping_shl(8).wrapping_add(128),
                        y: row.tile_y.wrapping_shl(8).wrapping_add(128),
                        z: 0,
                    };
                    let position = Position {
                        z: terrain.ground_height(position),
                        ..position
                    };
                    rings::runtime::construct_marker(
                        &mut arena,
                        &mut terrain,
                        position,
                        row.subtype as u8,
                    )?;
                }
                _ => {}
            }
        }
        arena.finish_source_group();
        let effect_lists = crate::native_world::area_damage::capture_effect_lists(&mut arena, None);
        // The same numeric source order used by UpdateEntities: group-zero
        // rows reuse free slots below the retained cave controllers. A close
        // trigger therefore selects life1 before the controller's callback.
        for slot in 1..arena.entities.len() {
            let source = &arena.entities[slot];
            if !source.active {
                continue;
            }
            match (source.class, source.model, source.action) {
                (10, 0, 0) => {
                    crate::world::effects::visit(
                        &mut arena,
                        &mut terrain,
                        slot as u16,
                        &self.particles,
                        &mut effect_state,
                        |arena, damage| {
                            crate::native_world::area_damage::apply_effect(
                                arena,
                                None,
                                None,
                                None,
                                &effect_lists,
                                damage,
                            )
                        },
                    )?;
                }
                (10, 45, 51) => {
                    step_house(&mut arena.entities[slot], &mut terrain, &self.buildings)?
                }
                (10, 63 | 64, _) => rings::runtime::step_trigger(&mut arena, slot as u16)?,
                (14, 2, _) => {
                    cave_corridors::visit(&mut arena, &mut terrain, slot as u16);
                }
                _ => {}
            }
        }
        let (ground, shading) = terrain.dynamic_lighting_planes();
        lighting.service_recompute(
            shading,
            true,
            |slot| {
                arena
                    .entities
                    .get(usize::from(slot))
                    .filter(|row| row.active)
                    .map(|row| DynamicLightLinkedEntity {
                        position: row.position,
                        life: row.life,
                        disabled: row.control_flags & 0x400 != 0,
                    })
            },
            |x, y| {
                i16::try_from(crate::legacy_terrain::interpolate_ground(ground, x, y))
                    .expect("u8 terrain interpolation fits i16")
            },
        );
        Ok(terrain)
    }

    /// The same geometry consumer used by normal campaign flight.
    pub fn terrain(&self) -> &SessionTerrain {
        &self.terrain
    }

    pub fn allocation_count(&self) -> usize {
        self.allocation_count
    }

    pub fn live_entities(&self) -> impl Iterator<Item = &EntitySnapshot> {
        self.arena.entities.iter().filter(|source| source.active)
    }

    pub(crate) fn flight_world(&self) -> Result<LegacyTerrain, String> {
        LegacyTerrain::new(
            self.terrain.planes().terrain_types.to_vec(),
            self.terrain.planes().ground_heights.to_vec(),
            Some(self.terrain.planes().secondary_heights.to_vec()),
            self.terrain.planes().angles.to_vec(),
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
    pub(crate) rings: &'a BTreeMap<u16, TerrainDisasterDefinition>,
    pub(crate) effects: &'a BTreeMap<u16, EffectDefinition>,
    pub(crate) particles: &'a [ParticleParameter],
}

/// Admit only terrain constructors composed by this startup owner. Linked
/// paths delegate to their existing disaster owners in the same source scan.
pub(crate) fn supports_authored(authored: &[u8]) -> Result<bool, String> {
    Ok(!parse_authored_features(authored)?
        .iter()
        .skip(1)
        .any(|row| {
            row.kind == 10
                && ((row.activation_group == -1
                    && matches!(row.subtype, 9 | 15 | 27 | 30 | 32 | 51 | 88))
                    || (row.activation_group == 0
                        && matches!(row.subtype, 9..=11 | 15 | 27..=32 | 50 | 51 | 80..=86 | 88)))
        }))
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
            (10, 80..=85 | 11 | 28 | 29 | 31 | 45 | 50) | (14, 2) => {}
            // Other rows are constructed later by normal startup, outside
            // GenerateEvents' seven ordered preparation phases.
            (10, 9 | 15 | 27 | 30 | 32 | 51 | 88) => {
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
    let mut terrain = SessionTerrain::from_planes(
        MapType::Cave,
        crate::native_world::parse_terrain(inputs.pre_terrain).map_err(|e| e.to_string())?,
        word(boundary, 8)?,
    );
    if terrain
        .planes()
        .entity_indices
        .iter()
        .any(|head| *head != 0)
    {
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
    // PrepareEvents writes the shared predictedAxis Z for direct rows.
    // Marked paths (sub_48690) replace only X/Y; other linked constructors
    // use local positions and must not overwrite that retained scratch Z.
    let mut scratch_z = 0;
    for phase in 0..7 {
        for index in 1..features.len() {
            let row = features[index];
            if row.activation_group != -1 {
                continue;
            }
            let selected = match phase {
                0 => (row.kind, row.subtype) == (10, 82),
                1 => row.kind == 10 && matches!(row.subtype, 11 | 29 | 31 | 50 | 83..=85),
                2 => row.kind == 10 && matches!(row.subtype, 80 | 81),
                3 => (row.kind, row.subtype) == (14, 2),
                4 => (row.kind, row.subtype) == (10, 28),
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
            if !matches!(row.subtype, 28 | 29 | 31 | 50 | 80) {
                scratch_z = position.z;
            }
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
                    let properties = inputs.rings.get(&row.index).ok_or_else(|| {
                        format!(
                            "terrain ring row {} lacks typed construction content",
                            row.index
                        )
                    })?;
                    let profile = RingProfile::from(*properties);
                    if rings::construct(&mut arena, &mut terrain, position, profile).is_some() {
                        allocations += 1;
                    }
                }
                (10, 28 | 29 | 31 | 50) => {
                    let prepared = rings::authored::prepare_chain(
                        &mut arena,
                        &mut terrain,
                        &mut features,
                        row.index,
                        scratch_z,
                    )?;
                    allocations += prepared.source_slots.len();
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
    }
    Ok(CaveStartupWorld {
        terrain,
        arena,
        authored: features,
        allocation_count: allocations,
        buildings,
        effects: inputs.effects.clone(),
        particles: inputs.particles.to_vec().into_boxed_slice(),
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
                        hide = rings::step(arena, terrain, slot as u16, apply_damage)?.hide;
                    }
                    (27, 27..=29) => {
                        again = true;
                        rings::ribbon::step(arena, terrain, slot as u16)?;
                    }
                    (30, 32) => {
                        again = true;
                        rings::paths::step(arena, terrain, slot as u16)?;
                    }
                    (32, 34) | (51, 55) => {
                        again = true;
                        hide = rings::travelling::step(arena, terrain, slot as u16, apply_damage)?
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

fn apply_damage(arena: &mut SourceArena, damage: rings::AreaDamage) -> Result<u16, String> {
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
    // Startup has not allocated player spell holders; sub6D8B0 rejects this
    // class10 self owner for XP. Damage still precedes the geometry writes.
    crate::native_world::area_damage::apply(arena, None, None, request)
}

fn construct_house(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    feature: AuthoredFeature,
    building: &Building,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let (x, y, z) = level1_load_house_base(
        terrain.planes(),
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
    // Source action51 samples its live Z before edge smoothing. The final
    // sweep rebuilds topology without changing floor heights, so dispatch
    // completion before the combined sweep/smoothing transaction. Ordinary
    // countdown visits still consume their terrain writes before dispatch.
    if remaining <= 0 {
        house.dispatch_build_entry(terrain);
    }
    terrain.apply_house_construction_with_clearance(
        position,
        (remaining > 0).then_some(remaining),
        &building.footprint,
        building.parameter.flags & 4 == 0,
    )?;
    if remaining > 0 {
        house.dispatch_build_entry(terrain);
    }
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
    fn ordinary_cave_reset_constructs_geometry_before_flight_consumption() {
        for mission in [4, 6, 8, 12, 15, 16, 21, 24] {
            let relative = if mission == 8 {
                // This versioned owner export also retains the four ordinary
                // BUILD53 constructor inputs absent from old feature-only v1.
                "../../../generated/mc-bhq5-cave-reset-v2-level8".to_owned()
            } else if matches!(mission, 4 | 6) {
                format!("../../../generated/mc-bhq6-ordinary-20261024-b/level{mission}")
            } else {
                format!("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level{mission}")
            };
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
            let package = DecodedCampaignPackage::decode(
                &root,
                CampaignSelection::ordinary(mission).unwrap(),
            )
            .expect("ordinary owner package");
            assert!(
                supports_authored(package.authored_bytes()).unwrap(),
                "Mission{mission} must use the ordinary source-construction path"
            );
            let generated = package
                .generate_cave_startup()
                .expect("source feature constructors");
            let feature_oracle = crate::native_world::parse_terrain(
                package.member("terrain-post-features.bin").unwrap(),
            )
            .unwrap();
            let end = package.member("feature-end.bin").unwrap();
            assert_eq!(
                generated.terrain.terrain_rng(),
                word(end, 8).unwrap(),
                "Mission{mission} feature terrain RNG"
            );
            assert_eq!(
                generated.arena.economy.global_rng,
                dword(end, 12).unwrap(),
                "Mission{mission} feature allocation RNG"
            );
            let allocator = package.member("feature-allocator.bin").unwrap();
            let expected_free = (0..dword(allocator, 16).unwrap() as usize)
                .map(|index| word(allocator, 32 + 2 * index).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                generated.arena.economy.free_source_slots(),
                expected_free,
                "Mission{mission} feature free-stack order"
            );
            let planes = generated.terrain.planes();
            for (name, actual, expected) in [
                (
                    "material",
                    planes.terrain_types.as_ref(),
                    feature_oracle.terrain_types.as_ref(),
                ),
                (
                    "floor",
                    planes.ground_heights.as_ref(),
                    feature_oracle.ground_heights.as_ref(),
                ),
                (
                    "shading",
                    planes.shading.as_ref(),
                    feature_oracle.shading.as_ref(),
                ),
                (
                    "topology",
                    planes.angles.as_ref(),
                    feature_oracle.angles.as_ref(),
                ),
                (
                    "ceiling",
                    planes.secondary_heights.as_ref(),
                    feature_oracle.secondary_heights.as_ref(),
                ),
            ] {
                let differences = actual
                    .iter()
                    .zip(expected)
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .collect::<Vec<_>>();
                assert!(
                    differences.is_empty(),
                    "Mission{mission} feature {name}: {} mismatches; first {:?}",
                    differences.len(),
                    &differences[..differences.len().min(8)]
                );
            }
            assert_eq!(
                planes.entity_indices, feature_oracle.entity_indices,
                "Mission{mission} feature map heads"
            );
            let rows = package.member("entities-post-features.bin").unwrap();
            for row in generated.arena.entities.iter() {
                let start = 12 + usize::from(row.index) * 176;
                let expected = &rows[start..start + 176];
                assert_eq!(
                    row.active,
                    expected[2] != 0,
                    "Mission{mission} feature slot{} activity",
                    row.index
                );
                if row.active {
                    let differences = row
                        .record
                        .iter()
                        .zip(expected)
                        .enumerate()
                        .filter(|(_, (a, b))| a != b)
                        .collect::<Vec<_>>();
                    assert!(
                        differences.is_empty(),
                        "Mission{mission} retained slot{}: {:?}",
                        row.index,
                        differences
                    );
                }
            }
            let reset = generated
                .reset_terrain()
                .expect("ordinary terrain callbacks");
            if mission == 8 {
                // Pinned remc2, unchanged exporter after its first ordinary
                // UpdateEntities: four BUILD51 writes advance this RNG.
                assert_eq!(generated.terrain.terrain_rng(), 26043);
                assert_eq!(reset.terrain_rng(), 39990);
            }
            let oracle =
                crate::native_world::parse_terrain(package.member("terrain.bin").unwrap()).unwrap();
            let planes = reset.planes();
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
                    .collect::<Vec<_>>();
                assert!(
                    differences.is_empty(),
                    "Mission{mission} reset {name}: {} mismatches; first {:?}",
                    differences.len(),
                    &differences[..differences.len().min(8)]
                );
            }
            let world = LegacyTerrain::new(
                planes.terrain_types.to_vec(),
                planes.ground_heights.to_vec(),
                Some(planes.secondary_heights.to_vec()),
                planes.angles.to_vec(),
            )
            .expect("ordinary flight terrain");
            let mut open = 0;
            for y in 0..256 {
                for x in 0..256 {
                    let point = Position {
                        x: x << 8,
                        y: y << 8,
                        z: 0,
                    };
                    assert_eq!(world.ground_height(point), reset.ground_height(point));
                    assert_eq!(world.ceiling_height(point), reset.ceiling_height(point));
                    open += usize::from(
                        world.ceiling_height(point).unwrap() > world.ground_height(point),
                    );
                }
            }
            assert!(open > 0, "ordinary constructed cave has traversable rooms");
            eprintln!(
                "Mission{mission}: all five reset planes exact; {open} open floor/ceiling samples; {} feature allocations; terrain RNG {}",
                generated.allocation_count(),
                reset.terrain_rng()
            );
        }
    }

    #[test]
    fn mission_four_ordinary_startup_constructs_source_terrain_and_retained_rows() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261024-b/level4");
        let package =
            DecodedCampaignPackage::decode(&root, CampaignSelection::ordinary(4).unwrap())
                .expect("real Mission4 owner package");
        let generated = package
            .generate_cave_startup()
            .expect("ordinary source generation");
        let oracle = crate::native_world::parse_terrain(
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
        if generated.arena.economy.global_rng != dword(end, 12).unwrap() {
            failures.push(format!(
                "global RNG: {:#x} vs {:#x}",
                generated.arena.economy.global_rng,
                dword(end, 12).unwrap()
            ));
        }
        if generated.terrain.terrain_rng() != word(end, 8).unwrap() {
            failures.push(format!(
                "terrain RNG: {} vs {}",
                generated.terrain.terrain_rng(),
                word(end, 8).unwrap()
            ));
        }
        let rows = package.member("entities-post-features.bin").unwrap();
        let allocator = package.member("feature-allocator.bin").unwrap();
        let free_count = dword(allocator, 16).unwrap() as usize;
        let expected_free = (0..free_count)
            .map(|index| word(allocator, 32 + 2 * index).unwrap())
            .collect::<Vec<_>>();
        if generated.arena.economy.free_source_slots() != expected_free {
            failures.push("source free-stack order differs".into());
        }
        for row in generated.arena.entities.iter() {
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
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        let point = Position {
            x: 118 << 8,
            y: 108 << 8,
            z: 0,
        };
        let ordinary = generated
            .flight_world()
            .expect("ordinary constructed flight consumer");
        assert_eq!(
            ordinary.ground_height(point),
            generated.terrain.ground_height(point)
        );
        assert_eq!(
            ordinary.ceiling_height(point),
            generated.terrain.ceiling_height(point)
        );
        assert!(ordinary.ceiling_height(point).unwrap() > ordinary.ground_height(point));
        eprintln!(
            "Mission4 source feature boundary exact: all five planes, map heads, allocator, RNG and retained rows; {} allocations, {} live rows, collision floor/ceiling {}/{:?}",
            generated.allocation_count(),
            generated.live_entities().count(),
            ordinary.ground_height(point),
            ordinary.ceiling_height(point)
        );
        let mut session =
            crate::campaign::CampaignSession::open(&root, CampaignSelection::ordinary(4).unwrap())
                .expect("ordinary Mission4 game session consumes constructed geometry");
        assert_eq!(
            session.inspection().terrain_rng(),
            generated.terrain.terrain_rng()
        );
        assert_eq!(
            session.inspection().terrain().ground_height(point),
            ordinary.ground_height(point)
        );
        assert_eq!(
            session.inspection().terrain().ceiling_height(point),
            ordinary.ceiling_height(point)
        );
        let reset_hash = session.state_hash();
        let reset_terrain_hash = session.terrain_hash();
        session.step(crate::PlayerCommand::default(), 0);
        assert_eq!(session.tick(), 1);
        session.reset();
        assert_eq!(session.state_hash(), reset_hash);
        assert_eq!(session.terrain_hash(), reset_terrain_hash);
    }
}
