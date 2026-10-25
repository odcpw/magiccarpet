//! Authored transport34 and the explicitly supported action37 follower.

use super::{Outcome, PaletteRequest};
use crate::{
    FlightState, FlightWorld, Position,
    campaign::source_arena::SourceArena,
    heading_for_positions,
    level_definition::FieldDefinition,
    move_entity,
    native_world::{EntitySnapshot, ParticleParameter, SessionTerrain},
    renderable_map_chain::RenderableMapSeed,
};

pub(crate) const PARTICLE: u16 = 223;

pub(crate) fn destination(source: &EntitySnapshot) -> Position {
    Position {
        x: u16::from_le_bytes([source.record[158], source.record[159]]),
        y: u16::from_le_bytes([source.record[160], source.record[161]]),
        z: i16::from_le_bytes([source.record[162], source.record[163]]),
    }
}

fn set_destination(source: &mut EntitySnapshot, position: Position) {
    source.record[158..160].copy_from_slice(&position.x.to_le_bytes());
    source.record[160..162].copy_from_slice(&position.y.to_le_bytes());
    source.record[162..164].copy_from_slice(&position.z.to_le_bytes());
}

/// sub_4FE40. An authored destination replaces the constructor's random XY
/// only after its mandatory RNG draw and MoveEntity call have occurred.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: FieldDefinition,
    particle: ParticleParameter,
) -> Result<Option<u16>, String> {
    let FieldDefinition::Transport {
        destination,
        lifetime,
    } = definition
    else {
        return Err("transport constructor needs transport properties".to_owned());
    };
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut source = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    source.class = 10;
    source.model = 34;
    source.record[67] = 10;
    source.record[68] = 34;
    source.record[69] = 3;
    source.record[70] = 255;
    source.set_source_action(36);
    source.set_max_life(lifetime);
    source.set_life(lifetime);
    source.set_control_flags(source.control_flags & !8);
    if let Err(error) =
        crate::world::terrain::cave_features::bind_particle(&mut source, PARTICLE, particle)
    {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    source.set_source_overlap_half_extents(256, 256);
    let mut elevated = position;
    elevated.z = terrain.ground_height(position).wrapping_add(640);
    source.set_position(elevated);
    let mut target = elevated;
    source.set_entity_rng(source.entity_rng.wrapping_mul(9_377).wrapping_add(9_439));
    move_entity(&mut target, (source.entity_rng & 2047) as i16, 0, i16::MIN);
    target.x = destination.x.wrapping_shl(8).wrapping_add(128);
    target.y = destination.y.wrapping_shl(8).wrapping_add(128);
    set_destination(&mut source, target);
    if let Err(error) = arena.economy.map_chains.sync_entity(RenderableMapSeed {
        entity_index: slot,
        active: true,
        x: elevated.x,
        y: elevated.y,
    }) {
        arena.economy.release_slot(slot, Some(source.entity_rng));
        return Err(error);
    }
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.entities[usize::from(slot)] = source;
    crate::world::terrain::cave_features::synchronize_links(arena, terrain, slot);
    Ok(Some(slot))
}

/// sub_35390. The source tests overlap and facing before copying canonical
/// position. Default life0 is persistent; old life1 becomes0 and hides.
pub(crate) fn step(
    source: &mut EntitySnapshot,
    player_body: &mut EntitySnapshot,
    player: &mut FlightState,
    terrain: &impl FlightWorld,
    destination_clearance: i16,
    local_player_color: u8,
) -> Outcome {
    let mut outcome = Outcome::default();
    if source.control_flags & 2 == 0 {
        source.set_control_flags(source.control_flags | 2);
        outcome.sounds.push((source.index, 21));
    }
    let life = source.life;
    if life > 0 {
        source.set_life(life.wrapping_sub(1));
        if life == 1 {
            source.disable_drawing();
            outcome.sounds.push((source.index, 20));
            return outcome;
        }
    }
    let overlap = crate::level1_mission::source_entities_overlap(
        source.position,
        source.array_pitch(),
        source.array_yaw(),
        source.array_fov(),
        player.position,
        player_body.array_pitch(),
        player_body.array_yaw(),
        player_body.array_fov(),
    );
    if overlap {
        let heading = heading_for_positions(player.position, source.position) as i16;
        let difference =
            (i32::from(player.yaw) - i32::from(heading) + 1024).rem_euclid(2048) - 1024;
        if difference.unsigned_abs() < 170 {
            let mut target = destination(source);
            target.z = terrain
                .ground_height(target)
                .wrapping_add(destination_clearance);
            set_destination(source, target);
            player.position = target;
            player_body.set_position(target);
            outcome.transported = true;
            outcome.sounds.push((source.index, 22));
            if player.player_color_index == local_player_color {
                outcome.palette = Some(PaletteRequest { mode: 6, count: 0 });
            }
        }
    }
    let mut position = source.position;
    position.z = terrain.ground_height(position);
    source.set_position(position);
    outcome
}

/// sub_35530 has no ordinary constructor. A valid parent slot is sufficient;
/// class, hidden state and negative life are deliberately not admission gates.
pub(crate) fn step_follower(
    source: &mut EntitySnapshot,
    parent: Option<&EntitySnapshot>,
    terrain: &impl FlightWorld,
) {
    let parent_slot = u16::from_le_bytes([source.record[44], source.record[45]]);
    let Some(parent) = parent.filter(|parent| parent_slot != 0 && parent.index == parent_slot)
    else {
        return;
    };
    if parent.life == 1 {
        source.disable_drawing();
    } else {
        let mut position = parent.position;
        move_entity(&mut position, source.yaw, source.pitch, source.actual_speed);
        source.set_yaw(source.yaw.wrapping_add(45) & 2047);
        position.z = terrain.ground_height(position).wrapping_add(256);
        source.set_position(position);
    }
}
