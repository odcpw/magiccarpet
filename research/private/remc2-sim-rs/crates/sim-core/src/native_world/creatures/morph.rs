//! MC2 Morph bodies: source constructors followed by StageVar2 selector12.
//!
//! The raw arena record owns the body. It follows the caster rather than
//! entering the independent creature AI; the holder owns its expiry.

use crate::level_definition::{CreatureKind, MORPH_PROPERTIES, MorphProperties};
use crate::native_world::{
    EntitySnapshot, Level2EconomyState, MapType, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::{
    Position,
    campaign::source_arena::{SourceArena, SourceEntityHandle},
};
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
        self.bodies
            .insert(holder.source_slot, (holder.generation, body));
    }

    pub(crate) fn take_body(&mut self, holder: SourceEntityHandle) -> Option<SourceEntityHandle> {
        if self.bodies.get(&holder.source_slot)?.0 != holder.generation {
            return None;
        }
        self.bodies
            .remove(&holder.source_slot)
            .map(|(_, body)| body)
    }
}

/// The source arena owns slot retirement; the Morph owner keeps its bindings.
pub(crate) fn holder_generations(
    economy: &Level2EconomyState,
) -> impl Iterator<Item = (u16, u32)> + '_ {
    economy
        .morph
        .bodies
        .iter()
        .map(|(&slot, &(generation, _))| (slot, generation))
}

pub(crate) fn forget_holder(economy: &mut Level2EconomyState, slot: u16) {
    economy.morph.bodies.remove(&slot);
}

pub(crate) const fn product(map: MapType, tier: u8) -> Option<u8> {
    select_product(map, tier, MORPH_PROPERTIES)
}

pub(crate) const fn select_product(
    map: MapType,
    tier: u8,
    properties: MorphProperties,
) -> Option<u8> {
    if tier >= 3 {
        return None;
    }
    let row = properties.tiers[tier as usize];
    let product = match map {
        MapType::Day => row.day_product,
        MapType::Night | MapType::Cave => row.night_cave_product,
    };
    // Numeric identities belong to this MC2 adapter; definitions select
    // creature families rather than sharing the source's class/model bytes.
    match product {
        CreatureKind::Bee => Some(2),
        CreatureKind::Firefly => Some(19),
        CreatureKind::Cymmerian => Some(25),
        CreatureKind::Wyvern => Some(16),
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
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    model: u8,
    parent: &EntitySnapshot,
    position: Position,
) -> Result<Option<u16>, String> {
    let particle = match model {
        16 => 207,
        19 => 287,
        25 => 290,
        _ => return Err(format!("Morph has no source constructor for model{model}")),
    };
    particles
        .get(particle)
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| format!("Morph model{model} requires particle{particle}"))?;
    let cymmerian_phase = if model == 25 {
        Some(
            *runtime
                .bytes
                .get(crate::native_world::RUNTIME_BYTES + 25)
                .ok_or("Morph Cymmerian source constructor counter table is absent")?,
        )
    } else {
        None
    };
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    // The ordinary source constructor receives &parent.position. NewEvent
    // clears that address first when it reuses the parent slot.
    let position = if slot == parent.index {
        Position::default()
    } else {
        position
    };
    let initial_rng = arena.economy.new_event_rng(slot);
    let mut row = match model {
        16 => {
            let mut counter = arena.economy.morph.phases[0];
            let row = super::creature_16::mc2::construct_row(
                slot,
                arena.slot_generation(slot),
                initial_rng,
                position,
                &mut counter,
                particles,
            )?;
            arena.economy.morph.phases[0] = counter;
            if let Some(mirror) = runtime
                .bytes
                .get_mut(crate::native_world::RUNTIME_BYTES + 16)
            {
                *mirror = counter;
            }
            row
        }
        25 => {
            let at = crate::native_world::RUNTIME_BYTES + 25;
            let phase =
                cymmerian_phase.expect("model25 validated its source counter before allocation");
            let row = super::creature_25::mc2::construct_row(
                slot,
                initial_rng,
                position,
                phase,
                particles,
            )?;
            runtime.bytes[at] = phase.wrapping_add(1);
            row
        }
        19 => {
            // The ordinary Firefly adapter grounds authored creatures. This
            // source constructor instead preserves the Morph parent's Z.
            let phase = arena.economy.class5_model19_constructor_phase;
            arena.economy.class5_model19_constructor_phase = phase.wrapping_add(1);
            if let Some(mirror) = runtime
                .bytes
                .get_mut(crate::native_world::RUNTIME_BYTES + 19)
            {
                *mirror = phase.wrapping_add(1);
            }
            let mut row =
                crate::native_world::cave_features::new_event(slot, initial_rng, position);
            row.class = 5;
            row.record[67] = 5;
            row.model = 19;
            row.record[68] = 19;
            row.set_source_action(153);
            row.set_max_life(600);
            row.set_life(600);
            row.set_mana(300);
            row.set_subspell_index(300);
            row.set_actual_speed(76);
            row.record[136..138].copy_from_slice(&76_i16.to_le_bytes());
            row.record[138..140].copy_from_slice(&8_i16.to_le_bytes());
            let rng = initial_rng.wrapping_mul(9_377).wrapping_add(9_439);
            row.set_entity_rng(rng);
            let angle = (rng & 0x7ff).wrapping_sub(1) as i16;
            row.set_yaw(angle);
            row.set_pitch(angle);
            row.set_source_roll(angle);
            row.parameter_relation = Some(88);
            row.record[164..168].copy_from_slice(&89_u32.to_le_bytes());
            row.record[60] = 1;
            row.record[61] = 35 - phase % 35 + 4;
            row.record[69] = 3;
            row.record[70] = 0;
            row.set_phase(phase);
            row.set_source_dword_10(i32::from(slot % 100));
            crate::world::effects::bind_particle(&mut row, 287, particles)?;
            row.set_source_overlap_half_extents(85, 85);
            row.record[92..94].copy_from_slice(&51_i16.to_le_bytes());
            row
        }
        _ => unreachable!("source constructor was checked before allocating"),
    };
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// Common StageVar2 selector12, shared with the canonical Bee follower.
pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    parent: Option<&EntitySnapshot>,
) -> Result<(), String> {
    let mut row = arena.entities[usize::from(slot)].clone();
    if let Some(parent) = parent {
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

    #[test]
    fn canonical_constructor_counter_and_source_position_alias_are_preserved() {
        let session = super::super::super::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let mut terrain = session.canonical_terrain().clone();
        let particles = &session.session.definition.presentation.particles;
        let slot = *arena.economy.free_source_slots().last().unwrap();
        let mut parent = arena.entities[usize::from(slot)].clone();
        parent.set_position(session.state.position);
        let free = arena.economy.free_source_slots().to_vec();
        assert!(
            allocate(
                &mut arena,
                &mut runtime,
                &mut terrain,
                particles,
                25,
                &parent,
                parent.position
            )
            .is_err(),
            "an absent required counter is a reported input error, never an unchecked-index panic"
        );
        assert_eq!(arena.economy.free_source_slots(), free);
        // This compact fixture omits the exported constructor-counter block.
        // Supply that declared source input, then exercise the real allocator.
        let mut counter_bytes = runtime.bytes.to_vec();
        counter_bytes.resize(crate::native_world::RUNTIME_BYTES + 256, 0);
        runtime.bytes = counter_bytes.into_boxed_slice();
        runtime.bytes[crate::native_world::RUNTIME_BYTES + 25] = 41;
        arena.economy.morph.phases[1] = 7;
        let body = allocate(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            25,
            &parent,
            parent.position,
        )
        .unwrap()
        .unwrap();
        assert_eq!(body, slot);
        let row = &arena.entities[usize::from(body)];
        assert_eq!(
            row.position,
            Position::default(),
            "NewEvent clears the aliased position pointer"
        );
        assert_eq!(
            row.phase, 41,
            "Morph shares the ordinary Cymmerian source counter"
        );
        assert_eq!(runtime.bytes[crate::native_world::RUNTIME_BYTES + 25], 42);
        assert_eq!(
            arena.economy.morph.phases[1], 7,
            "the separate old Morph counter is not authoritative"
        );
        assert_eq!(
            (
                row.class,
                row.model,
                row.action,
                row.record[77],
                row.record[70]
            ),
            (5, 25, 201, 0, 0)
        );
        assert_eq!(
            row.source_id, slot,
            "the ordinary constructor retains NewEvent's identity until the caller's Morph tail"
        );
        assert_eq!(row.source_spell_parent_entity_index(), 0);
        assert_eq!(
            (
                row.life,
                row.max_life,
                row.mana,
                row.particle_parameter_index
            ),
            (7_500, 7_500, 3_750, Some(290))
        );
    }
}
