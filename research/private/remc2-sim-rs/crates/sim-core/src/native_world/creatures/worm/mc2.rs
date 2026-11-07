//! Source Worm StageVar callback composition over the actual arena carrier.

use crate::campaign::source_arena::SourceArena;
use crate::native_world::{ParticleParameter, RuntimeSnapshot, SessionTerrain, source_target};
use crate::{FlightWorld, Position};

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct InheritedDamageReceipt {
    pub root: crate::campaign::source_arena::SourceEntityHandle,
    pub child: crate::campaign::source_arena::SourceEntityHandle,
    pub life_before: i32,
    pub life_after: i32,
    pub attacker: u16,
}

#[cfg(test)]
thread_local! {
    static INHERITED_DAMAGE: std::cell::RefCell<Option<(
        crate::campaign::source_arena::SourceEntityHandle, Vec<InheritedDamageReceipt>,
    )>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_inheritance_trace(root: crate::campaign::source_arena::SourceEntityHandle) {
    INHERITED_DAMAGE.with(|trace| *trace.borrow_mut() = Some((root, Vec::new())));
}

#[cfg(test)]
pub(crate) fn take_inheritance_trace() -> Vec<InheritedDamageReceipt> {
    INHERITED_DAMAGE.with(|trace| {
        trace
            .borrow_mut()
            .take()
            .map_or_else(Vec::new, |(_, rows)| rows)
    })
}

/// Called only at the real first lower-life child selection, never inferred
/// from a later pose, mailbox or root result. This does not change gameplay.
#[cfg(test)]
pub(super) fn record_inherited_damage(
    arena: &SourceArena,
    root_slot: u16,
    child_slot: u16,
    life_before: i32,
    life_after: i32,
    attacker: u16,
) {
    INHERITED_DAMAGE.with(|trace| {
        let mut trace = trace.borrow_mut();
        let Some((wanted, rows)) = trace.as_mut() else {
            return;
        };
        if arena.handle(root_slot) != Some(*wanted) {
            return;
        }
        let child = arena
            .handle(child_slot)
            .expect("actual inherited child has a current source generation");
        rows.push(InheritedDamageReceipt {
            root: *wanted,
            child,
            life_before,
            life_after,
            attacker,
        });
    });
}

fn current_position(
    arena: &SourceArena,
    targets: source_target::Context<'_>,
    slot: u16,
) -> Result<Position, String> {
    source_target::read(arena, targets, slot)?
        .map(|row| row.position)
        .ok_or_else(|| format!("Worm expiry lost current address {slot}"))
}

/// Source1FA50/1C930: each root or part converts its own mana and hides
/// through the same fixed arena address. NewEvent may reuse another current
/// source occupant, so neither birth can use an economy-only allocator.
pub(crate) fn step_action29_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    targets: source_target::Context<'_>,
) -> Result<(), String> {
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Worm death lost {slot}"))?;
    let root = arena
        .economy
        .authored_class5
        .iter()
        .find(|row| row.active && row.source_entity_index == slot && row.model == 3)
        .copied();
    let part = arena
        .economy
        .authored_class5_model3_children
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .copied();
    let (phase, rng, mana, owner, position, yaw) = if let Some(row) = root {
        if row.action != 29 {
            return Err(format!("Worm root {slot} is not in action29"));
        }
        (
            row.source_byte_3e,
            row.entity_rng,
            row.mana,
            arena.economy.source_owner(slot),
            row.position,
            row.yaw,
        )
    } else if let Some(row) = part {
        if row.action != 29 {
            return Err(format!("Worm part {slot} is not in action29"));
        }
        (
            row.source_byte_3e,
            row.entity_rng,
            row.mana,
            row.player_owner_entity_index,
            row.position,
            row.yaw,
        )
    } else {
        if arena.economy.live_arena_kind(slot).is_some() {
            return Err(format!("Worm death {slot} has an unsupported typed owner"));
        }
        let row = arena
            .entities
            .get(usize::from(slot))
            .filter(|row| row.active && (row.class, row.model, row.action) == (5, 3, 29))
            .ok_or_else(|| format!("Worm death {slot} has no current source packet"))?;
        (
            row.phase,
            row.entity_rng,
            row.mana,
            row.source_player_entity_index(),
            row.position,
            row.yaw,
        )
    };
    if phase & 7 != 0 {
        return Ok(());
    }
    if mana > 0 {
        let rng = crate::native_world::level2_next_rng16(rng);
        if let Some(row) = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot && row.model == 3)
        {
            row.entity_rng = rng;
        } else if let Some(row) = arena
            .economy
            .authored_class5_model3_children
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            row.entity_rng = rng;
        } else {
            arena.entities[usize::from(slot)].set_entity_rng(rng);
        }
        arena.economy.source_rng.insert(slot, rng);
        let _ = crate::world::effects::spawn_death_mana_sphere(
            arena, terrain, slot, position, yaw, mana, owner,
        )?;
        if !crate::world::effects::clear_replacement_mana_owner(arena, slot)? {
            if arena.handle(slot) != Some(handle) {
                return Err(format!("Worm death {slot} changed before owner clear"));
            }
            if root.is_some() {
                arena.economy.source_owner.insert(slot, 0);
            } else if let Some(row) = arena
                .economy
                .authored_class5_model3_children
                .iter_mut()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                row.player_owner_entity_index = 0;
            } else {
                arena.entities[usize::from(slot)].set_source_player_entity_index(0);
            }
        }
    }
    // KillEntity reads the same current address after the mana NewEvent.
    let current = source_target::read(arena, targets, slot)?
        .ok_or_else(|| format!("Worm death lost current address {slot}"))?;
    let burst = if current.control_flags & 0x0010_0000 == 0 {
        let particles = targets
            .particles
            .ok_or_else(|| format!("Worm death {slot} has no current particle definitions"))?;
        crate::world::effects::spawn_death_burst(
            arena,
            terrain,
            particles,
            slot,
            current.position,
            current.source_id,
        )?
    } else {
        None
    };
    if crate::world::effects::hide_birth_replacement(arena, slot, burst)? {
        return Ok(());
    }
    if arena.handle(slot) != Some(handle) {
        return Err(format!("Worm death {slot} changed before hide"));
    }
    if let Some(row) = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot && row.model == 3)
    {
        row.drawing_enabled = false;
        let state = arena
            .economy
            .authored_class5_model3
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .ok_or_else(|| format!("Worm death {slot} lost typed root state"))?;
        state.control_flags |= 0x400;
    } else if let Some(row) = arena
        .economy
        .authored_class5_model3_children
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        row.drawing_enabled = false;
        row.control_flags |= 0x400;
    } else {
        arena.entities[usize::from(slot)].disable_drawing();
    }
    Ok(())
}

fn expire(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    targets: source_target::Context<'_>,
    slot: u16,
    summoned: bool,
) -> Result<(), String> {
    // Source1E580 passes this fixed position address to both constructors.
    // Commit the actor before entry and reread after each real NewEvent.
    if summoned {
        let position = current_position(arena, targets, slot)?;
        let visual =
            super::super::goat::runtime::spawn_expiry_visual(arena, terrain, position, particles)?;
        if visual == Some(slot) {
            // Source51A00 dereferences its position argument AFTER NewEvent
            // clears the same address, then grounds that zero XY position.
            let mut position = Position::default();
            position.z = terrain.ground_height(position);
            arena.entities[usize::from(slot)].set_position(position);
            terrain.set_source_predicted_position(position);
            crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
        }
    }
    let position = current_position(arena, targets, slot)?;
    let impact = crate::world::effects::allocate_from_source_position(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        slot,
        position,
        particles,
    )?;
    if crate::world::effects::hide_birth_replacement(arena, slot, impact)? {
        return Ok(());
    }
    if let Some(mut actor) = super::dynamic::current_root_packet(arena, slot) {
        let handle = arena
            .handle(slot)
            .ok_or_else(|| format!("Worm expiry lost current root {slot}"))?;
        actor.disable_drawing();
        if !super::dynamic::commit_root_packet(arena, handle, &actor)? {
            return Err(format!("Worm expiry root {slot} changed before hide"));
        }
    } else if arena.economy.live_arena_kind(slot).is_none() {
        // A successful first birth can leave a raw model73 here if the
        // second birth fails. Hide that actual current packet, not the actor
        // snapshot from before either allocation.
        arena.entities[usize::from(slot)].disable_drawing();
    } else {
        return Err(format!(
            "Worm expiry current occupant {slot} has no hide binding"
        ));
    }
    Ok(())
}

pub(crate) fn step_action31_slot(
    arena: &mut SourceArena,
    runtime: &RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    slot: u16,
    models: &[Vec<u16>; 29],
    class3: &[u16],
    targets: source_target::Context<'_>,
) -> Result<Vec<(u16, u8)>, String> {
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Worm Stage31 lost current root {slot}"))?;
    let mut actor = super::dynamic::current_root_packet(arena, slot)
        .ok_or_else(|| format!("Worm Stage31 lost current packet {slot}"))?;
    let effects = super::stage::step(arena, runtime, terrain, &mut actor, models, class3, targets)?;
    if !super::dynamic::commit_root_packet(arena, handle, &actor)? {
        return Err(format!("Worm Stage31 root {slot} changed before commit"));
    }
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    if let Some(summoned) = effects.expiry {
        let particles = targets
            .particles
            .ok_or_else(|| format!("Worm expiry {slot} has no current particle definitions"))?;
        expire(arena, terrain, particles, targets, slot, summoned)?;
    }
    // No old-actor commit follows the constructors. The common dispatcher
    // owns this visit's phase epilogue and following-pass reclaim.
    Ok(effects.spell_awards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_stage_expiry_hides_the_typed_root_and_publishes_actual_births() {
        let session = crate::native_world::tests::level_two_campaign_session();
        for mode in [13, 16] {
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            let particles = &session.session.definition.presentation.particles;
            let position = session.state.position;
            let root = super::super::dynamic::construct_authored(
                &mut arena,
                &mut terrain,
                307,
                position,
                9000,
                particles,
            )
            .unwrap();
            // sub_4B6F0 grounds the constructor. Source1E580 subsequently passes
            // that actor position, not the caller's airborne player position.
            let expiry_position = root.position;
            assert_eq!(expiry_position.z, terrain.ground_height(expiry_position));
            let slot = root.source_entity_index;
            let handle = arena.handle(slot).unwrap();
            arena
                .economy
                .authored_class5
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .action = 31;
            let state = arena
                .economy
                .authored_class5_model3
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            state.stage_var2 = mode;
            state.stage_remaining = 0;
            let raw_before = arena.entities[usize::from(slot)].clone();
            let before = arena
                .entities
                .iter()
                .filter(|row| row.active && row.class == 10 && matches!(row.model, 0 | 73))
                .map(|row| arena.handle(row.index).unwrap())
                .collect::<Vec<_>>();
            let models = std::array::from_fn(|_| Vec::new());
            let targets = source_target::Context {
                particles: Some(particles),
                ..Default::default()
            };
            step_action31_slot(
                &mut arena,
                session.runtime(),
                &mut terrain,
                slot,
                &models,
                &[],
                targets,
            )
            .unwrap();
            assert_eq!(arena.handle(slot), Some(handle));
            assert!(
                source_target::read(&arena, targets, slot)
                    .unwrap()
                    .unwrap()
                    .drawing_disabled()
            );
            assert_eq!(
                (
                    arena.entities[usize::from(slot)].drawing_disabled(),
                    arena.entities[usize::from(slot)].action
                ),
                (raw_before.drawing_disabled(), raw_before.action),
                "typed hide cannot mutate its retired raw action/visibility; map links retain their own authority"
            );
            let born = arena
                .entities
                .iter()
                .filter(|row| {
                    row.active
                        && row.class == 10
                        && matches!(row.model, 0 | 73)
                        && arena
                            .handle(row.index)
                            .is_some_and(|handle| !before.contains(&handle))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                born.iter().filter(|row| row.model == 0).count(),
                1,
                "both source expiry modes construct one actual new impact"
            );
            assert_eq!(
                born.iter().filter(|row| row.model == 73).count(),
                usize::from(mode == 13),
                "only mode13 constructs the actual summoned-expiry visual"
            );
            assert!(
                born.iter().all(|row| row.position == expiry_position),
                "new births use the current grounded actor pose: {born:?}"
            );
            assert!(
                born.iter().all(|row| row.particle_parameter_index
                    == Some(if row.model == 0 { 7 } else { 220 })),
                "the real effect constructors bind their original presentation descriptors"
            );
            assert_eq!(
                arena
                    .economy
                    .authored_class5_model3_children
                    .iter()
                    .filter(|row| row.active && row.parent_entity_index == slot)
                    .count(),
                16,
                "the callback retains actual linked parts for their own later source visits"
            );
        }
    }
}
