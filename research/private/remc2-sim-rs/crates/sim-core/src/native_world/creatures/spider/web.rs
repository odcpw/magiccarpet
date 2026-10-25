//! Spider's MC2 web producer and flying row (9/21, action22).
//!
//! Mutable state lives in SourceArena. The scheduler owns the common phase
//! tail and following-pass reclaim; the existing status owner consumes 10/66.

use crate::campaign::source_arena::SourceArena;
use crate::level_definition::{FieldKind, SPIDER_CLASS_PROPERTIES as RULES, TilePosition};
use crate::level1_mission::{
    projectile_guidance_angles, source_map_search_slots, source_search_offsets,
};
use crate::native_world::{EntitySnapshot, ParticleParameter, SessionTerrain, area_damage};
use crate::{FlightWorld, Position, move_entity, turn_toward};

const WEB_PARTICLE: u16 = 319;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WebOutcome {
    pub(crate) impact_slot: Option<u16>,
    pub(crate) experience: Option<(u16, u8, u16)>,
}

fn word(source: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([source.record[offset], source.record[offset + 1]])
}

fn put(source: &mut EntitySnapshot, offset: usize, value: u16) {
    source.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

/// sub_4DCC0. No entity/global RNG draw follows NewEvent's slot-derived seed.
fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent_slot: u16,
) -> Result<Option<u16>, String> {
    let position = arena.entities[usize::from(parent_slot)].position;
    let particle = particles
        .get(usize::from(WEB_PARTICLE))
        .ok_or_else(|| "Spider web needs retained particle319".to_owned())?;
    let limit = particle
        .animation_limit()
        .ok_or_else(|| "Spider web particle319 has invalid animation mode".to_owned())?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    // The source constructor borrows &parent.position across NewEvent's
    // memset. Replacing that address clears the constructor's input too.
    let position = if slot == parent_slot {
        Position { x: 0, y: 0, z: 0 }
    } else {
        position
    };
    let mut row = crate::world::terrain::cave_features::new_event(
        slot,
        arena.economy.new_event_rng(slot),
        position,
    );
    row.class = 9;
    row.model = 21;
    row.record[67] = 9;
    row.record[68] = 21;
    row.set_source_action(22);
    row.set_actual_speed(RULES.web_speed);
    put(&mut row, 136, RULES.web_speed as u16);
    row.set_max_life(RULES.web_range / i32::from(RULES.web_speed));
    row.set_life(row.max_life);
    row.set_control_flags(row.control_flags & !8);
    row.particle_parameter_index = Some(WEB_PARTICLE);
    put(&mut row, 94, WEB_PARTICLE);
    row.record[96] = 0;
    row.record[97] = limit;
    put(&mut row, 86, particle.rotation_speed / 2);
    row.set_source_overlap_half_extents(RULES.web_half_width, RULES.web_half_width);
    put(&mut row, 92, RULES.web_half_height as u16);
    row.set_control_flags(row.control_flags | 4);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
    Ok(Some(slot))
}

/// sub_1D1A0. Inputs are fixed source addresses, re-read after NewEvent can
/// replace either one. The caller applies successful player music feedback.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent_slot: u16,
    target_slot: u16,
) -> Result<Option<u16>, String> {
    if arena.entities.get(usize::from(parent_slot)).is_none() {
        return Err("Spider web parent address is outside arena".to_owned());
    }
    if arena.entities.get(usize::from(target_slot)).is_none() {
        return Err("Spider web target address is outside arena".to_owned());
    }
    let Some(slot) = construct(arena, terrain, particles, parent_slot)? else {
        return Ok(None);
    };
    let parent = arena.entities[usize::from(parent_slot)].clone();
    let target = arena.entities[usize::from(target_slot)].clone();
    let (yaw, pitch) = projectile_guidance_angles(parent.position, target.position);
    let row = &mut arena.entities[usize::from(slot)];
    row.record[71] = 10;
    row.record[72] = 66;
    row.source_id = parent.source_id;
    put(row, 30, parent.source_id);
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    row.set_position(Position {
        z: row.position.z.wrapping_add(RULES.web_lift),
        ..row.position
    });
    put(row, 154, word(&parent, 154));
    // Exported descriptor identity is one-based, as in the existing adapters.
    row.parameter_relation = Some(65);
    row.record[164..168].copy_from_slice(&66_u32.to_le_bytes());
    row.record[69] = target.class;
    row.record[70] = target.model;
    row.set_subspell_index(RULES.web_damage);
    Ok(Some(slot))
}

fn centre(position: Position, model: u8, vertical_offset: i16) -> Position {
    Position {
        z: if model == 2 {
            position.z
        } else {
            position.z.wrapping_add(vertical_offset)
        },
        ..position
    }
}

/// sub_10780: actual map-chain order, selector/id filters and strict source
/// extents. A retained target's life/draw state is not an extra admission gate.
fn collision(arena: &mut SourceArena, row: &EntitySnapshot) -> Option<(u16, Position)> {
    let width = i32::from(row.array_pitch()) + 255;
    let radius = ((width - width.signum() * 255) >> 8) as u8;
    for slot in source_map_search_slots(
        &arena.economy.map_chains,
        row.position,
        &source_search_offsets(radius),
    ) {
        let Some(actor) = area_damage::source_actor(arena, None, slot) else {
            continue;
        };
        if actor.control_flags & 8 == 0
            || actor.source_id == row.source_id
            || !(row.record[69] == 255
                || row.record[69] == actor.class
                    && (row.record[70] == 255 || row.record[70] == actor.model))
        {
            continue;
        }
        let dx = i32::from(actor.position.x.wrapping_sub(row.position.x) as i16).abs();
        let dy = i32::from(actor.position.y.wrapping_sub(row.position.y) as i16).abs();
        let dz = (i32::from(actor.position.z) + i32::from(actor.shape.vertical_offset)
            - i32::from(row.position.z)
            - i32::from(row.array_yaw()))
        .abs();
        if dx < i32::from(row.array_pitch()) + i32::from(actor.shape.half_width)
            && dy < i32::from(row.array_roll()) + i32::from(actor.shape.half_depth)
            && dz < i32::from(row.array_fov()) + i32::from(actor.shape.half_height)
        {
            return Some((
                slot,
                centre(actor.position, actor.model, actor.shape.vertical_offset),
            ));
        }
    }
    None
}

/// sub_67780 -> sub_65820. This model is explicitly excluded by source's
/// reflection, shield absorption, auto-acquisition and hit-stat selector lists.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<WebOutcome, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model, row.action) == (9, 21, 22))
        .ok_or_else(|| format!("Spider web callback lost slot{slot}"))?
        .clone();
    let target = word(&row, 154);
    if target != 0 {
        // Even a reclaimed fixed address retains the position used by source.
        let raw = arena
            .entities
            .get(usize::from(target))
            .ok_or_else(|| "Spider web guidance address is outside arena".to_owned())?;
        let mut position = centre(raw.position, raw.model, raw.array_yaw());
        if let Some(actor) = area_damage::source_actor(arena, None, target) {
            position = centre(actor.position, actor.model, actor.shape.vertical_offset);
        }
        let (yaw, pitch) = projectile_guidance_angles(row.position, position);
        row.set_source_roll(yaw);
        row.set_fov(pitch);
        row.set_yaw(
            row.yaw
                .wrapping_add(turn_toward(row.yaw, yaw, RULES.web_yaw_turn))
                & 2047,
        );
        row.set_pitch(
            row.pitch
                .wrapping_add(turn_toward(row.pitch, pitch, RULES.web_pitch_turn))
                & 2047,
        );
    } else if row.control_flags & 2 == 0 {
        row.set_control_flags(row.control_flags | 2);
        row.set_source_roll(row.yaw);
        row.set_fov(row.pitch);
    }
    let delta = i32::from(row.minimum_speed()) - i32::from(row.actual_speed);
    row.set_actual_speed(row.actual_speed.wrapping_add((2 * delta.signum()) as i16));
    let mut position = row.position;
    move_entity(&mut position, row.yaw, row.pitch, row.actual_speed);
    row.set_position(position);
    arena.entities[usize::from(slot)] = row.clone();
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let hit = collision(arena, &row);
    let terminal = if let Some((_, position)) = hit {
        row.set_position(position);
        true
    } else {
        let ground = terrain.ground_height(row.position);
        let limit = if ground > row.position.z {
            Some(ground)
        } else {
            terrain
                .ceiling_height(row.position)
                .map(|ceiling| ceiling.wrapping_sub(row.array_fov()))
                .filter(|limit| row.position.z > *limit)
        };
        if let Some(z) = limit {
            row.set_position(Position { z, ..row.position });
            arena.entities[usize::from(slot)] = row.clone();
            if terrain.material_mask(row.position) == 1 {
                let splash = crate::world::effects::allocate(
                    arena,
                    terrain,
                    sim_definition::adapter::mc2_effect_definition(
                        crate::level_definition::EffectKind::Ash,
                    ),
                    row.position,
                    particles,
                )?;
                if let Some(splash) = splash {
                    let owner = arena.entities[usize::from(slot)].source_id;
                    let child = &mut arena.entities[usize::from(splash)];
                    child.source_id = owner;
                    put(child, 30, owner);
                }
                arena.entities[usize::from(slot)].disable_drawing();
                return Ok(WebOutcome::default());
            }
            true
        } else {
            row.set_life(row.life.wrapping_sub(1));
            row.life < 0
        }
    };
    let position = row.position;
    arena.entities[usize::from(slot)] = row;
    if hit.is_some() {
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    }
    if !terminal {
        return Ok(WebOutcome::default());
    }
    let definition =
        sim_definition::adapter::mc2_field_definition(FieldKind::Web, TilePosition { x: 0, y: 0 });
    let Some(impact_slot) =
        crate::world::fields::status::construct(arena, terrain, position, definition)?
    else {
        // No hide on NewEvent failure: source retries on its next visit.
        return Ok(WebOutcome::default());
    };
    let current = arena.entities[usize::from(slot)].clone();
    let mut outcome = WebOutcome {
        impact_slot: Some(impact_slot),
        experience: None,
    };
    if let Some((target, _)) = hit {
        let holder = word(&current, 42);
        if holder != 0 {
            let model = arena
                .entities
                .get(usize::from(holder))
                .ok_or_else(|| "Spider web spell holder address is outside arena".to_owned())?
                .model;
            outcome.experience = Some((current.source_id, model, 1));
        }
        // sub_686D0 only changes a class3 owner; ordinary Spider owns itself.
        if let Some(owner) = arena.entities.get_mut(usize::from(current.source_id))
            && owner.class == 3
            && matches!(owner.model, 0 | 1)
        {
            put(owner, 154, target);
        }
    }
    let impact = &mut arena.entities[usize::from(impact_slot)];
    impact.source_id = current.source_id;
    put(impact, 30, current.source_id);
    impact.set_yaw(current.yaw);
    impact.set_pitch(current.pitch);
    if let Some((target, _)) = hit {
        put(impact, 154, target);
    }
    impact.set_subspell_index(current.subspell_index);
    impact.record[74] = current.record[74];
    arena.entities[usize::from(slot)].disable_drawing();
    Ok(outcome)
}
