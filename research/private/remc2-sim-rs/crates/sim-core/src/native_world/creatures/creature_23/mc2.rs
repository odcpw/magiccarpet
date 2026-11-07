//! Model23 source-record constructor and same-row callback adapter.

use super::{
    Actor, Castle, Context, DESCRIPTOR, Effects, RULES, SOURCE_MODEL, Target, level2_next_rng16,
};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

const SOURCE_PARTICLE: u16 = 289;
pub(crate) const CALLBACK_SOUND: u16 = 59;

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RayBirthReceipt {
    pub source_before: Option<crate::campaign::source_arena::SourceEntityHandle>,
    pub source_after: Option<crate::campaign::source_arena::SourceEntityHandle>,
    pub target_before: Option<crate::campaign::source_arena::SourceEntityHandle>,
    pub target_after: Option<crate::campaign::source_arena::SourceEntityHandle>,
    pub ray: crate::campaign::source_arena::SourceEntityHandle,
    pub owner: u16,
}

#[cfg(test)]
thread_local! {
    static RAY_BIRTH_TRACE: std::cell::RefCell<Option<Vec<RayBirthReceipt>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_ray_birth_trace() {
    RAY_BIRTH_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_ray_birth_trace() -> Vec<RayBirthReceipt> {
    RAY_BIRTH_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct SphereConsumptionReceipt {
    pub actor: crate::campaign::source_arena::SourceEntityHandle,
    pub sphere: crate::campaign::source_arena::SourceEntityHandle,
    pub sphere_mana: i32,
    pub mana_before: i32,
    pub mana_after: i32,
}

#[cfg(test)]
thread_local! {
    static SPHERE_CONSUMPTION_TRACE: std::cell::RefCell<Option<Vec<SphereConsumptionReceipt>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_sphere_consumption_trace() {
    SPHERE_CONSUMPTION_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_sphere_consumption_trace() -> Vec<SphereConsumptionReceipt> {
    SPHERE_CONSUMPTION_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct CallbackDamageReceipt {
    pub actor: crate::campaign::source_arena::SourceEntityHandle,
    pub before: EntitySnapshot,
    pub after: Option<EntitySnapshot>,
    pub entry_writer: Option<crate::native_world::area_damage::PlayerMailboxWrite>,
    pub kill_credit: Option<u16>,
}

#[cfg(test)]
thread_local! {
    static CALLBACK_DAMAGE_TRACE: std::cell::RefCell<Option<(crate::campaign::source_arena::SourceEntityHandle, Vec<CallbackDamageReceipt>)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_callback_damage_trace(
    actor: crate::campaign::source_arena::SourceEntityHandle,
) {
    CALLBACK_DAMAGE_TRACE.with(|trace| *trace.borrow_mut() = Some((actor, Vec::new())));
}

#[cfg(test)]
pub(crate) fn take_callback_damage_trace() -> Vec<CallbackDamageReceipt> {
    CALLBACK_DAMAGE_TRACE.with(|trace| {
        trace
            .borrow_mut()
            .take()
            .map_or_else(Vec::new, |(_, receipts)| receipts)
    })
}

/// Sound59 is prepared at entry to source callbacks 186/187, before motion,
/// target selection or any NewEvent can change the emitting address.
pub(crate) struct SourceSound {
    pub slot: u16,
    pub position: Position,
    pub source_id: u16,
    pub control_flags: u32,
}

pub(crate) fn prepare_callback_sound(
    arena: &SourceArena,
    slot: u16,
) -> Result<Option<SourceSound>, String> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model23 sound source slot{slot} is absent"))?;
    Ok(matches!(row.action, 186 | 187).then_some(SourceSound {
        slot,
        position: row.position,
        source_id: row.source_id,
        control_flags: row.control_flags,
    }))
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn launch_lightning(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: crate::native_world::source_target::Context<'_>,
    players: &mut crate::native_world::source_target::Creature23PlayerFeedback<'_>,
    request: super::LightningRequest,
) -> Result<bool, String> {
    // sub_1D260 publishes this launch axis even when NewEvent fails.
    terrain.set_source_predicted_position(request.position);
    #[cfg(test)]
    let source_before = arena.handle(request.source_slot);
    #[cfg(test)]
    let target_before = arena.handle(request.target_slot);
    let Some(slot) = crate::native_world::lightning::construct(
        arena,
        terrain,
        particles,
        crate::native_world::lightning::Product::Ray,
        request.position,
    )?
    else {
        return Ok(false);
    };
    // Both source pointers are fixed addresses. NewEvent has already run,
    // so source ID/word96 and the target's position/type come from their
    // current owners, including any actual replacement during allocation.
    let source = crate::native_world::source_target::status_parent(
        arena,
        players.context(source_context),
        request.source_slot,
    )?
    .ok_or_else(|| {
        format!(
            "model23 source {} is absent after Ray allocation",
            request.source_slot
        )
    })?;
    let target = crate::native_world::source_target::read(
        arena,
        players.context(source_context),
        request.target_slot,
    )?
    .ok_or_else(|| {
        format!(
            "model23 target {} is absent after Ray allocation",
            request.target_slot
        )
    })?;
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(request.position, target.position);
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 9 && row.model == 9)
        .ok_or_else(|| format!("model23 Lightning constructor lost ray slot{slot}"))?;
    row.source_id = source.source_id;
    put(row, 30, source.source_id);
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    row.record[65] = target.class;
    row.record[66] = target.model;
    row.record[71] = 10;
    row.record[72] = 23;
    put(row, 154, word(&source, 154));
    row.set_subspell_index(request.damage as u16);
    crate::world::effects::sync_position(arena, terrain, slot, request.position, true, false)?;

    #[cfg(test)]
    RAY_BIRTH_TRACE.with(|trace| {
        if let Some(receipts) = trace.borrow_mut().as_mut() {
            if let Some(ray) = arena.handle(slot) {
                receipts.push(RayBirthReceipt {
                    source_before,
                    source_after: arena.handle(request.source_slot),
                    target_before,
                    target_after: arena.handle(request.target_slot),
                    ray,
                    owner: source.source_id,
                });
            }
        }
    });
    players.apply(arena, source_context, request.target_slot)?;
    Ok(true)
}

fn validate_particle(particles: &[ParticleParameter]) -> Result<(), String> {
    let particle = particles
        .get(usize::from(SOURCE_PARTICLE))
        .ok_or_else(|| "model23 requires retained particle289".to_owned())?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "model23 particle289 has unsupported render mode {}",
            particle.render_mode
        ));
    }
    Ok(())
}

pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    phase: u8,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    validate_particle(particles)?;
    let mut position = position;
    position.z = RULES.maximum_lift;
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(184);
    row.set_max_life(RULES.max_life);
    row.set_life(RULES.max_life);
    row.set_mana(RULES.initial_mana);
    row.set_actual_speed(RULES.initial_speed);
    // sub_4CBF0 initializes source word_0x2C_44 to 0x2000, then copies it
    // into position.z. The four-byte entity header places that word at
    // record bytes 48..50; bytes 44..46 remain the parent relation.
    put(&mut row, 48, RULES.maximum_lift as u16);
    put(&mut row, 136, RULES.minimum_speed as u16);
    put(&mut row, 138, RULES.maximum_speed as u16);
    row.set_entity_rng(level2_next_rng16(seed));
    let angle = (row.entity_rng & 2047).wrapping_sub(1) as i16;
    row.set_yaw(angle);
    row.set_source_roll(angle);
    row.set_subspell_index(RULES.constructor_subspell);
    row.set_fov(0);
    row.record[60] = 1;
    row.record[61] = 64;
    row.record[69] = 3;
    row.set_source_byte_46(0);
    row.set_phase(phase);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR) + 1).to_le_bytes());
    crate::world::effects::bind_particle(&mut row, SOURCE_PARTICLE, particles)?;
    row.set_source_overlap_half_extents(384, 384);
    row.record[92..94].copy_from_slice(&384_i16.to_le_bytes());
    Ok(row)
}

fn edit_route(row: &mut EntitySnapshot, runtime: &mut dyn ArcherStageRuntime, bind: Option<u8>) {
    let mut action = row.action;
    let mut index = row.record[76] as i8;
    let mut mode = row.record[77] as i8;
    let mut relation = word(row, 78);
    let mut route = crate::native_world::creatures::stage_route::Route {
        model: SOURCE_MODEL,
        position: row.position,
        action: &mut action,
        index: &mut index,
        mode: &mut mode,
        relation: &mut relation,
    };
    if let Some(index) = bind {
        route.bind(runtime, index);
    } else {
        route.prepare(runtime, |_| false);
    }
    row.set_source_action(action);
    row.record[76] = index as u8;
    row.record[77] = mode as u8;
    put(row, 78, relation);
}

fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model23 StageVar bind lost slot{slot}"))?;
    edit_route(row, runtime, Some(index));
    Ok(())
}

/// Ordinary source sub_4CBF0 followed by the authored StageVar relation.
pub(crate) fn allocate_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    max_life: i32,
    stage_index: Option<u8>,
    authored_record: u16,
) -> Result<Option<u16>, String> {
    validate_particle(particles)?;
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model23 source constructor counters are absent".to_owned())?[23];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let phase = counter;
    counter = counter.wrapping_add(1);
    let row = construct_row(slot, arena.new_event_rng(slot), position, phase, particles)?;
    let mut row = row;
    row.bind_frame_zero_max_life(max_life);
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 23] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    let position = row.position;
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_authored_route(arena, runtime, slot, index)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model23 authored constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature23,
        CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "model23 record{authored_record} lost its source origin"
        ));
    }
    Ok(Some(slot))
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    family: &[u16],
) {
    let dead = arena
        .entities
        .iter()
        .filter(|target| target.life < 0 || target.control_flags & 0x400 != 0)
        .map(|target| target.index)
        .collect::<std::collections::BTreeSet<_>>();
    for &slot in family {
        let Some(row) = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        else {
            continue;
        };
        let mut action = row.action;
        let mut index = row.record[76] as i8;
        let mut mode = row.record[77] as i8;
        let mut relation = word(row, 78);
        let mut route = crate::native_world::creatures::stage_route::Route {
            model: SOURCE_MODEL,
            position: row.position,
            action: &mut action,
            index: &mut index,
            mode: &mut mode,
            relation: &mut relation,
        };
        route.prepare(runtime, |target| dead.contains(&target));
        row.set_source_action(action);
        row.record[76] = index as u8;
        row.record[77] = mode as u8;
        put(row, 78, relation);
    }
}

fn target_views(
    arena: &SourceArena,
    context: crate::native_world::source_target::Context<'_>,
    slots: &[u16],
    scan_castles: &[u16],
) -> Result<Vec<Target>, String> {
    let mut targets = Vec::with_capacity(slots.len());
    for &slot in slots {
        let Some(row) = crate::native_world::source_target::status_parent(arena, context, slot)?
        else {
            continue;
        };
        let castle = if scan_castles.contains(&slot)
            && (matches!(row.model, 0 | 1) || (row.model == 3 && row.mana > 0))
        {
            crate::native_world::source_target::creature23_castle_protection(arena, context, slot)?
                .map(|(position, shape)| Castle {
                    position,
                    half_x: shape.half_width,
                    half_y: shape.half_depth,
                })
        } else {
            None
        };
        targets.push(Target {
            slot,
            source_id: row.source_id,
            class: row.class,
            model: row.model,
            life: row.life,
            mana: row.mana,
            control_flags: row.control_flags,
            position: row.position,
            half_x: word(&row, 88) as i16,
            half_y: word(&row, 90) as i16,
            castle,
        });
    }
    Ok(targets)
}

/// Continue source27E00 after the caller has committed 1B8C0 and any
/// source1ED30 Alliance relation updates. `filtered_target` is that helper's
/// returned address; it does not overwrite the actor's word96.
pub(super) fn attack_after_move(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: crate::native_world::source_target::Context<'_>,
    players: &mut crate::native_world::source_target::Creature23PlayerFeedback<'_>,
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    filtered_target: Option<u16>,
) -> Result<Effects, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .cloned()
        .ok_or_else(|| format!("model23 attack source slot{slot} is absent"))?;
    let slots = filtered_target
        .filter(|slot| *slot != 0)
        .into_iter()
        .collect::<Vec<_>>();
    let targets = target_views(arena, players.context(source_context), &slots, &[])?;
    let peers = families
        .get(usize::from(SOURCE_MODEL))
        .into_iter()
        .flatten()
        .map(|slot| {
            crate::native_world::source_target::read(arena, players.context(source_context), *slot)
        })
        .collect::<Vec<_>>();
    let mut effects = Effects {
        combat_tail_pending: true,
        ..Effects::default()
    };
    let result = {
        let (entities, economy) = (&mut arena.entities, &mut arena.economy);
        let mut context = Context {
            terrain,
            targets: &targets,
            target_error: None,
            mana_candidates: &[],
            class3,
            families,
            peers: &peers,
            entities,
            spheres: &mut economy.spheres,
            population: Some(&mut economy.source_creature_population),
            runtime,
        };
        super::action_attack_after_move(
            &mut Actor { row: &mut row },
            &mut context,
            &mut effects,
            filtered_target,
        )
    };
    arena.entities[usize::from(slot)] = row;
    result?;
    finish_callback(
        arena,
        runtime,
        terrain,
        particles,
        source_context,
        players,
        slot,
        class3,
        families,
        effects,
        false,
    )
}

/// One source callback, committing its own row after sphere-link updates.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: crate::native_world::source_target::Context<'_>,
    players: &mut crate::native_world::source_target::Creature23PlayerFeedback<'_>,
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    mana_slots: &[u16],
) -> Result<Effects, String> {
    if arena.entities.get(usize::from(slot)).is_some_and(|row| {
        row.active && row.class == 5 && row.model == SOURCE_MODEL && row.action == 191
    }) {
        return super::stage::scheduled_step(
            arena,
            runtime,
            terrain,
            particles,
            source_context,
            players,
            slot,
            class3,
            families,
        );
    }
    if arena.entities.get(usize::from(slot)).is_some_and(|row| {
        row.active
            && row.class == 5
            && row.model == SOURCE_MODEL
            && row.action == 186
            && row.record[77] == 14
    }) {
        return super::stage::step_alliance_attack(
            arena,
            runtime,
            terrain,
            particles,
            source_context,
            players,
            slot,
            class3,
            families,
        );
    }
    let Some(mut actor_row) = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .cloned()
    else {
        return Err(format!("model23 callback lost active slot{slot}"));
    };
    let source_prekill = actor_row.action == 188;
    #[cfg(test)]
    let callback_entry = arena.handle(slot).and_then(|actor| {
        CALLBACK_DAMAGE_TRACE.with(|trace| {
            let trace = trace.borrow();
            let (wanted, _) = trace.as_ref()?;
            if *wanted != actor {
                return None;
            }
            let payload = i32::from_le_bytes(
                actor_row.record[98..102]
                    .try_into()
                    .expect("source mailbox"),
            );
            let attacker = word(&actor_row, 102);
            Some((
                actor,
                actor_row.clone(),
                crate::native_world::area_damage::current_actor_mailbox_writer(
                    actor,
                    (payload, attacker),
                ),
            ))
        })
    });
    #[cfg(test)]
    let consumption_before = if actor_row.action == 187 {
        let target = actor_row.source_word_96();
        arena
            .handle(slot)
            .zip(arena.handle(target))
            .and_then(|(actor, sphere)| {
                arena
                    .economy
                    .spheres
                    .iter()
                    .find(|row| row.source_entity_index == target && !row.pending_delete)
                    .map(|row| (actor, sphere, row.mana, actor_row.mana))
            })
    } else {
        None
    };
    let mut target_slots = Vec::new();
    let target = word(&actor_row, 154);
    if actor_row.action == 186 && target != 0 {
        target_slots.push(target);
    }
    let (targets, target_error) =
        match target_views(arena, players.context(source_context), &target_slots, &[]) {
            Ok(targets) => (targets, None),
            Err(error) => (Vec::new(), Some(error)),
        };
    let peers = families
        .get(usize::from(SOURCE_MODEL))
        .into_iter()
        .flatten()
        .map(|slot| {
            crate::native_world::source_target::read(arena, players.context(source_context), *slot)
        })
        .collect::<Vec<_>>();
    let mana_candidates = mana_slots
        .iter()
        .map(|&candidate_slot| {
            crate::native_world::source_target::read(
                arena,
                players.context(source_context),
                candidate_slot,
            )
            .map(|current| {
                current
                    .filter(|row| row.model == 39)
                    .map(|row| super::ManaCandidate {
                        slot: candidate_slot,
                        position: row.position,
                    })
            })
        })
        .collect::<Vec<_>>();
    let source_generation = arena.slot_generation(slot);
    let step_result = {
        let (entities, economy) = (&mut arena.entities, &mut arena.economy);
        let mut context = Context {
            terrain,
            targets: &targets,
            target_error: target_error.as_deref(),
            mana_candidates: &mana_candidates,
            class3,
            families,
            peers: &peers,
            entities,
            spheres: &mut economy.spheres,
            population: Some(&mut economy.source_creature_population),
            runtime,
        };
        super::step(
            &mut Actor {
                row: &mut actor_row,
            },
            &mut context,
        )
    };
    #[cfg(test)]
    if let Some((actor, sphere, sphere_mana, mana_before)) = consumption_before
        && arena.handle(slot) == Some(actor)
        && arena.handle(sphere.source_slot) == Some(sphere)
        && arena
            .economy
            .spheres
            .iter()
            .any(|row| row.source_entity_index == sphere.source_slot && row.pending_delete)
        && actor_row.mana == mana_before.wrapping_add(sphere_mana)
    {
        SPHERE_CONSUMPTION_TRACE.with(|trace| {
            if let Some(receipts) = trace.borrow_mut().as_mut() {
                receipts.push(SphereConsumptionReceipt {
                    actor,
                    sphere,
                    sphere_mana,
                    mana_before,
                    mana_after: actor_row.mana,
                });
            }
        });
    }
    if arena.slot_generation(slot) != source_generation
        || !arena.entities[usize::from(slot)].active
        || arena.entities[usize::from(slot)].model != SOURCE_MODEL
    {
        return Err(format!(
            "model23 source slot{slot} was reclaimed during callback"
        ));
    }
    arena.entities[usize::from(slot)] = actor_row;
    let position = arena.entities[usize::from(slot)].position;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let effects = step_result?;
    let result = finish_callback(
        arena,
        runtime,
        terrain,
        particles,
        source_context,
        players,
        slot,
        class3,
        families,
        effects,
        source_prekill,
    );
    #[cfg(test)]
    if let Some((actor, before, entry_writer)) = callback_entry {
        let after =
            (arena.handle(slot) == Some(actor)).then(|| arena.entities[usize::from(slot)].clone());
        let kill_credit = result.as_ref().ok().and_then(|effects| effects.kill_credit);
        if let Some(after) = &after {
            let payload =
                i32::from_le_bytes(before.record[98..102].try_into().expect("source mailbox"));
            let attacker = word(&before, 102);
            if attacker != 0 && word(after, 102) == 0 {
                let _ = crate::native_world::area_damage::take_player_mailbox_writer(
                    actor,
                    (payload, attacker),
                );
            }
        }
        CALLBACK_DAMAGE_TRACE.with(|trace| {
            if let Some((wanted, receipts)) = trace.borrow_mut().as_mut()
                && *wanted == actor
                && receipts.len() < 64
            {
                receipts.push(CallbackDamageReceipt {
                    actor,
                    before,
                    after,
                    entry_writer,
                    kill_credit,
                });
            }
        });
    }
    result
}

fn finish_callback(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: crate::native_world::source_target::Context<'_>,
    players: &mut crate::native_world::source_target::Creature23PlayerFeedback<'_>,
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    mut effects: Effects,
    source_prekill: bool,
) -> Result<Effects, String> {
    // sub_27E00 creates its real Ray and writes target feedback before the
    // same callback's sub_28110 damage/nearest-victim tail.
    if let Some(request) = effects.lightning.take() {
        launch_lightning(arena, terrain, particles, source_context, players, request)?;
    }
    if effects.combat_tail_pending {
        let mut current = arena.entities[usize::from(slot)].clone();
        let sentinel_class3 = arena.entities.first().is_some_and(|row| row.class == 3);
        let tail = super::consume_combat_tail(&mut Actor { row: &mut current }, sentinel_class3);
        // The following fixed-address class3 scan observes the consumed
        // mailbox/life even if a frozen member now points back to this slot.
        arena.entities[usize::from(slot)] = current.clone();
        if matches!(tail, super::CombatTail::Retaliate(_))
            || (tail == super::CombatTail::Continue && current.phase & 0x1f == 0)
        {
            let targets = if tail == super::CombatTail::Continue {
                target_views(arena, players.context(source_context), class3, class3)?
            } else {
                Vec::new()
            };
            let (entities, economy) = (&mut arena.entities, &mut arena.economy);
            let mut context = Context {
                terrain,
                targets: &targets,
                target_error: None,
                mana_candidates: &[],
                class3,
                families,
                peers: &[],
                entities,
                spheres: &mut economy.spheres,
                population: Some(&mut economy.source_creature_population),
                runtime,
            };
            super::finish_combat_tail(&mut Actor { row: &mut current }, &mut context, tail);
        }
        super::finish_attack(&mut Actor { row: &mut current }, &mut effects);
        arena.entities[usize::from(slot)] = current;
        effects.combat_tail_pending = false;
    }
    if source_prekill {
        effects.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 184)?;
    }
    arena
        .economy
        .source_rng
        .insert(slot, arena.entities[usize::from(slot)].entity_rng);
    if effects.kill {
        super::super::creature_24::mc2::kill(arena, terrain, particles, slot)?;
    }
    Ok(effects)
}
