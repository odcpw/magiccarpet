//! Passive world items over the one authoritative entity arena.
//!
//! Construction owns the item packet. The source scheduler owns its common
//! phase tail and following-pass reclamation; empty items invent no pickup.

use crate::{
    Position,
    campaign::source_arena::SourceArena,
    native_world::{EntitySnapshot, ParticleParameter, SessionTerrain},
    world::terrain::cave_features::{bind_particle, new_event},
};
use sim_definition::adapter::PASSIVE_ITEM_SOURCE_BINDING as SOURCE;

pub(crate) use sim_definition::level_definition::PassiveItemDefinition as Definition;

fn valid(row: &EntitySnapshot) -> bool {
    row.active && (row.class, row.model, row.action) == (SOURCE.class, SOURCE.model, SOURCE.action)
}

fn bind_shape(row: &mut EntitySnapshot, definition: Definition) {
    row.set_source_overlap_half_extents(
        definition.horizontal_half_extent,
        definition.horizontal_half_extent,
    );
    row.record[92..94].copy_from_slice(&definition.vertical_half_extent.to_le_bytes());
}

/// Adopt immutable properties without reconstructing an imported incarnation.
pub(crate) fn bind_initial(row: &mut EntitySnapshot, definition: Definition) -> Result<(), String> {
    definition.validate()?;
    if !valid(row) {
        return Err("passive item definition requires its active source carrier".into());
    }
    let deficit = row.max_life.wrapping_sub(row.life);
    row.set_max_life(definition.max_life);
    row.set_life(definition.max_life.wrapping_sub(deficit));
    bind_shape(row, definition);
    Ok(())
}

/// MC2 sub51530 -> sub514E0: NewEvent, map link, CopyMaxLifeToLife,
/// SetHalfSpeed(particle77), then SetEntityShiftRot(384,384). No extra RNG
/// draw or terrain-height write follows the caller's construction position.
pub(crate) fn construct(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    definition: Definition,
    particles: &[ParticleParameter],
) -> Result<Option<u16>, String> {
    definition.validate()?;
    let particle = *particles
        .get(usize::from(SOURCE.descriptor))
        .ok_or("passive item requires retained particle77")?;
    if particle.animation_limit().is_none() {
        return Err("passive item particle77 has an unsupported render mode".into());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = SOURCE.class;
    row.model = SOURCE.model;
    row.record[67] = SOURCE.class;
    row.record[68] = SOURCE.model;
    row.set_source_action(SOURCE.action);
    // AddEventToMap sets bit2 and retains NewEvent's bit3.
    row.set_control_flags(row.control_flags | 4);
    row.set_max_life(definition.max_life);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let row = &mut arena.entities[usize::from(slot)];
    row.set_life(row.max_life);
    bind_particle(row, SOURCE.descriptor, particle)?;
    bind_shape(row, definition);
    Ok(Some(slot))
}

/// Enabled source action0 is empty. Even queued damage or negative life does
/// not introduce motion, a reward or a death branch. Phase is scheduler-owned.
pub(crate) fn step(arena: &SourceArena, slot: u16) -> Result<(), String> {
    if arena.entities.get(usize::from(slot)).is_some_and(valid) {
        Ok(())
    } else {
        Err(format!(
            "passive item callback lost its source carrier {slot}"
        ))
    }
}

#[cfg(test)]
mod tests;
