//! Spider's MC2 web producer and flying row (9/21, action22).
//!
//! Mutable state lives in SourceArena. The scheduler owns the common phase
//! tail and following-pass reclaim; the existing status owner consumes 10/66.

use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::SPIDER_CLASS_PROPERTIES as RULES;
use crate::level1_mission::projectile_guidance_angles;
use crate::native_world::{EntitySnapshot, ParticleParameter, SessionTerrain, guided_projectile};

#[cfg(test)]
use crate::level_definition::{FieldKind, TilePosition};
#[cfg(test)]
use crate::{FlightWorld, move_entity};
#[cfg(test)]
use guided_projectile::centre;

const WEB_PARTICLE: u16 = 319;

#[cfg(test)]
mod tests;

pub(crate) type WebOutcome = guided_projectile::StepOutcome;

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

/// sub_67780 delegates to the same sub_65820 used by Lightning. Source21
/// remains excluded from acquisition, reflection, absorption and hit stats.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<WebOutcome, String> {
    guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        guided_projectile::Guidance {
            yaw_turn: RULES.web_yaw_turn,
            pitch_turn: RULES.web_pitch_turn,
        },
        None,
        context,
    )
}
