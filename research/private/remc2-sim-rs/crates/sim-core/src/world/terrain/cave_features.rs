//! Authored cave passages, rooms, shaped obstructions, and ground features.
//!
//! The source row is the only controller state. The arena owns allocation,
//! map links, common cadence and reclaim; this owner borrows just that row
//! and the live terrain. Terrain topology uses the existing source rebuild
//! and its independent RNG, so collision and presentation see the same writes.

use crate::level_definition::CaveFeatureDefinition;
use crate::native_world::{
    EntityAuthority, EntitySnapshot, EntitySpecialRelation, MapType, ParticleParameter,
    SessionTerrain, TerrainPlanes, add_building_to_terrain, building_topology_rebuild_required,
    rebuild_building_topology, terrain_index,
};
use crate::{
    FlightWorld, Position, SIN_16_16,
    campaign::source_arena::SourceArena,
    heading_for_positions, legacy_sqrt,
    legacy_terrain::{cave_perimeter_extrema, perturb_cave_ceiling},
    level1_mission::AuthoredFeature,
    move_entity,
    renderable_map_chain::RenderableMapSeed,
};

pub(crate) mod startup;

/// One successful NewEvent allocation. The caller installs retained particle
/// defaults before honoring `reclaim`, just as sub_50960 does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Allocation {
    pub(crate) source_slot: u16,
    pub(crate) particle_index: Option<u16>,
    pub(crate) reclaim: bool,
}

/// Runtime group producer (`sub_4A310`), distinct from startup PrepareEvents.
/// In particular, published80 is a hide-only row, published82 retains its
/// constructor's room defaults, and only84/85 undo the centered XY placement.
pub(crate) fn prepare_group(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    feature: AuthoredFeature,
    particles: &[ParticleParameter],
    properties: Option<CaveFeatureDefinition>,
) -> Result<Option<u16>, String> {
    if (feature.kind, feature.subtype) == (2, 7) {
        return Ok(None);
    }
    if feature.kind != 10 || !(80..=86).contains(&feature.subtype) {
        return Err("published feature does not belong to cave constructors".to_owned());
    }
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let Some(allocation) = allocate(arena, terrain, feature.subtype as u8, position)? else {
        return Ok(None);
    };
    if let Some(index) = allocation.particle_index {
        let Some(particle) = particles.get(usize::from(index)).copied() else {
            reclaim(arena, terrain, allocation.source_slot);
            return Err(format!("ground feature needs retained particle {index}"));
        };
        if let Err(error) = bind_particle(
            &mut arena.entities[usize::from(allocation.source_slot)],
            index,
            particle,
        ) {
            reclaim(arena, terrain, allocation.source_slot);
            return Err(error);
        }
    }
    if allocation.reclaim {
        reclaim(arena, terrain, allocation.source_slot);
        return Ok(None);
    }
    let source = &mut arena.entities[usize::from(allocation.source_slot)];
    match source.model {
        83 => write_axis(source, 0, feature.word_10 as i16),
        84 | 85 => {
            let mut position = source.position;
            position.x = position.x.wrapping_sub(128);
            position.y = position.y.wrapping_sub(128);
            position.z = feature.parameter_3 as i16;
            source.set_position(position);
            write_axis(source, 0, feature.word_10 as i16);
        }
        _ => {}
    }
    if let Some(properties) = properties {
        bind_definition(source, properties);
    }
    Ok(Some(allocation.source_slot))
}

fn startup_position(feature: AuthoredFeature, z: i16) -> Position {
    Position {
        x: feature.tile_x << 8,
        y: feature.tile_y << 8,
        z,
    }
}

/// SetEntityIndexAndRot_49CD0 consumes the retained package particle row.
/// The same lossless fields feed ordinary source-sprite observations.
pub(crate) fn bind_particle(
    source: &mut EntitySnapshot,
    index: u16,
    particle: ParticleParameter,
) -> Result<(), String> {
    let animation_limit = particle.animation_limit().ok_or_else(|| {
        format!(
            "cave particle {index} has unsupported render mode {}",
            particle.render_mode
        )
    })?;
    source.particle_parameter_index = Some(index);
    source.record[94..96].copy_from_slice(&index.to_le_bytes());
    source.record[96] = 0;
    source.record[97] = animation_limit;
    for (offset, value) in [
        (86, particle.rotation_speed / 2),
        (88, particle.speed / 2),
        (90, particle.speed / 2),
        (92, particle.rotation_speed / 2),
    ] {
        source.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

/// Ground86's immediate smoke descendant. The source-global draw happens
/// before NewEvent, even on exhaustion; the entity draw happens only on birth.
pub(crate) fn allocate_ground_smoke(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    particle: ParticleParameter,
) -> Result<Option<u16>, String> {
    arena.economy.advance_global_rng();
    let lifetime = (arena.economy.global_rng % 23 + 17) as i32;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    source.class = 10;
    source.model = 87;
    source.record[67] = 10;
    source.record[68] = 87;
    source.set_source_action(94);
    source.set_max_life(lifetime);
    source.set_life(lifetime);
    source.set_entity_rng(source.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
    source.set_control_flags((source.control_flags & 0xfffd_fff7) | 0x0002_0004);
    source.record[69] = 10;
    source.record[70] = 87;
    source.record[138..140].copy_from_slice(&30_i16.to_le_bytes());
    source.set_actual_speed((source.entity_rng % 53 + 51) as i16);
    if let Err(error) = bind_particle(&mut source, 67, particle) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: position.x,
        y: position.y,
    }) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

/// PrepareEvents' cave branch. The caller scans ordinary authored rows in
/// source order and shares this mutable working copy across generation phases.
/// Linked80 clears each visited stage tag, preventing duplicate construction
/// when a later authored record reaches the same path.
pub(crate) fn prepare_authored(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    features: &mut [AuthoredFeature],
    authored_index: u16,
) -> Result<Vec<Allocation>, String> {
    let feature = *features
        .get(usize::from(authored_index))
        .ok_or_else(|| format!("cave authored row {authored_index} is absent"))?;
    if feature.kind != 10 || !(80..=86).contains(&feature.subtype) {
        return Ok(Vec::new());
    }
    if feature.subtype != 80 {
        let position =
            startup_position(feature, terrain.ground_height(startup_position(feature, 0)));
        let Some(allocation) = allocate(arena, terrain, feature.subtype as u8, position)? else {
            return Ok(Vec::new());
        };
        bind_authored(
            &mut arena.entities[usize::from(allocation.source_slot)],
            [
                feature.parameter_1,
                feature.parameter_2,
                feature.parameter_3,
            ],
            feature.word_10,
        );
        return Ok(vec![allocation]);
    }
    if feature.stage_tag == 0 {
        return Ok(Vec::new());
    }
    let mut cursor = authored_index;
    let mut visited = std::collections::BTreeSet::new();
    loop {
        if !visited.insert(cursor) {
            return Err("cave path predecessor cycle".to_owned());
        }
        let row = features
            .get(usize::from(cursor))
            .ok_or_else(|| "cave path predecessor escapes authored table".to_owned())?;
        if (row.kind, row.subtype) != (10, 80) {
            return Err("cave path predecessor changes feature kind".to_owned());
        }
        if row.parameter_1 == 0 {
            break;
        }
        cursor = row.parameter_1;
    }
    visited.clear();
    let mut allocations = Vec::new();
    loop {
        if !visited.insert(cursor) {
            return Err("cave path successor cycle".to_owned());
        }
        let current = *features
            .get(usize::from(cursor))
            .ok_or_else(|| "cave path successor escapes authored table".to_owned())?;
        if (current.kind, current.subtype) != (10, 80) {
            return Err("cave path successor changes feature kind".to_owned());
        }
        features[usize::from(cursor)].stage_tag = 0;
        if current.parameter_2 == 0 {
            break;
        }
        let next = *features
            .get(usize::from(current.parameter_2))
            .ok_or_else(|| "cave path target escapes authored table".to_owned())?;
        let widths = ((current.parameter_3 as u8 & 15) << 4) | (next.parameter_3 as u8 & 15);
        if let Some(allocation) = allocate(arena, terrain, 81, startup_position(current, 0))? {
            bind_link(
                &mut arena.entities[usize::from(allocation.source_slot)],
                startup_position(next, 0),
                widths,
            );
            allocations.push(allocation);
        }
        cursor = current.parameter_2;
    }
    Ok(allocations)
}

/// One ascending ApplyEvents scan of the cave-owned rows. The shared
/// GenerateEvents orchestrator advances the global RNG once per barrier and
/// composes other families in the same scan. No RNG draw occurs per retry.
pub(crate) fn apply_generation_pass(arena: &mut SourceArena, terrain: &mut SessionTerrain) -> bool {
    let mut ran = false;
    for slot in 1..arena.entities.len() {
        let source = &mut arena.entities[slot];
        if !source.active || source.class != 10 || !(80..=85).contains(&source.model) {
            continue;
        }
        ran = true;
        let visit = step(terrain, source);
        source.set_phase(source.phase.wrapping_add(1));
        if visit.hide {
            source.disable_drawing();
            reclaim(arena, terrain, slot as u16);
        }
    }
    ran
}

/// Ordinary constructor producer into the SAME source arena the scheduler
/// visits. A cave-only rejection happens before NewEvent, not after it.
pub(crate) fn allocate(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    model: u8,
    position: Position,
) -> Result<Option<Allocation>, String> {
    if !enabled(terrain, model) {
        return Ok(None);
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = new_event(slot, arena.economy.new_event_rng(slot), position);
    let (map_linked, particle_index, reclaim) = match construct(&mut source, terrain, model) {
        Construction::Retain {
            map_linked,
            particle_index,
        } => (map_linked, particle_index, false),
        Construction::Reclaim {
            map_linked,
            particle_index,
        } => (map_linked, particle_index, true),
        Construction::Unsupported | Construction::NotCave => unreachable!("constructor preflight"),
    };
    if map_linked {
        if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
            entity_index: slot,
            active: true,
            x: source.position.x,
            y: source.position.y,
        }) {
            arena.economy.release_slot(slot, Some(source.entity_rng));
            return Err(error);
        }
        source.set_control_flags(source.control_flags | 4);
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    if map_linked {
        synchronize_links(arena, terrain, slot);
    }
    Ok(Some(Allocation {
        source_slot: slot,
        particle_index,
        reclaim,
    }))
}

/// Immediate sub_57F20 (constructor86 failure or ApplyEvents terminal row).
/// Runtime UpdateEntities continues to use its existing following-pass reclaim.
pub(crate) fn reclaim(arena: &mut SourceArena, terrain: &mut SessionTerrain, slot: u16) {
    let rng = arena.entities[usize::from(slot)].entity_rng;
    arena.economy.map_chains.set_map_entity_unlink(slot);
    synchronize_links(arena, terrain, slot);
    arena.entities[usize::from(slot)].reclaim_source_row();
    arena.economy.release_slot(slot, Some(rng));
}

pub(crate) fn synchronize_links(arena: &mut SourceArena, terrain: &mut SessionTerrain, slot: u16) {
    // Source map surgery changes neighboring fixed rows as well as the head.
    // Dynamic families retain their own projections in the shared map owner.
    for source in &mut arena.entities {
        source.record[26..28].copy_from_slice(
            &arena
                .economy
                .map_chains
                .old_map_entity(source.index)
                .to_le_bytes(),
        );
        source.record[28..30].copy_from_slice(
            &arena
                .economy
                .map_chains
                .next_entity(source.index)
                .to_le_bytes(),
        );
    }
    let source = &arena.entities[usize::from(slot)];
    let tile = index(source.position);
    terrain.disaster_parts().0.entity_indices[tile] =
        arena.economy.map_chains.tile_head(tile as u16) as i16;
}

pub(crate) fn new_event(slot: u16, rng: u16, position: Position) -> EntitySnapshot {
    let mut record = vec![0; 176].into_boxed_slice();
    record[0..2].copy_from_slice(&slot.to_le_bytes());
    record[2] = 1;
    record[30..32].copy_from_slice(&slot.to_le_bytes());
    record[61] = 250; // byte39: -6
    record[66] = slot as u8;
    record[69] = 255; // xtype
    record[70] = 255; // xsubtype
    record[71] = 10;
    record[164..168].copy_from_slice(&60_u32.to_le_bytes()); // str_D7BD6[59]
    record[168] = 2;
    record[172..176].copy_from_slice(&1_u32.to_le_bytes()); // unk_F42B0x
    let mut source = EntitySnapshot {
        index: slot,
        source_id: slot,
        authority: EntityAuthority::FrozenObserved,
        active: true,
        max_life: 300,
        life: 0,
        next_relation: None,
        class: 0,
        model: 0,
        control_flags: 8,
        entity_rng: rng,
        action: 0,
        action_marker: 0,
        level_gate: 0,
        position,
        yaw: 0,
        pitch: 0,
        fov: 0,
        phase: slot as u8,
        subspell_index: 100,
        byte_3b: 0,
        byte_3c: 0,
        font_type_index: 0,
        active_value: 0,
        activation_value: 0,
        mana_regen: 0,
        actual_speed: 16,
        max_mana: 0,
        mana_cost: 0,
        mana: 0,
        particle_parameter_index: Some(0),
        parameter_relation: Some(59),
        special_relation: EntitySpecialRelation::GlobalSpecialState,
        record,
    };
    source.set_max_life(300);
    source.set_control_flags(8);
    source.set_entity_rng(rng);
    source.set_actual_speed(16);
    source.set_subspell_index(100);
    source.set_position(position);
    source
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Construction {
    Unsupported,
    NotCave,
    Retain {
        map_linked: bool,
        particle_index: Option<u16>,
    },
    Reclaim {
        map_linked: bool,
        particle_index: Option<u16>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Visit {
    pub(crate) hide: bool,
    pub(crate) terrain_mutated: bool,
    /// `sub_31120` allocates model87, THEN draws the global RNG for sound.
    pub(crate) spawn_model87: bool,
    pub(crate) random_sound_pending: bool,
    pub(crate) animate: bool,
    pub(crate) sound_id: Option<u16>,
}

/// Production callback, including ground86's synchronous child allocation
/// and global sound draw before the parent's postdecrement/animation. The
/// scheduler supplies the retained presentation table, publishes `sound_id`,
/// and performs its common phase epilogue and following-pass reclamation.
pub(crate) fn visit_source(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
    particles: &[ParticleParameter],
) -> Result<Visit, String> {
    let source = arena
        .entities
        .get(usize::from(source_slot))
        .filter(|source| source.active)
        .ok_or_else(|| format!("cave callback source slot {source_slot} is not live"))?;
    let old_tile = index(source.position);
    let birth = source.action == 93
        && terrain.ground_height(source.position) == source.position.z
        && source.max_life.wrapping_sub(5) == source.life;
    let mut sound_id = None;
    if birth {
        let position = source.position;
        let sound = source
            .particle_parameter_index
            .ok_or_else(|| "ground feature has no retained particle binding".to_owned())?
            .checked_sub(282)
            .ok_or_else(|| "ground feature particle cannot select its source sound".to_owned())?;
        let particle = particles
            .get(67)
            .copied()
            .ok_or_else(|| "ground feature smoke needs retained particle67".to_owned())?;
        allocate_ground_smoke(arena, terrain, position, particle)?;
        arena.economy.advance_global_rng();
        if arena.economy.global_rng & 1 == 0 {
            sound_id = Some(sound);
        }
    }
    let source = &mut arena.entities[usize::from(source_slot)];
    let mut visit = step(terrain, source);
    visit.sound_id = sound_id;
    if source.control_flags & 4 != 0 {
        arena.economy.map_chains.sync_entity(RenderableMapSeed {
            entity_index: source_slot,
            active: true,
            x: source.position.x,
            y: source.position.y,
        })?;
        synchronize_links(arena, terrain, source_slot);
        terrain.disaster_parts().0.entity_indices[old_tile] =
            arena.economy.map_chains.tile_head(old_tile as u16) as i16;
    }
    Ok(visit)
}

/// Check before calling NewEvent: the six cave-only constructors do not
/// allocate or advance allocation/RNG state on an outdoor map.
pub(crate) fn enabled(terrain: &SessionTerrain, model: u8) -> bool {
    model == 86 || ((80..=85).contains(&model) && terrain.map_type() == MapType::Cave)
}

/// Initialize a freshly allocated NewEvent row at its requested position.
/// The caller applies the returned particle defaults and map linkage even
/// when model86 subsequently requests immediate reclaim.
pub(crate) fn construct(
    source: &mut EntitySnapshot,
    terrain: &SessionTerrain,
    model: u8,
) -> Construction {
    if !(80..=86).contains(&model) {
        return Construction::Unsupported;
    }
    if !enabled(terrain, model) {
        return Construction::NotCave;
    }
    source.class = 10;
    source.model = model;
    source.record[67] = 10;
    source.record[68] = model;
    source.set_source_action(model + 7);
    if model != 82 {
        source.set_control_flags(source.control_flags & !8);
    }
    let mut map_linked = false;
    match model {
        80 => {
            source.set_max_life(0);
            source.set_life(0);
            map_linked = true;
        }
        81 => {
            source.set_max_life(0);
            source.set_life(0);
            source.set_actual_speed(256);
            source.set_source_byte_46(2);
        }
        82 => {
            source.set_max_life(0);
            source.set_source_byte_46(2);
            source.record[71] = 3;
            source.record[72] = 3;
        }
        83..=85 => {
            source.set_life(16);
            source.set_source_byte_46(0);
            write_axis(source, 0, 2);
            write_axis(source, 2, 0);
            source.set_control_flags(source.control_flags | 1);
            let mut position = source.position;
            position.z = 0;
            source.set_position(position);
        }
        86 => {
            source.set_max_life(9);
            source.set_life(9);
            source.set_control_flags((source.control_flags & 0xfffd_fff7) | 0x0002_0000);
            let mut position = source.position;
            position.z = terrain.ground_height(position);
            source.set_position(position);
            source.set_entity_rng(source.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
            let particle_index = Some(332 + source.entity_rng % 3);
            return if terrain.planes().angles[index(position)] & 15 == 0 {
                Construction::Retain {
                    map_linked: true,
                    particle_index,
                }
            } else {
                Construction::Reclaim {
                    map_linked: true,
                    particle_index,
                }
            };
        }
        _ => unreachable!(),
    }
    Construction::Retain {
        map_linked,
        particle_index: None,
    }
}

/// Apply the authored-only parameter overrides from PrepareEvents_49540.
pub(crate) fn bind_authored(source: &mut EntitySnapshot, parameters: [u16; 3], radius: u16) {
    match source.model {
        82 => {
            source.record[71] = parameters[0] as u8;
            source.record[72] = parameters[1] as u8;
            source.set_source_byte_46(parameters[2] as u8);
        }
        83..=85 => {
            write_axis(source, 0, radius as i16);
            if source.model != 83 {
                let mut position = source.position;
                position.z = parameters[2] as i16;
                source.set_position(position);
            }
        }
        _ => {}
    }
}

/// Linked authored80 calls constructor81 at the PREVIOUS point, with the
/// current point as target. Each endpoint contributes its low width nibble.
pub(crate) fn bind_link(source: &mut EntitySnapshot, target: Position, widths: u8) {
    write_axis(source, 0, target.x as i16);
    write_axis(source, 1, target.y as i16);
    write_axis(source, 2, target.z);
    source.set_source_byte_46(widths);
}

/// Install the retained semantic properties on the authoritative controller.
/// The MC2 adapter chooses startup or published defaults; geometry and later
/// callbacks read these exact words rather than another mutable property copy.
pub(crate) fn bind_definition(source: &mut EntitySnapshot, properties: CaveFeatureDefinition) {
    match properties {
        CaveFeatureDefinition::PathNode { .. } => {}
        CaveFeatureDefinition::Passage {
            start_radius_tiles,
            end_radius_tiles,
            destination,
        } => {
            bind_link(
                source,
                Position {
                    x: destination.x << 8,
                    y: destination.y << 8,
                    z: 0,
                },
                ((start_radius_tiles - 2) << 4) | (end_radius_tiles - 2),
            );
        }
        CaveFeatureDefinition::Room {
            half_width_tiles,
            half_depth_tiles,
            clearance_steps,
        } => {
            source.record[71] = half_width_tiles;
            source.record[72] = half_depth_tiles;
            source.set_source_byte_46(clearance_steps);
        }
        CaveFeatureDefinition::RoundObstruction { radius_tiles } => {
            write_axis(source, 0, radius_tiles)
        }
        CaveFeatureDefinition::CeilingPendant {
            radius_tiles,
            height_factor,
        }
        | CaveFeatureDefinition::FloorMound {
            radius_tiles,
            height_factor,
        } => {
            write_axis(source, 0, radius_tiles);
            let mut position = source.position;
            position.z = height_factor;
            source.set_position(position);
        }
        CaveFeatureDefinition::GroundVent { lifetime } => {
            source.set_max_life(lifetime);
            source.set_life(lifetime);
        }
    }
}

pub(crate) fn step(terrain: &mut SessionTerrain, source: &mut EntitySnapshot) -> Visit {
    let mut visit = Visit::default();
    match source.action {
        87 => visit.hide = true, // Dynamic80 only hides; authored80 is linked81.
        88 => {
            passage(terrain, source);
            visit.hide = true;
            visit.terrain_mutated = true;
        }
        89 => {
            room(terrain, source);
            visit.hide = true;
            visit.terrain_mutated = true;
        }
        90..=92 => {
            source.set_life(source.life.wrapping_sub(1));
            if source.life <= 0 {
                visit.hide = true;
            } else {
                visit.terrain_mutated = shape(terrain, source);
            }
        }
        93 => {
            if terrain.ground_height(source.position) != source.position.z {
                visit.hide = true;
            } else {
                if source.max_life.wrapping_sub(5) == source.life {
                    visit.spawn_model87 = true;
                    visit.random_sound_pending = true;
                }
                let old_life = source.life;
                source.set_life(old_life.wrapping_sub(1));
                visit.hide = old_life < 0;
                visit.animate = !visit.hide;
                if visit.animate && source.record[96] < source.record[97] {
                    source.record[96] += 1;
                }
            }
        }
        94 => {
            crate::world::effects::step_smoke(source, terrain);
            visit.hide = source.control_flags & 0x400 != 0;
        }
        _ => {}
    }
    if visit.terrain_mutated {
        let (_, _, _, sequence) = terrain.disaster_parts();
        *sequence = sequence.wrapping_add(1);
    }
    visit
}

fn index(position: Position) -> usize {
    terrain_index((position.x >> 8) as u8, (position.y >> 8) as u8)
}

fn axis(source: &EntitySnapshot, component: usize) -> i16 {
    let offset = 158 + 2 * component;
    i16::from_le_bytes([source.record[offset], source.record[offset + 1]])
}

fn write_axis(source: &mut EntitySnapshot, component: usize, value: i16) {
    let offset = 158 + 2 * component;
    source.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn distance(from: Position, to: Position) -> i32 {
    let dx = i32::from(to.x.wrapping_sub(from.x) as i16);
    let dy = i32::from(to.y.wrapping_sub(from.y) as i16);
    legacy_sqrt((dx * dx) as u32 + (dy * dy) as u32) as i32
}

fn normalize(planes: &mut TerrainPlanes, index: usize, reset_ceiling: bool) {
    if planes.secondary_heights[index] > planes.ground_heights[index] {
        planes.angles[index] &= 0xf7;
    } else {
        if reset_ceiling {
            planes.secondary_heights[index] = planes.ground_heights[index].wrapping_sub(1);
        }
        planes.angles[index] |= 8;
    }
}

/// `sub_570F0(x,y,height,0,force_material,preserve_zero_neighbors)`.
pub(crate) fn write_ground(
    terrain: &mut SessionTerrain,
    x: u8,
    y: u8,
    height: i32,
    force_material: bool,
    preserve_zero_neighbors: bool,
) {
    let (planes, map_type, terrain_rng, _) = terrain.disaster_parts();
    let index = terrain_index(x, y);
    let height = height.clamp(0, 255) as u8;
    planes.ground_heights[index] = height;
    if force_material || building_topology_rebuild_required(planes.terrain_types[index]) {
        planes.angles[index] = (planes.angles[index] & 0xf8) | 1;
    }
    if height == 0
        && (!preserve_zero_neighbors
            || (-1_i8..=1).all(|dy| {
                (-1_i8..=1).all(|dx| {
                    (dx == 0 && dy == 0)
                        || !matches!(
                            planes.angles[terrain_index(
                                x.wrapping_add_signed(dx),
                                y.wrapping_add_signed(dy)
                            )] & 7,
                            2 | 3 | 5
                        )
                })
            }))
    {
        planes.angles[index] &= 0xf0;
    }
    add_building_to_terrain(planes, x, y, map_type, terrain_rng);
}

/// Source sub_34B00: repair closed perimeter cells in literal call order.
fn repair_border(terrain: &mut SessionTerrain, x: u8, y: u8, width: i32, height: i32) {
    for column in 0..width {
        let x = x.wrapping_add(column as u8);
        repair_cell(terrain, x, y, true);
        repair_cell(terrain, x, y.wrapping_add(height as u8), false);
    }
    for row in 0..height {
        let y = y.wrapping_add(row as u8);
        repair_cell(terrain, x, y, true);
        repair_cell(terrain, x.wrapping_add(width as u8), y, false);
    }
}

fn repair_cell(terrain: &mut SessionTerrain, x: u8, y: u8, mark_type: bool) {
    let index = terrain_index(x, y);
    let (planes, map_type, terrain_rng, _) = terrain.disaster_parts();
    if planes.angles[index] & 8 == 0 {
        return;
    }
    planes.angles[index] = (planes.angles[index] & 0xf8) | 1;
    if mark_type {
        planes.terrain_types[index] = 1;
    }
    rebuild_building_topology(planes, x, y, x, y, map_type, terrain_rng);
}

fn perturb(terrain: &mut SessionTerrain, x: u8, y: u8, width: u16, height: u16) {
    let (planes, _, _, _) = terrain.disaster_parts();
    perturb_cave_ceiling(
        &planes.ground_heights,
        &mut planes.secondary_heights,
        &mut planes.angles,
        x,
        y,
        width,
        height,
    );
}

fn room(terrain: &mut SessionTerrain, source: &EntitySnapshot) {
    let half_width = i32::from(source.record[71]);
    let half_height = i32::from(source.record[72]);
    let width = 2 * half_width;
    let height = 2 * half_height;
    let x = ((source.position.x >> 8) as u8).wrapping_sub(half_width as u8);
    let y = ((source.position.y >> 8) as u8).wrapping_sub(half_height as u8);
    let floor = cave_perimeter_extrema(
        &terrain.planes().ground_heights,
        x,
        y,
        width as u16,
        height as u16,
    )
    .0;
    // Deliberately the FLOOR maximum, not ceiling maximum (sub_48E90).
    let ceiling = cave_perimeter_extrema(
        &terrain.planes().ground_heights,
        x,
        y,
        width as u16,
        height as u16,
    )
    .1;
    let clearance = 3 * i32::from(source.record[74]);
    for row in 0..height {
        for column in 0..width {
            let x = x.wrapping_add(column as u8);
            let y = y.wrapping_add(row as u8);
            let index = terrain_index(x, y);
            let target = (floor - clearance).clamp(0, 254);
            if i32::from(terrain.planes().ground_heights[index]) > target {
                write_ground(terrain, x, y, target, false, false);
            }
            let target = (ceiling + clearance).clamp(0, 254) as u8;
            let planes = terrain.disaster_parts().0;
            if planes.secondary_heights[index] < target {
                planes.secondary_heights[index] = target;
            }
            normalize(planes, index, true);
        }
    }
    repair_border(
        terrain,
        x.wrapping_sub(1),
        y.wrapping_sub(1),
        width + 1,
        height + 1,
    );
    perturb(terrain, x, y, width as u16, height as u16);
}

fn midpoint(terrain: &SessionTerrain, position: Position) -> u8 {
    let index = index(position);
    ((u16::from(terrain.planes().ground_heights[index])
        + u16::from(terrain.planes().secondary_heights[index]))
        / 2)
    .min(254) as u8
}

fn passage(terrain: &mut SessionTerrain, source: &EntitySnapshot) {
    let target = Position {
        x: axis(source, 0) as u16,
        y: axis(source, 1) as u16,
        z: axis(source, 2),
    };
    let steps = distance(source.position, target) / 85;
    let heading = heading_for_positions(source.position, target) as i16;
    let first_radius = (i32::from(source.record[74] >> 4) << 8) + 512;
    let final_radius = (i32::from(source.record[74] & 15) << 8) + 512;
    let mut ahead = source.position;
    let mut midpoints = [0_u8; 32];
    for midpoint_value in &mut midpoints {
        *midpoint_value = midpoint(terrain, ahead);
        move_entity(&mut ahead, heading, 0, 85);
    }
    let mut position = source.position;
    for step in 0..steps {
        let radius = first_radius + step * ((final_radius - first_radius) / steps);
        let radius_squared = radius * radius;
        let diameter = (2 * radius + 128) >> 8;
        let half = (2 * radius + 128) >> 9;
        let center_x = (position.x.wrapping_add(128) >> 8) as u8;
        let center_y = (position.y.wrapping_add(128) >> 8) as u8;
        let x = center_x.wrapping_sub(half as u8);
        let y = center_y.wrapping_sub(half as u8);
        for row in 0..diameter {
            for column in 0..diameter {
                let x = x.wrapping_add(column as u8);
                let y = y.wrapping_add(row as u8);
                let index = terrain_index(x, y);
                let dx = i32::from(x.wrapping_sub(center_x) as i8) << 8;
                let dy = i32::from(y.wrapping_sub(center_y) as i8) << 8;
                let squared = dx * dx + dy * dy;
                if squared <= radius_squared {
                    let clearance = (legacy_sqrt((radius_squared - squared) as u32) >> 5) as i32;
                    let floor = (i32::from(midpoints[0]) - clearance).clamp(0, 254);
                    if i32::from(terrain.planes().ground_heights[index]) > floor {
                        write_ground(terrain, x, y, floor, false, true);
                    }
                    let ceiling = (i32::from(midpoints[0]) + clearance).clamp(0, 254) as u8;
                    let planes = terrain.disaster_parts().0;
                    if planes.secondary_heights[index] < ceiling {
                        planes.secondary_heights[index] = ceiling;
                    }
                }
                normalize(terrain.disaster_parts().0, index, true);
            }
        }
        repair_border(
            terrain,
            x.wrapping_sub(1),
            y.wrapping_sub(1),
            diameter + 1,
            diameter + 1,
        );
        move_entity(&mut position, heading, 0, 85);
        // Source shifts scratch bytes 2..32 from 3..33. Our slice starts
        // at source byte 2, rather than including its two-byte prefix.
        midpoints.copy_within(1..32, 0);
        midpoints[31] = midpoint(terrain, ahead);
        move_entity(&mut ahead, heading, 0, 85);
    }
}

fn shape(terrain: &mut SessionTerrain, source: &mut EntitySnapshot) -> bool {
    let radius = i32::from(axis(source, 0));
    let diameter = 2 * radius;
    let x = ((source.position.x.wrapping_add(128) >> 8) as u8).wrapping_sub(radius as u8);
    let y = ((source.position.y.wrapping_add(128) >> 8) as u8).wrapping_sub(radius as u8);
    let phase = source.record[74];
    if phase == 0 {
        let floor = cave_perimeter_extrema(
            &terrain.planes().ground_heights,
            x,
            y,
            diameter as u16,
            diameter as u16,
        );
        let ceiling = cave_perimeter_extrema(
            &terrain.planes().secondary_heights,
            x,
            y,
            diameter as u16,
            diameter as u16,
        );
        let (base, span) = match source.model {
            83 => (ceiling.1, ceiling.1 - floor.0),
            84 => (ceiling.1, ceiling.1 - floor.1 - 1),
            85 => (floor.0, ceiling.0 - floor.0 - 1),
            _ => return false,
        };
        write_axis(source, 2, base as i16);
        let mut position = source.position;
        if source.model == 83 {
            position.z = floor.0 as i16;
            source.set_source_byte_46(if span <= 0 { 2 } else { 1 });
        } else if span <= 0 {
            source.set_source_byte_46(2);
        } else {
            let offset = if position.z != 0 {
                (51 * i32::from(position.z) * span / 256) as u32
            } else {
                source.set_entity_rng(source.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
                u32::from(source.entity_rng) % span as u32
            };
            position.z = if source.model == 84 {
                (base as i16).wrapping_sub(offset as i16)
            } else {
                (base as i16).wrapping_add(offset as i16)
            };
            source.set_source_byte_46(1);
        }
        source.set_position(position);
        return false;
    }
    if phase == 2 {
        if source.model == 84 {
            perturb(terrain, x, y, diameter as u16, diameter as u16);
        }
        source.set_life(0);
        return source.model == 84;
    }
    if phase != 1 || radius <= 0 {
        return false;
    }
    let world_radius = radius << 8;
    let base = i32::from(axis(source, 2));
    let span = if source.model == 83 {
        base - i32::from(source.position.z)
    } else {
        (i32::from(source.position.z) - base).abs()
    };
    for row in 0..diameter {
        for column in 0..diameter {
            let x = x.wrapping_add(column as u8);
            let y = y.wrapping_add(row as u8);
            let position = Position {
                x: u16::from(x) << 8,
                y: u16::from(y) << 8,
                z: 0,
            };
            let distance = distance(source.position, position);
            if distance >= world_radius {
                continue;
            }
            let index = terrain_index(x, y);
            let weight =
                (65_536 + SIN_16_16[512 + ((distance << 10) / world_radius) as usize]) >> 1;
            let amplitude = (span * weight) >> 16;
            let floor = i32::from(terrain.planes().ground_heights[index]);
            let ceiling = i32::from(terrain.planes().secondary_heights[index]);
            match source.model {
                83 => {
                    let target = (i32::from(source.position.z) + amplitude).min(254);
                    if target > floor {
                        write_ground(
                            terrain,
                            x,
                            y,
                            floor + (target - floor) / source.life,
                            distance <= 192 * radius,
                            true,
                        );
                    }
                    let target = (base - amplitude).max(0);
                    if target < ceiling {
                        terrain.disaster_parts().0.secondary_heights[index] =
                            (ceiling - (ceiling - target) / source.life) as u8;
                    }
                    normalize(terrain.disaster_parts().0, index, false);
                }
                84 => {
                    let target = (base - amplitude).max(0);
                    let next = ceiling - (ceiling - target) / source.life;
                    if next < ceiling {
                        terrain.disaster_parts().0.secondary_heights[index] = next as u8;
                    }
                    normalize(terrain.disaster_parts().0, index, true);
                }
                85 => {
                    let target = (base + amplitude).min(254);
                    let next = floor + (target - floor) / source.life;
                    if next > floor {
                        write_ground(terrain, x, y, next, distance <= 192 * radius, true);
                    }
                    normalize(terrain.disaster_parts().0, index, true);
                }
                _ => {}
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::Level2EconomyState;

    fn terrain() -> SessionTerrain {
        SessionTerrain::from_planes(
            MapType::Cave,
            TerrainPlanes {
                terrain_types: vec![1; 65_536].into_boxed_slice(),
                ground_heights: vec![40; 65_536].into_boxed_slice(),
                shading: vec![32; 65_536].into_boxed_slice(),
                angles: vec![0; 65_536].into_boxed_slice(),
                secondary_heights: vec![80; 65_536].into_boxed_slice(),
                entity_indices: vec![0; 65_536].into_boxed_slice(),
            },
            12_345,
        )
    }

    fn arena(terrain: &SessionTerrain) -> SourceArena {
        let entities = (0..1_000)
            .map(|slot| {
                let mut source = new_event(slot, 0, Position::default());
                source.reclaim_source_row();
                source
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let economy =
            Level2EconomyState::from_parts(terrain.clone(), &entities, 0, 0, 0, 0, 12_345)
                .expect("empty source arena");
        SourceArena::new(entities, economy)
    }

    #[test]
    fn full_arena_cave_replacement_publishes_the_old_cell_before_unlinked_construction() {
        let mut terrain = terrain();
        let mut arena = arena(&terrain);
        let old_position = Position {
            x: 10 << 8,
            y: 10 << 8,
            z: 0,
        };
        let new_position = Position {
            x: 30 << 8,
            y: 30 << 8,
            z: 0,
        };
        let vent = allocate(&mut arena, &mut terrain, 86, old_position)
            .expect("ordinary mapped vent")
            .expect("first free source slot");
        assert_eq!(vent.source_slot, 1);
        assert_eq!(terrain.planes().entity_indices[index(old_position)], 1);
        while arena.economy.free_slot_count() != 0 {
            allocate(&mut arena, &mut terrain, 82, new_position)
                .expect("ordinary room allocation")
                .expect("remaining free source slot");
        }
        // Source49F90 freezes replacement addresses before a group starts;
        // NewEvent consumes that snapshot rather than rescanning newborns.
        arena.rebuild_source_free_arena();
        let replacement = allocate(&mut arena, &mut terrain, 83, new_position)
            .expect("ordinary source fallback")
            .expect("mapped replaceable vent is reused");
        assert_eq!(replacement.source_slot, vent.source_slot);
        assert_eq!(arena.entities[1].model, 83);
        assert!(!arena.economy.map_chains.is_linked(1));
        assert_eq!(
            arena
                .economy
                .map_chains
                .tile_head(index(old_position) as u16),
            0
        );
        assert_eq!(terrain.planes().entity_indices[index(old_position)], 0);
        assert_eq!(terrain.planes().entity_indices[index(new_position)], 0);
        let rng = arena.economy.global_rng;
        assert!(
            allocate(&mut arena, &mut terrain, 86, old_position)
                .expect("no replaceable source row remains")
                .is_none()
        );
        assert_eq!(arena.economy.global_rng, rng);
    }

    #[test]
    fn cave_constructor_defaults_and_immediate_failed_ground_feature_reclaim() {
        let mut terrain = terrain();
        let mut arena = arena(&terrain);
        let position = Position {
            x: 10 << 8,
            y: 10 << 8,
            z: 700,
        };
        let mut row = new_event(3, 99, position);
        assert_eq!(
            construct(&mut row, &terrain, 82),
            Construction::Retain {
                map_linked: false,
                particle_index: None
            }
        );
        assert_eq!(
            (row.class, row.model, row.action, row.life, row.max_life),
            (10, 82, 89, 0, 0)
        );
        assert_eq!(
            (
                row.control_flags,
                row.record[71],
                row.record[72],
                row.record[74]
            ),
            (8, 3, 3, 2)
        );
        terrain = SessionTerrain::from_planes(
            MapType::Day,
            terrain.planes().clone(),
            terrain.terrain_rng(),
        );
        let free = arena.economy.free_slot_count();
        assert!(
            allocate(&mut arena, &mut terrain, 82, position)
                .expect("outdoor rejection")
                .is_none()
        );
        assert_eq!(arena.economy.free_slot_count(), free);
        terrain.disaster_parts().0.angles[index(position)] = 8;
        let allocation = allocate(&mut arena, &mut terrain, 86, position)
            .expect("ground feature")
            .expect("allocated before eligibility");
        assert!(allocation.reclaim);
        assert!((332..=334).contains(&allocation.particle_index.expect("particle before reclaim")));
        assert!(arena.economy.map_chains.is_linked(allocation.source_slot));
        reclaim(&mut arena, &mut terrain, allocation.source_slot);
        assert!(!arena.entities[usize::from(allocation.source_slot)].active);
        assert_eq!(arena.economy.free_slot_count(), free);
        assert_eq!(terrain.planes().entity_indices[index(position)], 0);
    }

    #[test]
    fn published_cave_features_keep_group_defaults_and_source_coordinate_overrides() {
        let mut terrain = terrain();
        let mut arena = arena(&terrain);
        let feature = |model| AuthoredFeature {
            index: 1,
            kind: 10,
            subtype: model,
            tile_x: 10,
            tile_y: 12,
            activation_group: 51,
            word_10: 7,
            stage_tag: 51,
            parameter_1: 15,
            parameter_2: 10,
            parameter_3: 2,
        };
        let marker = prepare_group(&mut arena, &mut terrain, feature(80), &[], None)
            .expect("published marker")
            .expect("marker row");
        assert_eq!(
            arena.entities[usize::from(marker)].model,
            80,
            "group publication never walks the startup linked path"
        );
        let room = prepare_group(&mut arena, &mut terrain, feature(82), &[], None)
            .expect("published room")
            .expect("room row");
        let room = &arena.entities[usize::from(room)];
        assert_eq!(
            (room.record[71], room.record[72], room.record[74]),
            (3, 3, 2),
            "sub4A310 does not copy room parameters"
        );
        assert_eq!(
            (room.position.x, room.position.y),
            ((10 << 8) + 128, (12 << 8) + 128)
        );
        for model in 83..=85 {
            let slot = prepare_group(&mut arena, &mut terrain, feature(model), &[], None)
                .expect("published shaped feature")
                .expect("shape row");
            let row = &arena.entities[usize::from(slot)];
            assert_eq!(axis(row, 0), 7);
            let offset = if model == 83 { 128 } else { 0 };
            assert_eq!(
                (row.position.x, row.position.y),
                ((10 << 8) + offset, (12 << 8) + offset)
            );
            assert_eq!(row.position.z, if model == 83 { 0 } else { 2 });
        }
        let mut barrel = feature(80);
        barrel.kind = 2;
        barrel.subtype = 7;
        let free = arena.economy.free_slot_count();
        assert!(
            prepare_group(&mut arena, &mut terrain, barrel, &[], None)
                .expect("source cave barrel no-op")
                .is_none()
        );
        assert_eq!(arena.economy.free_slot_count(), free);
        let configured = prepare_group(
            &mut arena,
            &mut terrain,
            feature(82),
            &[],
            Some(CaveFeatureDefinition::Room {
                half_width_tiles: 6,
                half_depth_tiles: 7,
                clearance_steps: 8,
            }),
        )
        .expect("typed room producer")
        .expect("typed room birth");
        let configured = &arena.entities[usize::from(configured)];
        assert_eq!(
            (
                configured.record[71],
                configured.record[72],
                configured.record[74]
            ),
            (6, 7, 8),
            "retained semantic properties reach the actual controller consumed by room geometry"
        );
    }

    #[test]
    fn ground_feature_birth_draws_smoke_then_sound_and_retains_its_particle_consumer() {
        let mut terrain = terrain();
        let mut arena = arena(&terrain);
        let particle = ParticleParameter {
            base_tmap: 10,
            word_2: 0,
            word_4: 0,
            speed: 200,
            rotation_speed: 800,
            byte_10: 0,
            byte_11: 0,
            render_mode: 4,
            byte_13: 0,
        };
        let particles = vec![particle; 347];
        let allocation = allocate(
            &mut arena,
            &mut terrain,
            86,
            Position {
                x: 10 << 8,
                y: 10 << 8,
                z: 0,
            },
        )
        .expect("ground feature allocation")
        .expect("eligible open tile");
        assert!(!allocation.reclaim);
        let source_slot = allocation.source_slot;
        let particle_index = allocation
            .particle_index
            .expect("retained ground feature particle");
        bind_particle(
            &mut arena.entities[usize::from(source_slot)],
            particle_index,
            particle,
        )
        .expect("particle producer");
        let global = arena.economy.global_rng;
        for _ in 0..5 {
            let visit = visit_source(&mut arena, &mut terrain, source_slot, &particles)
                .expect("ground feature callback");
            assert!(!visit.spawn_model87);
            assert!(visit.sound_id.is_none());
        }
        let visit = visit_source(&mut arena, &mut terrain, source_slot, &particles)
            .expect("ground feature birth callback");
        assert!(visit.spawn_model87);
        let smoke_rng = global.wrapping_mul(9_377).wrapping_add(9_439);
        let sound_rng = smoke_rng.wrapping_mul(9_377).wrapping_add(9_439);
        assert_eq!(arena.economy.global_rng, sound_rng);
        assert_eq!(
            visit.sound_id,
            (sound_rng & 1 == 0).then_some(particle_index - 282)
        );
        let child = arena
            .entities
            .iter()
            .find(|source| source.active && source.model == 87)
            .expect("real smoke child source row");
        let child_slot = child.index;
        let child_rng = child_slot
            .wrapping_add(smoke_rng as u16)
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        assert_eq!(child.entity_rng, child_rng);
        assert_eq!(child.max_life, (smoke_rng % 23 + 17) as i32);
        assert_eq!(child.particle_parameter_index, Some(67));
        assert_eq!(child.array_yaw(), 400);
        assert_eq!(child.array_pitch(), 100);
        let z = child.position.z;
        assert!(
            !visit_source(&mut arena, &mut terrain, child_slot, &particles)
                .expect("smoke callback")
                .hide
        );
        assert!(arena.entities[usize::from(child_slot)].position.z > z);
        assert!(arena.economy.map_chains.is_linked(child_slot));
    }

    #[test]
    fn linked_authored_path_constructs_real_passage_and_dynamic80_only_hides() {
        let mut terrain = terrain();
        terrain.disaster_parts().0.secondary_heights.fill(39);
        terrain.disaster_parts().0.angles.fill(8);
        let mut arena = arena(&terrain);
        let feature = |index, x, previous, next, width| AuthoredFeature {
            index,
            kind: 10,
            subtype: 80,
            tile_x: x,
            tile_y: 10,
            activation_group: -1,
            word_10: 0,
            stage_tag: 1,
            parameter_1: previous,
            parameter_2: next,
            parameter_3: width,
        };
        let mut features = vec![
            feature(0, 0, 0, 0, 0),
            feature(1, 10, 0, 2, 1),
            feature(2, 16, 1, 0, 3),
        ];
        let allocations = prepare_authored(&mut arena, &mut terrain, &mut features, 1)
            .expect("linked construction");
        assert_eq!(allocations.len(), 1);
        let slot = allocations[0].source_slot;
        let row = &arena.entities[usize::from(slot)];
        assert_eq!(
            (row.model, row.action, row.record[74], axis(row, 0)),
            (81, 88, 0x13, 16 << 8)
        );
        assert_eq!(features[1].stage_tag, 0);
        assert_eq!(features[2].stage_tag, 0);
        assert!(apply_generation_pass(&mut arena, &mut terrain));
        assert!(!apply_generation_pass(&mut arena, &mut terrain));
        let center = terrain_index(12, 10);
        assert!(terrain.planes().ground_heights[center] < 40);
        assert!(terrain.planes().secondary_heights[center] > 39);
        assert_eq!(terrain.planes().angles[center] & 8, 0);
        let content = terrain.deterministic_hash();
        let mut dynamic = new_event(
            3,
            99,
            Position {
                x: 12 << 8,
                y: 10 << 8,
                z: 0,
            },
        );
        construct(&mut dynamic, &terrain, 80);
        assert!(step(&mut terrain, &mut dynamic).hide);
        assert_eq!(terrain.deterministic_hash(), content);
    }

    #[test]
    fn shaped_floor_and_ceiling_use_separate_source_phases_and_retained_rng() {
        for model in 83..=85 {
            let mut terrain = terrain();
            let mut source = new_event(
                1,
                99,
                Position {
                    x: 10 << 8,
                    y: 10 << 8,
                    z: 0,
                },
            );
            construct(&mut source, &terrain, model);
            bind_authored(&mut source, [0, 0, 2], 2);
            let initial = terrain.deterministic_hash();
            assert!(!step(&mut terrain, &mut source).terrain_mutated);
            assert_eq!(
                terrain.deterministic_hash(),
                initial,
                "initialization only stores controller bounds"
            );
            let rng = source.entity_rng;
            for _ in 0..15 {
                step(&mut terrain, &mut source);
            }
            assert_eq!(
                source.entity_rng, rng,
                "shape interpolation draws no random numbers"
            );
            assert!(step(&mut terrain, &mut source).hide);
            let center = terrain_index(10, 10);
            if model != 84 {
                assert!(terrain.planes().ground_heights[center] > 40);
            }
            if model != 85 {
                assert!(terrain.planes().secondary_heights[center] < 80);
            }
        }
    }

    #[test]
    fn mission_four_authored_room_builds_both_planes_for_live_flight_sampling() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-iol7n-campaign-census-b-final/level4");
        let bytes = std::fs::read(package.join("terrain-pre-features.bin"))
            .expect("real Mission4 prefeature owner terrain");
        let authored = std::fs::read(package.join("authored-level.bin"))
            .expect("real Mission4 authored records");
        let mut features = crate::level1_mission::parse_authored_features(&authored)
            .expect("owner authored table");
        let mut terrain = terrain();
        let planes = terrain.disaster_parts().0;
        planes.terrain_types.copy_from_slice(&bytes[..0x10000]);
        planes
            .ground_heights
            .copy_from_slice(&bytes[0x10000..0x20000]);
        planes.shading.copy_from_slice(&bytes[0x20000..0x30000]);
        planes.angles.copy_from_slice(&bytes[0x30000..0x40000]);
        planes
            .secondary_heights
            .copy_from_slice(&bytes[0x40000..0x50000]);
        let mut arena = arena(&terrain);
        let feature = features[13];
        assert_eq!((feature.kind, feature.subtype), (10, 82));
        let before = terrain.clone();
        let allocations = prepare_authored(&mut arena, &mut terrain, &mut features, 13)
            .expect("ordinary authored room producer");
        assert_eq!(allocations.len(), 1);
        assert!(apply_generation_pass(&mut arena, &mut terrain));
        assert!(!apply_generation_pass(&mut arena, &mut terrain));
        let position = feature.world_position(0);
        let ground = terrain.ground_height(position);
        let ceiling = terrain.ceiling_height(position).expect("live Cave ceiling");
        assert!(
            ground < ceiling,
            "constructed room must admit flight between its two planes"
        );
        assert_ne!(
            terrain.planes().ground_heights,
            before.planes().ground_heights
        );
        assert_ne!(
            terrain.planes().secondary_heights,
            before.planes().secondary_heights
        );
        assert_eq!(terrain.planes().angles[index(position)] & 8, 0);
        assert!(
            arena.entities.iter().all(|source| !source.active),
            "generation temporary was reclaimed"
        );
        println!(
            "Mission4 authored room13: ground={ground}, ceiling={ceiling}, live terrain mutation_sequence={}",
            terrain.mutation_sequence()
        );
    }
}
