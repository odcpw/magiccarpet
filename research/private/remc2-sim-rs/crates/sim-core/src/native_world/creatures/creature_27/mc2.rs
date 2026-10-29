//! Live SourceArena producer and root-callback adapter for model27.

use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena, SourceEntityHandle};
use crate::level_definition::CreatureKind;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::{FlightWorld, Position};

use super::{
    Allocation, BODY_ACTION, BODY_PARTICLE, HEAD_ACTION, HEAD_PARTICLE, PROPERTIES, ROOT_PARTICLE,
    SOURCE_MODEL, enabled_root_action,
};
use super::{combat, multipart, stage};

fn put_link(
    arena: &mut SourceArena,
    previous: SourceEntityHandle,
    next: u16,
) -> Result<(), String> {
    if arena.slot_generation(previous.source_slot) != previous.generation {
        return Err(format!(
            "model27 constructor predecessor {} was reused",
            previous.source_slot
        ));
    }
    let row = arena
        .entities
        .get_mut(usize::from(previous.source_slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| {
            format!(
                "model27 constructor lost predecessor {}",
                previous.source_slot
            )
        })?;
    row.record[56..58].copy_from_slice(&next.to_le_bytes());
    Ok(())
}

fn allocate_part(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    root_slot: u16,
    head_index: u8,
    action: u8,
    position: Position,
) -> Result<Option<SourceEntityHandle>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let generation = arena.slot_generation(slot);
    let handle = SourceEntityHandle {
        source_slot: slot,
        generation,
    };
    let result = match action {
        HEAD_ACTION => {
            multipart::initialize_head(arena, root_slot, slot, head_index, position, particles)
        }
        BODY_ACTION => {
            multipart::initialize_body(arena, root_slot, slot, head_index, position, particles)
        }
        _ => Err(format!(
            "model27 constructor received invalid part action {action}"
        )),
    };
    if let Err(error) = result {
        if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
            row.disable_drawing();
        }
        return Err(error);
    }
    if let Err(error) =
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
    {
        if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
            row.disable_drawing();
        }
        return Err(error);
    }
    Ok(Some(handle))
}

/// Ordinary model27 producer corresponding to sub_4D000 after its source
/// model gate: reserve the full 51 slots, build root + five linked heads +
/// forty-five linked body parts, publish every row/map link, then pose the
/// complete chain. A failed part allocation leaves the source-created partial
/// rows hidden until the normal following-pass reclaim.
pub(crate) fn allocate_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    authored_record: u16,
) -> Result<Option<Allocation>, String> {
    if !multipart::map_type_allows_model27(terrain)
        || arena.economy.free_slot_count() < PROPERTIES.roots_require_free_slots
    {
        return Ok(None);
    }
    for particle_index in [ROOT_PARTICLE, HEAD_PARTICLE, BODY_PARTICLE] {
        particles
            .get(usize::from(particle_index))
            .and_then(ParticleParameter::animation_limit)
            .ok_or_else(|| format!("model27 requires valid particle{particle_index}"))?;
    }
    if runtime.constructor_counters().is_none() {
        return Err("model27 source constructor counter table is absent".to_owned());
    }

    let Some(root_slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let root_handle = SourceEntityHandle {
        source_slot: root_slot,
        generation: arena.slot_generation(root_slot),
    };
    let mut allocated = vec![root_handle];
    let result = (|| {
        multipart::initialize_root(arena, runtime, root_slot, position, particles)?;
        crate::world::effects::sync_position(arena, terrain, root_slot, position, true, false)?;
        if !arena.bind_raw_creature_origin(
            root_handle,
            CreatureKind::Creature27,
            CreatureRootOrigin::AuthoredRecord { authored_record },
        ) {
            return Err(format!(
                "model27 authored record{authored_record} lost generation-bound root origin"
            ));
        }

        let mut heads = Vec::with_capacity(PROPERTIES.head_count);
        let mut body_groups = Vec::with_capacity(PROPERTIES.head_count);
        let mut previous = root_handle;
        for head_index in 0..PROPERTIES.head_count {
            let Some(head) = allocate_part(
                arena,
                terrain,
                particles,
                root_slot,
                head_index as u8,
                HEAD_ACTION,
                position,
            )?
            else {
                return Ok(None);
            };
            allocated.push(head);
            put_link(arena, previous, head.source_slot)?;
            if !arena.bind_raw_creature_part(head, root_handle, CreatureKind::Creature27) {
                return Err(format!(
                    "model27 head {} lost generation-bound root link",
                    head.source_slot
                ));
            }
            heads.push(head);
            previous = head;

            let mut bodies = Vec::with_capacity(PROPERTIES.body_parts_per_head);
            for _ in 0..PROPERTIES.body_parts_per_head {
                let Some(body) = allocate_part(
                    arena,
                    terrain,
                    particles,
                    root_slot,
                    head_index as u8,
                    BODY_ACTION,
                    position,
                )?
                else {
                    return Ok(None);
                };
                allocated.push(body);
                put_link(arena, previous, body.source_slot)?;
                if !arena.bind_raw_creature_part(body, root_handle, CreatureKind::Creature27) {
                    return Err(format!(
                        "model27 body {} lost generation-bound root link",
                        body.source_slot
                    ));
                }
                bodies.push(body);
                previous = body;
            }
            body_groups.push(bodies);
        }

        let heads: [SourceEntityHandle; 5] = heads
            .try_into()
            .map_err(|_| "model27 constructor did not retain five heads".to_owned())?;
        let bodies: [[SourceEntityHandle; 9]; 5] = body_groups
            .into_iter()
            .map(|group| {
                group
                    .try_into()
                    .map_err(|_| "model27 constructor did not retain nine body parts".to_owned())
            })
            .collect::<Result<Vec<_>, String>>()?
            .try_into()
            .map_err(|_| "model27 constructor did not retain five body groups".to_owned())?;
        let allocation = Allocation {
            root: root_handle,
            heads,
            bodies,
        };
        let chain = multipart::read_chain(arena, root_handle)?;
        multipart::pose_new_chain(arena, terrain, &chain)?;
        multipart::commit_root_constructor_phase(arena, runtime, root_slot)?;
        Ok(Some(allocation))
    })();
    match result {
        Ok(Some(allocation)) => Ok(Some(allocation)),
        Ok(None) => {
            arena.clear_raw_creature_origin(root_handle.source_slot);
            multipart::hide_unregistered(arena, &allocated);
            Ok(None)
        }
        Err(error) => {
            arena.clear_raw_creature_origin(root_handle.source_slot);
            multipart::hide_unregistered(arena, &allocated);
            Err(error)
        }
    }
}

/// Objective-facing root predicate. Linked heads and body segments share the
/// source model number, so observers must count only a live, generation-current
/// action root.
pub(crate) fn is_living_root(arena: &SourceArena, root: SourceEntityHandle) -> bool {
    arena.slot_generation(root.source_slot) == root.generation
        && arena
            .entities
            .get(usize::from(root.source_slot))
            .is_some_and(|row| {
                row.active
                    && row.class == 5
                    && row.model == SOURCE_MODEL
                    && enabled_root_action(row.action)
                    && row.life >= 0
                    && !row.drawing_disabled()
            })
}

/// Source sub_12100/sub_12330 binds an authored StageVar after sub_4D000.
pub(crate) fn bind_authored_stage(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    root: SourceEntityHandle,
    index: u8,
) -> Result<(), String> {
    if index == 0 {
        return Ok(());
    }
    let row = match arena.resolve(root) {
        Some(crate::campaign::source_arena::SourceEntityView::Imported(row))
            if row.class == 5 && row.model == SOURCE_MODEL && enabled_root_action(row.action) =>
        {
            row
        }
        _ => {
            return Err(format!(
                "model27 StageVar bind lost root {}",
                root.source_slot
            ));
        }
    };
    let mut action = row.action;
    let mut stage_index = row.record[76] as i8;
    let mut mode = row.record[77] as i8;
    let mut relation = u16::from_le_bytes([row.record[78], row.record[79]]);
    let mut route = crate::native_world::creatures::stage_route::Route {
        model: SOURCE_MODEL,
        position: row.position,
        action: &mut action,
        index: &mut stage_index,
        mode: &mut mode,
        relation: &mut relation,
    };
    route.bind(runtime, index);
    let row = &mut arena.entities[usize::from(root.source_slot)];
    row.set_source_action(action);
    row.record[76] = stage_index as u8;
    row.record[77] = mode as u8;
    row.record[78..80].copy_from_slice(&relation.to_le_bytes());
    Ok(())
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    pub callback_executed: bool,
    pub projectile_slots: Vec<u16>,
    pub sounds: Vec<(u16, u8)>,
    pub death: Option<combat::DeathOutcome>,
    /// Credit is set by the source pre-kill callback, committed once to the
    /// authored creature counter in this adapter, and cleared before return.
    pub kill_credit: Option<u16>,
}

/// Apply one enabled root action and run its five heads and 45 segments within
/// this callback. The shared dispatcher consumes projectiles before the
/// source pass tail and reclaims the hidden chain at its following pass.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    attack_range: u16,
    attack_cone: u16,
    setting_30: u16,
) -> Result<Effects, String> {
    let root = arena
        .handle(slot)
        .ok_or_else(|| format!("model27 callback lost root slot{slot}"))?;
    let action = arena.entities[usize::from(slot)].action;
    if !enabled_root_action(action) {
        return Err(format!("model27 action{action} has no source callback"));
    }
    let mut effects = Effects {
        callback_executed: true,
        ..Effects::default()
    };
    // PreKill writes action221 to the whole source chain. Higher slots can
    // run that enabled action in this pass, including already-hidden parts.
    if action == 221 {
        effects.death = Some(combat::kill(arena, terrain, root)?);
        return Ok(effects);
    }
    let chain = multipart::read_chain(arena, root)?;
    let root_callback = match action {
        217 | 218 => Some(action),
        219 => Some(217),
        _ => None,
    };
    match action {
        220 => {
            effects.kill_credit = combat::prekill(arena, root)?;
            if let Some(killer) = effects.kill_credit.take() {
                arena.economy.record_authored_creature_kill(killer);
            }
            return Ok(effects);
        }
        222 => {
            return Err("model27 action222 has an unsupported source callback".to_owned());
        }
        223 => {
            // sub_29930 delegates to sub_1D5D0 before its visual and
            // multipart tail. Mode 10 selects the descriptor's attack action;
            // modes 0, 11 and 15 have no sub_1D5D0 case.
            let mode = arena.entities[usize::from(slot)].record[77];
            match mode {
                1..=9 | 12..=14 | 16 | 17 => stage::step(
                    arena, runtime, terrain, particles, root, class3, families, mode,
                )?,
                10 => arena.entities[usize::from(slot)].set_source_action(218),
                0 | 11 | 15 => {}
                _ => return Err(format!("model27 StageVar mode {mode} is not implemented")),
            }
            let mode = arena.entities[usize::from(slot)].record[77];
            combat::set_root_visual(
                arena,
                particles,
                slot,
                if matches!(mode, 2 | 6..=9) { 337 } else { 315 },
            )?;
            let row = &mut arena.entities[usize::from(slot)];
            row.set_life(PROPERTIES.root_initial_life);
            let next_action = row.action;
            let target = u16::from_le_bytes([row.record[154], row.record[155]]);
            if next_action == 218 {
                row.record[77] = 10;
                for head in chain.heads {
                    let row = &mut arena.entities[usize::from(head.source_slot)];
                    if row.record[74] == 1 {
                        row.record[74] = 2;
                        row.record[154..156].copy_from_slice(&target.to_le_bytes());
                    }
                }
            } else if next_action == 216 {
                row.record[77] = 15;
            }
        }
        216 => {
            let previous = {
                let row = &mut arena.entities[usize::from(slot)];
                let previous = row.source_dword_10();
                row.set_source_dword_10(previous.wrapping_add(1));
                if previous == 0 {
                    row.set_life(1_000_000);
                    row.record[154..156].fill(0);
                }
                if previous == 18 {
                    row.set_source_action(218);
                    row.record[154..156].fill(0);
                    row.record[74] = 1;
                }
                previous
            };
            if matches!(previous, 3 | 12 | 15) {
                let mask = if previous == 12 {
                    0x0080_0000
                } else {
                    0x0100_0000
                };
                for handle in std::iter::once(root).chain(chain.ordered.iter().copied()) {
                    if arena.slot_generation(handle.source_slot) == handle.generation {
                        let part = &mut arena.entities[usize::from(handle.source_slot)];
                        part.set_control_flags((part.control_flags & !0x0180_0000) | mask);
                    }
                }
            }
            if matches!(previous, 0 | 18) {
                combat::set_root_visual(arena, particles, slot, 337)?;
            }
            let mut position = arena.entities[usize::from(slot)].position;
            position.z = terrain.ground_height(position);
            arena.entities[usize::from(slot)].set_position(position);
            crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
        }
        219 => arena.entities[usize::from(slot)].set_source_action(217),
        217 | 218 => {}
        _ => unreachable!("enabled_root_action covers exactly 216..=223"),
    }

    if let Some(callback_action) = root_callback {
        combat::step_root_action(
            arena,
            terrain,
            particles,
            root,
            callback_action,
            class3,
            attack_range,
            attack_cone,
        )?;
    }

    let head_effects = combat::step_heads(
        arena,
        terrain,
        particles,
        &Allocation {
            root,
            heads: chain.heads,
            bodies: chain.bodies,
        },
        &chain,
        class3,
        PROPERTIES.head_target_range,
        PROPERTIES.head_target_half_angle,
        setting_30,
    )?;
    effects.projectile_slots = head_effects.projectile_slots;
    effects.sounds = head_effects.sounds;

    Ok(effects)
}

/// Materialize either source projectile through its ordinary scheduled constructor.
pub(super) fn launch_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    shot: combat::ProjectileIntent,
) -> Result<Option<u16>, String> {
    crate::native_world::guided_projectile::boss::construct_model27(
        arena,
        terrain,
        particles,
        shot.head.source_slot,
        shot.target.source_slot,
        shot.selector,
    )
}

pub(crate) fn hide_for_terminal(
    arena: &mut SourceArena,
    allocation: &Allocation,
) -> Result<(), String> {
    let chain = multipart::read_chain(arena, allocation.root)?;
    for handle in std::iter::once(allocation.root).chain(chain.ordered.iter().copied()) {
        if arena.slot_generation(handle.source_slot) == handle.generation {
            let row = &mut arena.entities[usize::from(handle.source_slot)];
            row.set_control_flags(row.control_flags | ENTITY_DRAW_DISABLED_BIT);
        }
    }
    Ok(())
}
