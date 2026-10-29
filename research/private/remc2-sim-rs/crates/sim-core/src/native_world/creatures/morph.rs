//! MC2 Morph bodies: source constructors followed by StageVar2 selector12.
//!
//! The raw arena record owns the body. It follows the caster rather than
//! entering the independent creature AI; the holder owns its expiry.

use crate::native_world::{EntitySnapshot, Level2EconomyState, MapType, ParticleParameter, SessionTerrain};
use crate::{Position, campaign::source_arena::{SourceArena, SourceEntityHandle}};
use std::collections::BTreeMap;

/// Constructor counters absent from the ordinary creature families. Model19
/// shares its existing Firefly counter; Day model2 uses the canonical Bee.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    pub(crate) phases: [u8; 2],
    // Holder slots are reused independently of their controlled bodies.
    pub(crate) bodies: BTreeMap<u16, (u32, SourceEntityHandle)>,
}

impl State {
    pub(crate) fn bind_body(&mut self, holder: SourceEntityHandle, body: SourceEntityHandle) {
        self.bodies.insert(holder.source_slot, (holder.generation, body));
    }

    pub(crate) fn take_body(&mut self, holder: SourceEntityHandle) -> Option<SourceEntityHandle> {
        if self.bodies.get(&holder.source_slot)?.0 != holder.generation {
            return None;
        }
        self.bodies.remove(&holder.source_slot).map(|(_, body)| body)
    }
}

/// The source arena owns slot retirement; the Morph owner keeps its bindings.
pub(crate) fn holder_generations(economy: &Level2EconomyState) -> impl Iterator<Item = (u16, u32)> + '_ {
    economy.morph.bodies.iter().map(|(&slot, &(generation, _))| (slot, generation))
}

pub(crate) fn forget_holder(economy: &mut Level2EconomyState, slot: u16) {
    economy.morph.bodies.remove(&slot);
}

pub(crate) const fn product(map: MapType, tier: u8) -> Option<u8> {
    match (map, tier) {
        (MapType::Day, 0) => Some(2),
        (MapType::Night | MapType::Cave, 0) => Some(19),
        (_, 1) => Some(25),
        (_, 2) => Some(16),
        _ => None,
    }
}

pub(crate) fn is_body(row: &EntitySnapshot) -> bool {
    row.active
        && row.class == 5
        && matches!(row.model, 16 | 19 | 25)
        && row.action == row.model * 8 + 7
        && row.record[77] == 12
}

/// sub4C310/model16, sub4C6B0/model19, sub4CE00/model25, then sub6A030.
pub(crate) fn allocate(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    model: u8,
    parent: &EntitySnapshot,
    position: Position,
) -> Result<Option<u16>, String> {
    let (life, minimum_speed, maximum_speed, payload, descriptor, particle, width, height): (
        i32,
        i16,
        i16,
        u16,
        u16,
        usize,
        i16,
        i16,
    ) = match model {
        16 => (60_000, 60, 20, 500, 84, 207, 128, 128),
        19 => (600, 76, 8, 300, 88, 287, 85, 51),
        25 => (7_500, 60, 20, 300, 92, 290, 384, 384),
        _ => return Err(format!("Morph has no source constructor for model{model}")),
    };
    particles
        .get(particle)
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| format!("Morph model{model} requires particle{particle}"))?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let initial_rng = arena.economy.new_event_rng(slot);
    let mut row = crate::native_world::cave_features::new_event(slot, initial_rng, position);
    row.class = 5;
    row.record[67] = 5;
    row.model = model;
    row.record[68] = model;
    row.set_source_action(model * 8 + 7);
    row.record[77] = 12;
    row.set_max_life(life);
    row.set_life(life);
    row.set_mana(life >> 1);
    row.set_subspell_index(payload);
    row.actual_speed = minimum_speed;
    row.record[134..136].copy_from_slice(&minimum_speed.to_le_bytes());
    row.record[136..138].copy_from_slice(&minimum_speed.to_le_bytes());
    row.record[138..140].copy_from_slice(&maximum_speed.to_le_bytes());
    let rng = initial_rng.wrapping_mul(9_377).wrapping_add(9_439);
    row.set_entity_rng(rng);
    let angle = (rng & 0x7ff) as i16 - 1;
    row.set_yaw(angle);
    row.set_pitch(angle);
    row.set_source_roll(angle);
    row.parameter_relation = Some(descriptor);
    row.record[164..168].copy_from_slice(&(u32::from(descriptor) + 1).to_le_bytes());
    row.record[60] = 1;
    row.record[61] = 64;
    row.record[69] = 3;
    if model == 19 {
        row.record[70] = 0;
    }
    let phase = if model == 19 {
        let phase = arena.economy.class5_model19_constructor_phase;
        arena.economy.class5_model19_constructor_phase = phase.wrapping_add(1);
        row.record[61] = 35 - phase % 35 + 4;
        phase
    } else {
        let counter = &mut arena.economy.morph.phases[usize::from(model == 25)];
        let phase = *counter;
        *counter = phase.wrapping_add(1);
        phase
    };
    row.set_phase(phase);
    row.set_source_dword_10(if model == 19 {
        i32::from(slot % 100)
    } else {
        0
    });
    crate::world::effects::bind_particle(&mut row, particle as u16, particles)?;
    if model == 16 {
        // Pinned x_BYTE_D9F50 word294=1500; sub4C310 sets 5*1500/8.
        row.record[86..88].copy_from_slice(&937_i16.to_le_bytes());
    }
    row.set_source_overlap_half_extents(width, width);
    row.record[92..94].copy_from_slice(&height.to_le_bytes());
    row.source_id = parent.source_id;
    row.record[30..32].copy_from_slice(&parent.source_id.to_le_bytes());
    row.record[44..46].copy_from_slice(&parent.index.to_le_bytes());
    row.set_control_flags(row.control_flags | 1);
    arena.economy.source_rng.insert(slot, rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let (Some(child), Some(parent)) = (arena.handle(slot), arena.handle(parent.index)) {
        arena.bind_creature_parent(child, parent);
    }
    Ok(Some(slot))
}

/// Common StageVar2 selector12, shared with the canonical Bee follower.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
) -> Result<(), String> {
    let mut row = arena.entities[usize::from(slot)].clone();
    let parent_slot = row.source_spell_parent_entity_index();
    let parent = arena.handle(slot)
        .and_then(|child| arena.creature_parent(child))
        .filter(|parent| parent.source_slot == parent_slot)
        .and_then(|parent| arena.entities.get(usize::from(parent.source_slot)))
        .filter(|parent| parent.active);
    if let Some(parent) = parent
    {
        row.set_position(Position {
            z: (i32::from(parent.position.z)
                - match row.model {
                    16 => 896,
                    25 => 512,
                    _ => 0,
                })
            .max(0) as i16,
            ..parent.position
        });
        row.set_yaw(parent.yaw);
        row.set_source_roll(parent.source_roll());
        if parent.life < 0 {
            row.disable_drawing();
        }
    } else {
        row.set_position(Position::default());
        row.set_yaw(0);
        row.set_source_roll(0);
        row.disable_drawing();
    }
    let position = row.position;
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
}

pub(crate) fn expire(arena: &mut SourceArena, slot: u16) {
    if let Some(row) = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| is_body(row))
    {
        row.disable_drawing();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_product_table_covers_every_map_and_unlockable_tier() {
        for map in [MapType::Day, MapType::Night, MapType::Cave] {
            assert_eq!(
                product(map, 0),
                Some(if map == MapType::Day { 2 } else { 19 })
            );
            assert_eq!(product(map, 1), Some(25));
            assert_eq!(product(map, 2), Some(16));
            assert_eq!(product(map, 3), None);
        }
    }
}
