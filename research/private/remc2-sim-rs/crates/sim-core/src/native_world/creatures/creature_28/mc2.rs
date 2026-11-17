//! Source-row constructor and callback adapter for class5/model28.

use super::{
    Actor, Effects, Frame, SOURCE_ATTACK_VISUAL, SOURCE_CLASS, SOURCE_DESCRIPTOR,
    SOURCE_IDLE_ACTION, SOURCE_IDLE_VISUAL, SOURCE_MODEL, SOURCE_PROPERTIES,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct CallbackDamageReceipt {
    pub actor: crate::campaign::source_arena::SourceEntityHandle,
    pub before: EntitySnapshot,
    pub after: Option<EntitySnapshot>,
    pub entry_player: Option<(u16, Position, i16)>,
    pub entry_writer: Option<crate::native_world::area_damage::PlayerMailboxWrite>,
    pub melee: Option<super::MeleeRequest>,
    pub kill_credit: Option<u16>,
    pub splash: Option<crate::campaign::source_arena::SourceEntityHandle>,
}

#[cfg(test)]
thread_local! {
    static CALLBACK_TRACE: std::cell::RefCell<Option<(crate::campaign::source_arena::SourceEntityHandle, Vec<CallbackDamageReceipt>)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_callback_damage_trace(actor: crate::campaign::source_arena::SourceEntityHandle) {
    CALLBACK_TRACE.with(|trace| *trace.borrow_mut() = Some((actor, Vec::new())));
}

#[cfg(test)]
pub(crate) fn take_callback_damage_trace() -> Vec<CallbackDamageReceipt> {
    CALLBACK_TRACE.with(|trace| trace.borrow_mut().take().map_or_else(Vec::new, |(_, rows)| rows))
}

const SOURCE_ROW_MIN_LEN: usize = 176;
/// sub_2B860 reads this independent selector from x_BYTE_D9F50[0x5b6].
const SOURCE_ATTACK_ANIMATION_TMAP: u16 = 311;

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn animation_limit(particles: &[ParticleParameter], visual: u16) -> Result<u8, String> {
    particles
        .get(usize::from(visual))
        .ok_or_else(|| format!("model28 requires retained particle{visual}"))?
        .animation_limit()
        .ok_or_else(|| format!("model28 particle{visual} has invalid animation mode"))
}

pub(crate) fn load(row: &EntitySnapshot) -> Option<Actor> {
    (row.active
        && row.class == SOURCE_CLASS
        && row.model == SOURCE_MODEL
        && row.record.len() >= SOURCE_ROW_MIN_LEN)
        .then(|| Actor {
            slot: row.index,
            source_id: row.source_id,
            action: row.action,
            phase: row.record[74],
            flags: row.control_flags,
            position: row.position,
            rng: row.entity_rng,
            yaw: row.yaw,
            pitch: row.pitch,
            half_width: word(row, 88) as i16,
            roll: word(row, 36) as i16,
            speed: row.actual_speed,
            minimum_speed: word(row, 136) as i16,
            maximum_speed: word(row, 138) as i16,
            life: row.life,
            max_life: row.max_life,
            timer: row.source_dword_10(),
            source_animation: word(row, 48),
            descriptor: row.parameter_relation.unwrap_or(SOURCE_DESCRIPTOR),
            subspell_index: row.subspell_index,
            target: word(row, 154),
            child_head: word(row, 56),
            parent: word(row, 44),
            route_index: row.record[76],
            route_mode: row.record[77],
            route_relation: word(row, 78),
            route_remaining: word(row, 50) as i16,
            route_duration: word(row, 52),
            attacker: word(row, 42),
            killer: word(row, 40),
            pending_damage: i32::from_le_bytes(
                row.record[98..102].try_into().expect("fixed source row"),
            ),
            pending_attacker: word(row, 102),
            target_scan_limit: row.record[61],
            source_counter: row.record[66],
            visual: word(row, 94),
            animation_frame: row.record[96],
            animation_limit: row.record[97],
        })
}

fn bind_visual(
    row: &mut EntitySnapshot,
    visual: u16,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if particles.get(usize::from(visual)).is_none() {
        return Err(format!("model28 requires retained particle{visual}"));
    }
    crate::world::effects::bind_particle(row, visual, particles)?;
    match visual {
        SOURCE_IDLE_VISUAL => {
            put_word(row, 88, SOURCE_PROPERTIES.idle_half_width as u16);
            put_word(row, 90, SOURCE_PROPERTIES.idle_half_width as u16);
            put_word(row, 92, SOURCE_PROPERTIES.idle_half_height as u16);
        }
        SOURCE_ATTACK_VISUAL => {
            put_word(row, 88, SOURCE_PROPERTIES.attack_half_width as u16);
            put_word(row, 90, SOURCE_PROPERTIES.attack_half_width as u16);
            put_word(row, 92, SOURCE_PROPERTIES.attack_half_height as u16);
        }
        _ => return Err(format!("unsupported model28 visual{visual}")),
    }
    Ok(())
}

fn store(
    row: &mut EntitySnapshot,
    actor: Actor,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if row.index != actor.slot {
        return Err(format!("model28 source row moved from slot{}", actor.slot));
    }
    if word(row, 94) != actor.visual {
        bind_visual(row, actor.visual, particles)?;
    }
    row.source_id = actor.source_id;
    row.record[30..32].copy_from_slice(&actor.source_id.to_le_bytes());
    row.set_source_action(actor.action);
    row.set_source_byte_46(actor.phase);
    row.set_control_flags(actor.flags);
    row.set_position(actor.position);
    row.set_entity_rng(actor.rng);
    row.set_yaw(actor.yaw);
    row.set_pitch(actor.pitch);
    row.set_source_roll(actor.roll);
    row.set_actual_speed(actor.speed);
    put_word(row, 136, actor.minimum_speed as u16);
    put_word(row, 138, actor.maximum_speed as u16);
    row.set_max_life(actor.max_life);
    row.set_life(actor.life);
    row.set_source_dword_10(actor.timer);
    put_word(row, 48, actor.source_animation);
    row.set_subspell_index(actor.subspell_index);
    put_word(row, 154, actor.target);
    put_word(row, 56, actor.child_head);
    put_word(row, 44, actor.parent);
    row.record[76] = actor.route_index;
    row.record[77] = actor.route_mode;
    put_word(row, 78, actor.route_relation);
    put_word(row, 50, actor.route_remaining as u16);
    put_word(row, 52, actor.route_duration as u16);
    put_word(row, 42, actor.attacker);
    put_word(row, 40, actor.killer);
    row.record[98..102].copy_from_slice(&actor.pending_damage.to_le_bytes());
    put_word(row, 102, actor.pending_attacker);
    row.record[61] = actor.target_scan_limit;
    row.set_phase(actor.source_counter);
    put_word(row, 94, actor.visual);
    row.particle_parameter_index = Some(actor.visual);
    row.record[96] = actor.animation_frame;
    row.record[97] = actor.animation_limit;
    row.parameter_relation = Some(actor.descriptor);
    row.record[164..168].copy_from_slice(&(u32::from(actor.descriptor) + 1).to_le_bytes());
    Ok(())
}

/// Pinned sub_4D1D0 defaults. The per-model byte is consumed only after both
/// source animation dependencies have been validated.
pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    source_counter: u8,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let idle_limit = animation_limit(particles, SOURCE_IDLE_VISUAL)?;
    let _ = animation_limit(particles, SOURCE_ATTACK_VISUAL)?;
    let mut row = crate::world::terrain::cave_features::new_event(slot, seed, position);
    row.class = SOURCE_CLASS;
    row.model = SOURCE_MODEL;
    row.record[67] = SOURCE_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(SOURCE_IDLE_ACTION);
    row.record[60] = 1;
    row.record[61] = 64;
    row.set_phase(source_counter);
    row.record[69] = 3;
    row.set_control_flags(row.control_flags | 0x0800_0000);
    row.set_subspell_index(SOURCE_PROPERTIES.attack_damage);
    row.set_max_life(SOURCE_PROPERTIES.maximum_life);
    row.set_life(SOURCE_PROPERTIES.maximum_life);
    row.set_mana(SOURCE_PROPERTIES.initial_mana);
    row.set_actual_speed(SOURCE_PROPERTIES.initial_speed);
    put_word(&mut row, 136, SOURCE_PROPERTIES.minimum_speed as u16);
    put_word(&mut row, 138, SOURCE_PROPERTIES.maximum_speed as u16);

    let rng = seed.wrapping_mul(9_377).wrapping_add(9_439);
    row.set_entity_rng(rng);
    let heading = (rng & 2047).wrapping_sub(1) as i16;
    row.set_yaw(heading);
    row.set_pitch(heading);
    row.set_source_roll(heading);
    row.set_fov(0);
    row.set_source_byte_46(0);
    row.set_source_dword_10(0);
    row.parameter_relation = Some(SOURCE_DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(SOURCE_DESCRIPTOR) + 1).to_le_bytes());
    bind_visual(&mut row, SOURCE_IDLE_VISUAL, particles)?;
    row.record[97] = idle_limit;
    Ok(row)
}

/// Ordinary `sub_4A310` authored producer. The row is installed and mapped
/// before its StageVar and generation-bound authored identity are published.
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
    let _ = animation_limit(particles, SOURCE_IDLE_VISUAL)?;
    let _ = animation_limit(particles, SOURCE_ATTACK_VISUAL)?;
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model28 source constructor counter is absent".to_owned())?[28];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = construct_row(
        slot,
        arena.new_event_rng(slot),
        position,
        counter,
        particles,
    )?;
    row.bind_frame_zero_max_life(max_life);
    counter = counter.wrapping_add(1);
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 28] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        super::stage::bind_authored(arena, runtime, slot, index)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model28 constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature28,
        crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "model28 record{authored_record} lost generation-bound origin"
        ));
    }
    Ok(Some(slot))
}

/// Source StageVar prepass over the frozen model28 membership for this pass.
pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    targets: crate::native_world::source_target::Context<'_>,
    model_slots: &[u16],
) -> Result<(), String> {
    super::stage::prepare_source_pass(arena, runtime, targets, model_slots)
}

fn current_target(
    arena: &SourceArena,
    context: crate::native_world::source_target::Context<'_>,
    slot: u16,
) -> Result<Option<super::Target>, String> {
    let Some(view) = crate::native_world::source_target::read(arena, context, slot)? else { return Ok(None) };
    let Some(row) = crate::native_world::source_target::status_parent(arena, context, slot)? else { return Ok(None) };
    Ok(Some(super::Target {
        slot, source_id: view.source_id, class: view.class, model: view.model,
        action: row.action, phase: row.record[74], flags: view.control_flags,
        life: view.life, position: view.position, yaw: view.yaw.unwrap_or(0),
        half_width: view.collision.map_or(0, |shape| shape.half_width),
        next: view.relations.next.unwrap_or(0), attacker: view.relations.attacker.unwrap_or(0),
        killer: row.word_24(), route_index: view.relations.stage_index.unwrap_or(0),
        route_relation: view.relations.stage_relation.unwrap_or(0),
        route_target: view.relations.target.unwrap_or(0), parent: view.relations.parent.unwrap_or(0),
        route_mode: view.relations.stage_mode.unwrap_or(0), relations: view.relations,
        source_yaw: view.yaw, source_roll: view.roll,
        pitch_extent: view.collision.map(|shape| shape.half_width),
    }))
}

fn raw_sentinel(row: &EntitySnapshot) -> super::Target {
    super::Target {
        slot: 0, source_id: row.source_id, class: row.class, model: row.model,
        action: row.action, phase: row.record[74], flags: row.control_flags,
        life: row.life, position: row.position, yaw: row.yaw, half_width: row.array_pitch(),
        next: word(row, 56), attacker: word(row, 42), killer: word(row, 40),
        route_index: row.record[76], route_relation: word(row, 78),
        route_target: word(row, 154), parent: word(row, 44), route_mode: row.record[77],
        relations: crate::native_world::source_target::Relations {
            target: Some(word(row, 154)), attacker: Some(word(row, 42)), parent: Some(word(row, 44)),
            stage_mode: Some(row.record[77]), stage_index: Some(row.record[76]),
            stage_relation: Some(word(row, 78)), next: Some(word(row, 56)),
        },
        source_yaw: Some(row.yaw), source_roll: Some(row.source_roll()), pitch_extent: Some(row.array_pitch()),
    }
}

fn materialize_water_splash(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let Some(splash) = arena
        .economy
        .initialize_authored_model5_water_splash(slot, position, terrain)
    else {
        return Err(format!(
            "model28 water splash map linkage failed at allocated slot{slot}"
        ));
    };
    Ok(Some(splash.source_entity_index))
}

/// One model28 source callback. State is written before the integrator
/// consumes its melee/spawn requests in the same-pass source tail.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_context: crate::native_world::source_target::Context<'_>,
    animations: &crate::source_animation::SourceAnimationResidency,
    animation_action: Option<&mut crate::source_animation::SourceAnimationAction<'_>>,
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
) -> Result<Effects, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .cloned()
        .ok_or_else(|| format!("model28 callback lost slot{slot}"))?;
    #[cfg(test)]
    let callback_entry = CALLBACK_TRACE.with(|trace| {
        let trace = trace.borrow();
        let (wanted, _) = trace.as_ref()?;
        if arena.handle(slot) != Some(*wanted) { return None; }
        let mailbox = (i32::from_le_bytes(row.record[98..102].try_into().expect("source mailbox")), word(&row, 102));
        let entry_player = source_context.player.zip(source_context.player_orientation)
            .map(|((slot, position, _), (yaw, _))| (slot, position, yaw));
        Some((*wanted, row.clone(), entry_player, crate::native_world::area_damage::current_actor_mailbox_writer(*wanted, mailbox)))
    });
    let mut actor = load(&row).ok_or_else(|| format!("model28 row{slot} is malformed"))?;
    let read_error = std::cell::RefCell::new(None);
    let lookup = |target_slot| match current_target(arena, source_context, target_slot) {
        Ok(target) => target,
        Err(error) => { *read_error.borrow_mut() = Some(error); None }
    };
    let animation_action = std::cell::RefCell::new(animation_action);
    let animation_count = || {
        if let Some(action) = animation_action.borrow_mut().as_deref_mut() {
            action(SOURCE_ATTACK_ANIMATION_TMAP, false)?;
        }
        animations.resident_frame_count_read(SOURCE_ATTACK_ANIMATION_TMAP)
    };
    let frame = Frame { terrain, targets: &[], current_target: Some(&lookup),
        sentinel: arena.entities.first().map(raw_sentinel), class3, families: Some(families),
        attack_animation_count: Some(&animation_count), same_model: &families[usize::from(SOURCE_MODEL)] };
    let source_counter = actor.source_counter;
    let result = super::step(&mut actor, &frame, runtime, particles);
    if let Some(error) = read_error.into_inner() { return Err(error); }
    let mut callback = result?;
    let position = actor.position;
    let rng = actor.rng;
    store(&mut row, actor, particles)?;
    arena.economy.source_rng.insert(slot, rng);
    arena.entities[usize::from(slot)] = row;
    if callback.population_decrement { arena.economy.decrement_source_creature_population(); }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let splash_slot = if callback.water_splash { materialize_water_splash(arena, terrain, position)? } else { None };
    #[cfg(test)]
    let splash = splash_slot.and_then(|slot| arena.handle(slot));
    #[cfg(not(test))]
    let _ = splash_slot;
    if callback.prekill {
        callback.notices.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 224)?;
    }
    if callback.kill && source_counter & 7 == 0 {
        crate::native_world::creatures::creature_24::mc2::kill(arena, terrain, particles, slot)?;
    }
    for (child_slot, flags) in callback.child_fade_flags {
        let Some(child) = arena
            .entities
            .get_mut(usize::from(child_slot))
            .filter(|child| child.active)
        else {
            continue;
        };
        child.set_control_flags((child.control_flags & 0x00ff_ffff) | (u32::from(flags) << 24));
    }
    if callback.stage_mana_expiry {
        super::super::creature_24::mc2::expire(
            arena,
            terrain,
            particles,
            slot,
            callback.stage_mana_mode13,
        )?;
    } else if callback.stage_hide {
        if arena.entities[usize::from(slot)].active {
            arena.entities[usize::from(slot)].disable_drawing();
        }
    }
    #[cfg(test)]
    if let Some((actor, before, entry_player, entry_writer)) = callback_entry {
        let after = (arena.handle(slot) == Some(actor)).then(|| arena.entities[usize::from(slot)].clone());
        if let Some(after) = &after {
            let mailbox = (i32::from_le_bytes(before.record[98..102].try_into().expect("source mailbox")), word(&before, 102));
            if mailbox.1 != 0 && word(after, 102) == 0 {
                let _ = crate::native_world::area_damage::take_player_mailbox_writer(actor, mailbox);
            }
        }
        CALLBACK_TRACE.with(|trace| {
            if let Some((wanted, rows)) = trace.borrow_mut().as_mut()
                && *wanted == actor && rows.len() < 64 {
                rows.push(CallbackDamageReceipt { actor, before, after, entry_player, entry_writer,
                    melee: callback.notices.melee, kill_credit: callback.notices.kill_credit, splash });
            }
        });
    }
    Ok(callback.notices)
}

#[cfg(test)]
mod alliance_tests {
    use super::*;
    use super::super::{SOURCE_ATTACK_ACTION, SOURCE_RECOVERY_DESCRIPTOR};

    #[test]
    fn configured_generic_damage_settles_only_source_reached_paths() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let runtime = session.runtime();
        let particles = &session.session.definition.presentation.particles;
        let mut position = Position { x: 20_000, y: 20_000, z: 0 };
        position.z = terrain.ground_height(position).wrapping_add(1024);
        let row = construct_row(40, 1, position, 1, particles).unwrap();
        let initial = load(&row).unwrap();
        let frame = Frame { terrain, targets: &[], current_target: None, sentinel: None,
            class3: &[], families: None, attack_animation_count: None, same_model: &[] };
        for (action, fatal, expected_descent) in [(224, false, true), (224, true, true),
            (SOURCE_IDLE_ACTION, false, true), (SOURCE_IDLE_ACTION, true, false)] {
            let mut actor = initial;
            actor.action = action;
            actor.pending_attacker = if action == 224 && !fatal { 0 } else { 41 };
            actor.pending_damage = if fatal { actor.life + 1 } else { 1 };
            super::super::step(&mut actor, &frame, runtime, particles).unwrap();
            assert_eq!(actor.position.z, if expected_descent { position.z - 256 } else { position.z },
                "source224 always settles; source225 settles only its nonfatal hit");
            if fatal { assert_eq!(actor.action, super::super::SOURCE_DEATH_ACTION); }
        }
    }

    #[test]
    fn configured_roam_rng_precedes_byte39_and_scan_reads_frozen_current_slot() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let runtime = session.runtime();
        let particles = &session.session.definition.presentation.particles;
        let position = Position { x: 20_000, y: 20_000, z: 0 };
        let row = construct_row(40, 1, position, 16, particles).unwrap();
        let mut initial = load(&row).unwrap();
        initial.flags |= 0x800;
        initial.yaw = 0;
        initial.roll = 0;
        let mut target_position = position;
        crate::move_entity(&mut target_position, 0, 0, 100);
        // Configured current replacement in an existing frozen class3 slot:
        // source1BF90 reads position/flags only; it does not re-admit life,
        // visibility or class after the frozen list was built.
        let target = super::super::Target {
            slot: 41, source_id: 41, class: 10, model: 5, action: 0, phase: 0,
            flags: 0x400, life: -1, position: target_position, yaw: 0, half_width: 128,
            next: 0, attacker: 0, killer: 0, route_index: 0, route_relation: 0,
            route_target: 0, parent: 0, route_mode: 0, relations: Default::default(),
            source_yaw: Some(0), source_roll: Some(0), pitch_extent: Some(128),
        };
        let targets = [target];
        let frame = Frame { terrain, targets: &targets, current_target: None, sentinel: None,
            class3: &[41], families: None, attack_animation_count: None, same_model: &[] };
        let mut dormant = initial;
        dormant.target_scan_limit = 0;
        let expected_rng = initial.rng.wrapping_mul(9_377).wrapping_add(9_439)
            .wrapping_mul(9_377).wrapping_add(9_439);
        super::super::step(&mut dormant, &frame, runtime, particles).unwrap();
        assert_eq!(dormant.rng, expected_rng, "source consumes both cadence RNG values with byte39 zero");
        assert_eq!(dormant.action, SOURCE_IDLE_ACTION);

        let mut enabled = initial;
        enabled.target_scan_limit = 1;
        super::super::step(&mut enabled, &frame, runtime, particles).unwrap();
        assert_eq!(enabled.rng, expected_rng);
        assert_eq!((enabled.action, enabled.target), (SOURCE_ATTACK_ACTION, 41),
            "the frozen class3 slot resolves its current occupant without an added live/class guard");
    }

    #[test]
    fn configured_parent_loss_hides_current_actor_without_early_map_unlink() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let runtime = session.runtime().clone();
        let particles = &session.session.definition.presentation.particles;
        let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let position = session.state.position;
        let mut row = construct_row(slot, arena.new_event_rng(slot), position, 1, particles).unwrap();
        row.set_source_action(231);
        row.record[77] = 12;
        put_word(&mut row, 44, 0);
        arena.entities[usize::from(slot)] = row;
        crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
        let handle = arena.handle(slot).unwrap();
        let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        families[usize::from(SOURCE_MODEL)].push(slot);
        step_row(&mut arena, &runtime, &mut terrain, particles, Default::default(),
            &crate::source_animation::SourceAnimationResidency::default(), None, slot, &[], &families).unwrap();
        assert_eq!(arena.handle(slot), Some(handle));
        assert!(arena.entities[usize::from(slot)].drawing_disabled());
        assert!(arena.economy.map_chains.is_linked(slot),
            "source57F10 retains the current hidden map row until following-pass reclamation");
    }

    #[test]
    fn configured_source_movement_publishes_proposal_and_hold_preserves_it() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain().clone();
        let particles = &session.session.definition.presentation.particles;
        let mut position = Position { x: 20_000, y: 20_000, z: 0 };
        position.z = terrain.ground_height(position);
        let row = construct_row(40, 1, position, 1, particles).unwrap();
        let mut actor = load(&row).unwrap();
        actor.flags |= 0x800;
        actor.speed = 0;
        let prior = Position { x: 12_345, y: 23_456, z: 789 };
        terrain.set_source_predicted_position(prior);
        super::super::move_actor(&mut actor, &terrain);
        assert_eq!(terrain.source_predicted_position(), prior,
            "source hold clears the bit without writing the shared prediction");
        super::super::move_actor(&mut actor, &terrain);
        assert_eq!(terrain.source_predicted_position(), actor.position,
            "the actual accepted source proposal replaces the shared prediction");
        assert_ne!(terrain.source_predicted_position(), prior);
    }

    #[test]
    fn level14_authored_creature28_transforms_splashes_and_hits_player() {
        use crate::campaign::{CampaignSession, source_arena::CreatureRootOrigin, spec::CampaignSelection};
        use crate::native_world::{Mc2DispatchEvent, area_damage, Level2EconomyEvent};
        use crate::{PlayerCommand, level_definition::CreatureKind};

        let package = std::env::var_os("MC_LEVEL14_PACKAGE").map_or_else(
            || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level14"),
            std::path::PathBuf::from,
        );
        let owner_cache = std::env::var_os("MC_LEVEL14_OWNER_CACHE")
            .expect("MC_LEVEL14_OWNER_CACHE must name the qualified Level14 owner animation cache");
        let mut session = CampaignSession::open_with_owner_cache(package, owner_cache, CampaignSelection::ordinary(14).unwrap())
            .expect("whole retained Level14 owner package").simulation().clone();
        let player = session.state.entity_index;
        let player_handle = session.source_arena().handle(player).unwrap();
        let switch = session.source_arena().handle(499).unwrap();
        let trigger = &session.entities()[499];
        assert_eq!((trigger.class, trigger.model, trigger.action, trigger.source_id, trigger.level_gate),
            (11, 32, 32, 16, 1));
        assert_eq!(switch.generation, 0);
        for authored_record in [511, 512] {
            assert!(session.mission().authored_features().iter().any(|feature|
                feature.index == authored_record && (feature.kind, feature.subtype, feature.activation_group) == (5, 28, 16)));
            assert!(!session.creature_runtime_census().roots_of_kind(CreatureKind::Creature28)
                .any(|root| root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record })));
        }
        // Player-only survival control. Source callbacks naturally age the
        // existing startup protection; no actor, timer or RNG is edited.
        {
            let live = &mut session.session.state.live;
            live.player_life = 200_000;
            live.shared_spells.set_player_entity_life(200_000);
            let parent = &mut live.economy.entities[usize::from(player)];
            parent.set_max_life(200_000);
            parent.set_life(200_000);
            let parent = parent.clone();
            live.shared_spells.sync_player_entity_from_authority(&parent);
        }
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        for turn in 0..256 {
            if session.session.state.live.player_protection_ticks == 0 { break; }
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "protection turn {turn}: {:?}", session.dispatch_events());
        }
        assert_eq!(session.session.state.live.player_protection_ticks, 0);
        assert_eq!(session.source_arena().handle(499), Some(switch));
        // Declared prior objective completion; the actual source switch still
        // publishes the complete group, including all other authored actors.
        let color = usize::from(session.players()[0].player_color_index());
        assert_eq!(session.runtime().objectives[color].stage_selectors[1], 1);
        session.session.state.live.core.runtime.set_level2_objective_stage_selector_for(color, 1, 2);
        let mut produced = false;
        for turn in 0..32 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "producer turn {turn}: {:?}", session.dispatch_events());
            if session.dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 499, group: 16, authored_indices, .. }
                    if authored_indices.contains(&511) && authored_indices.contains(&512))) {
                produced = true;
                break;
            }
        }
        assert!(produced, "actual switch499 must publish whole group16");
        let identity = session.creature_runtime_census().roots_of_kind(CreatureKind::Creature28)
            .find(|root| root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 511 }))
            .expect("current original authored511 generation");
        assert!(identity.living);
        let root = identity.handle;
        let original = session.entities()[usize::from(root.source_slot)].clone();
        assert_eq!((original.class, original.model, original.max_life), (5, 28, 8_000));
        assert!(session.campaign_entity_presentations(0).unwrap().iter().any(|view|
            view.entity_index == root.source_slot && view.render_parameter_index == SOURCE_IDLE_VISUAL));
        assert_eq!(session.canonical_terrain().terrain_type(original.position), 0,
            "the retained actual authored position must be water");
        // A fixed player-only lure ahead of the actor admits natural targeting.
        // A separate single contact-boundary placement is declared below;
        // actor movement, target, animation, timing and RNG remain source-owned.
        let mut approach = original.position;
        crate::move_entity(&mut approach, original.yaw, 0, 768);
        approach.z = approach.z.wrapping_add(384);
        session.set_level2_entity_position(player, approach).unwrap();
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        let journal_start = session.effect_journal().cursor(0).next_sequence;
        area_damage::start_player_damage_trace(player_handle);
        let mut saw_recovery = false;
        let mut saw_attack_form = false;
        let mut saw_attack_dto = false;
        let mut saw_splash = false;
        let mut saw_splash_dto = false;
        let mut hit = None;
        let mut observations = Vec::new();
        let mut melee_requests = Vec::new();
        let mut consumptions = Vec::new();
        let mut matching_writers = Vec::new();
        let mut contact_setup = None;
        for turn in 0..320 {
            start_callback_damage_trace(root);
            session.step_outer_turn(PlayerCommand::default(), 0);
            let callbacks = take_callback_damage_trace();
            assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "combat turn {turn}: {:?}", session.dispatch_events());
            assert_eq!(session.source_arena().handle(root.source_slot), Some(root));
            assert_eq!(session.source_arena().handle(player), Some(player_handle));
            assert!(session.session.state.live.player_life > 0,
                "the declared player-survival control must remain alive on turn {turn}");
            if let Some(writer) = area_damage::latest_player_mailbox_writer()
                && writer.source == area_damage::PlayerMailboxSource::Contact(root)
                && matching_writers.len() < 16 {
                matching_writers.push((turn, writer));
            }
            let presentations = session.campaign_entity_presentations(0).unwrap();
            saw_attack_dto |= presentations.iter().any(|view|
                view.entity_index == root.source_slot && view.render_parameter_index == SOURCE_ATTACK_VISUAL);
            for callback in callbacks {
                let Some(after) = callback.after else { continue };
                let before = &callback.before;
                if let Some(melee) = callback.melee
                    && melee_requests.len() < 32 {
                    melee_requests.push((turn, melee, callback.entry_player));
                }
                if observations.len() < 96 && (turn < 4 || before.action != after.action
                    || before.record[74] != after.record[74] || callback.melee.is_some()
                    || (before.action == SOURCE_ATTACK_ACTION && before.record[74] == 4
                        && before.source_dword_10() > 0 && before.source_dword_10() & 3 == 3)) {
                    observations.push((turn,
                        (before.action, before.record[74], word(before, 154), before.source_dword_10(),
                            word(before, 48), before.position, before.yaw, before.source_roll()),
                        (after.action, after.record[74], word(&after, 154), after.source_dword_10(),
                            word(&after, 48), after.position, after.yaw, after.source_roll()),
                        callback.entry_player, callback.melee));
                }
                if callback.before.action == SOURCE_ATTACK_ACTION && callback.before.record[74] == 0 {
                    saw_recovery |= after.parameter_relation == Some(SOURCE_RECOVERY_DESCRIPTOR) && after.record[74] == 1;
                }
                if callback.before.action == SOURCE_ATTACK_ACTION && callback.before.record[74] == 3 {
                    saw_attack_form |= after.parameter_relation == Some(SOURCE_DESCRIPTOR)
                        && after.particle_parameter_index == Some(SOURCE_ATTACK_VISUAL);
                    if contact_setup.is_none() && after.record[74] == 4 {
                        assert_eq!(word(&after, 48), 24, "the qualified real resident header supplies 24 frames");
                        let current = &session.entities()[usize::from(root.source_slot)];
                        assert_eq!((current.action, current.record[74], current.position),
                            (SOURCE_ATTACK_ACTION, 4, after.position), "place the player only at the actual current attack boundary");
                        let heading = word(&after, 52) as i16;
                        let mut position = after.position;
                        crate::move_entity(&mut position, heading, 0, 512);
                        position.z = session.canonical_terrain().ground_height(position).wrapping_add(256);
                        session.set_level2_entity_position(player, position).unwrap();
                        session.state.actual_speed = 0;
                        session.state.target_speed = 0;
                        contact_setup = Some((turn, heading, position));
                    }
                }
                if let Some(splash) = callback.splash {
                    saw_splash |= session.source_arena().handle(splash.source_slot) == Some(splash)
                        && session.economy().authored_model5_water_splashes().iter().any(|effect|
                            effect.source_entity_index == splash.source_slot && effect.active
                                && (effect.source_class, effect.model, effect.action) == (10, 5, 5));
                    saw_splash_dto |= session.source_arena().handle(splash.source_slot) == Some(splash)
                        && presentations.iter().any(|view| view.entity_index == splash.source_slot
                            && view.render_parameter_index == 244);
                }
            }
            for consumed in area_damage::take_player_damage_consumptions() {
                if consumptions.len() < 24 { consumptions.push((turn, consumed)); }
                let Some(writer) = consumed.writer else { continue };
                if writer.source != area_damage::PlayerMailboxSource::Contact(root)
                    || consumed.target != player_handle || writer.target != player_handle
                    || writer.mailbox != consumed.mailbox { continue; }
                if consumed.life_after == consumed.life_before.wrapping_sub(consumed.mailbox.0)
                    && matches!(session.economy().events().get(consumed.event_cursor),
                        Some(Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, lethal: false })
                            if *target_entity_index == player && *attacker_entity_index == original.source_id && *damage == consumed.mailbox.0)
                { hit = Some(consumed); }
            }
            if saw_recovery && saw_attack_form && saw_attack_dto && saw_splash && saw_splash_dto && hit.is_some() { break; }
        }
        assert!(contact_setup.is_some() && saw_recovery && saw_attack_form && saw_attack_dto && saw_splash && saw_splash_dto && hit.is_some(),
            "actual authored511 must transform, emit its water splash and damage the player: recovery={saw_recovery} attack={saw_attack_form} attack_dto={saw_attack_dto} splash={saw_splash} splash_dto={saw_splash_dto} hit={hit:?} initial={original:?} lure={approach:?} one_contact_placement={contact_setup:?} observations={observations:?} melee_requests={melee_requests:?} matching_writers={matching_writers:?} player_consumptions={consumptions:?}");
        assert!(session.effect_journal().records().iter().any(|record| record.sequence >= journal_start && matches!(
            &record.effect, crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::Creature28Attack, entity_index, sound_id: 38, player_index: -1,
            }) if *entity_index == root.source_slot
        ) && record.audio.is_some_and(|audio| audio.command.audible && audio.play_type == 1)),
            "the same source generation must publish its audible attack sound");
    }

    #[test]
    fn configured_attack_reads_resident_frame_count_only_after_nonfatal_damage() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let runtime = session.runtime();
        let particles = &session.session.definition.presentation.particles;
        let position = Position { x: 20_000, y: 20_000, z: 0 };
        let row = construct_row(40, 1, position, 1, particles).unwrap();
        let mut actor = load(&row).unwrap();
        actor.action = SOURCE_ATTACK_ACTION;
        actor.phase = 3;
        actor.timer = 77;
        actor.parent = 99;
        actor.source_animation = 99;
        actor.flags |= 0x800;
        let read = || Ok(Some(12));
        let frame = Frame { terrain, targets: &[], current_target: None, sentinel: None,
            class3: &[], families: None, attack_animation_count: Some(&read), same_model: &[] };
        let result = super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!((actor.phase, actor.parent, actor.source_animation, actor.timer), (4, 99, 12, 11));
        assert_eq!(result.notices.sound, Some(38));
        assert_eq!(result.notices.sound_origin.unwrap().position, position);

        actor.phase = 3;
        actor.timer = 77;
        actor.parent = 99;
        actor.source_animation = 99;
        actor.flags |= 0x800;
        let absent = || Ok(None);
        let frame = Frame { attack_animation_count: Some(&absent), ..frame };
        super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!((actor.phase, actor.parent, actor.source_animation, actor.timer), (6, 99, 0, 0),
            "source state4 first clears timer; an unresident animation immediately enters state6");

        actor.phase = 3;
        actor.life = 1;
        actor.pending_attacker = 41;
        actor.pending_damage = 2;
        let forbidden = || -> Result<Option<u16>, String> { panic!("fatal damage must return before header read") };
        let frame = Frame { attack_animation_count: Some(&forbidden), ..frame };
        let result = super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!(actor.action, super::super::SOURCE_DEATH_ACTION);
        assert!(result.notices.sound_origin.is_none());
    }

    #[test]
    fn configured_attack_approach_respects_source_cadence_timer_and_peers() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let terrain = session.canonical_terrain();
        let runtime = session.runtime();
        let particles = &session.session.definition.presentation.particles;
        let position = Position { x: 20_000, y: 20_000, z: 0 };
        let row = construct_row(40, 1, position, 1, particles).unwrap();
        let mut actor = load(&row).unwrap();
        actor.action = SOURCE_ATTACK_ACTION;
        actor.phase = 2;
        actor.target = 41;
        actor.flags |= 0x800;
        actor.roll = 321;
        let target = super::super::Target {
            slot: 41, source_id: 41, class: 3, model: 0, action: 0, phase: 0,
            flags: 0, life: 100, position, yaw: 0, half_width: 128,
            next: 0, attacker: 0, killer: 0, route_index: 0, route_relation: 0,
            route_target: 0, parent: 0, route_mode: 0,
            relations: Default::default(), source_yaw: Some(0), source_roll: Some(0),
            pitch_extent: Some(128),
        };
        let targets = [target];
        let frame = Frame { terrain, targets: &targets, current_target: None, sentinel: None,
            class3: &[], families: None, attack_animation_count: None, same_model: &[] };
        actor.timer = 2;
        super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!((actor.phase, actor.timer, actor.roll), (2, 1, 321),
            "off-cadence source preserves heading and cannot enter the attack");

        actor.flags |= 0x800;
        actor.source_counter = 4;
        actor.timer = 20;
        super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!((actor.phase, actor.timer), (2, 19),
            "source waits until the decremented timer is below 14");

        actor.flags |= 0x800;
        actor.timer = 2;
        super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!((actor.phase, actor.timer), (3, 0),
            "a close on-cadence approach selects state3 and clears the source timer");

        let peer = super::super::Target { slot: 42, source_id: 42, class: 5, model: 28,
            position: Position { x: position.x + 64, ..position }, ..target };
        let targets = [target, peer];
        let frame = Frame { terrain, targets: &targets, current_target: None, sentinel: None,
            class3: &[], families: None, attack_animation_count: None, same_model: &[40, 42] };
        actor.phase = 2;
        actor.flags |= 0x800;
        actor.timer = 20;
        super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!(actor.roll, crate::heading_for_positions(peer.position, position) as i16,
            "the first overlapping current peer overrides the target heading");
        actor.target = 0;
        actor.phase = 2;
        actor.timer = 7;
        actor.source_counter = 1;
        super::super::step(&mut actor, &frame, runtime, particles).unwrap();
        assert_eq!((actor.phase, actor.timer), (3, 0),
            "source missing-target transition also resets the timer");
    }

    #[test]
    fn impact_drives_model28_selected_target_and_restores_stage() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let runtime = session.session.state.live.core.runtime.clone();
        let particles = &session.session.definition.presentation.particles;
        let parent = session.state.entity_index;
        let position = session.state.position;
        let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let seed = arena.new_event_rng(slot);
        let mut row = construct_row(slot, seed, position, 1, particles).unwrap();
        row.record[76] = 3;
        arena.entities[usize::from(slot)] = row;
        crate::world::effects::sync_position(&mut arena, &mut terrain, slot, position, true, false).unwrap();
        let enemy = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
        let mut target = crate::native_world::cave_features::new_event(enemy, 5, position);
        target.class = 5; target.model = 21; target.record[67] = 5; target.record[68] = 21;
        target.set_life(1_000);
        arena.entities[usize::from(enemy)] = target;
        put_word(&mut arena.entities[usize::from(parent)], 154, enemy);
        let impact = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
        let effect = &mut arena.entities[usize::from(impact)];
        effect.source_id = parent; put_word(effect, 30, parent); effect.record[74] = 32;
        effect.subspell_index = 40; put_word(effect, 154, slot);
        assert_eq!(crate::native_world::alliance_spell::step_impact(&mut arena, impact, &[slot]).unwrap().affected_entities, vec![slot]);
        let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
        families[usize::from(SOURCE_MODEL)].push(slot);
        let animations = crate::source_animation::SourceAnimationResidency::default();
        let effects = step_row(&mut arena, &runtime, &mut terrain, particles,
            Default::default(), &animations, None, slot, &[parent], &families).unwrap();
        assert_eq!(effects.feedback, vec![(parent, 24)]);
        let row = &arena.entities[usize::from(slot)];
        assert_eq!((row.action, word(row, 154), row.record[77]), (226, enemy, 14));
        let row = &mut arena.entities[usize::from(slot)];
        row.set_source_action(231); put_word(row, 50, 1);
        step_row(&mut arena, &runtime, &mut terrain, particles,
            Default::default(), &animations, None, slot, &[parent], &families).unwrap();
        let row = &arena.entities[usize::from(slot)];
        assert_eq!((row.record[77], row.record[76], word(row, 44)), (10, 3, 0));
        assert!(!row.drawing_disabled());
    }
}
