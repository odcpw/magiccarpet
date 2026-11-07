//! Lossless MC2 adapter. Callback-local typed state is committed to the same
//! arena row before any allocation can replace its fixed address.

use super::{
    DESCRIPTOR, SOURCE_MODEL, SpiderContext, SpiderEffects, SpiderState, StageState, Target,
    bind_stage, stage_prepass,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    EntitySnapshot, MapType, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::{FlightWorld, Position};

pub(crate) const CALLBACK_SOUND: u16 = 32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceSound {
    pub slot: u16,
    pub position: Position,
    pub source_id: u16,
    pub control_flags: u32,
}

/// sub_25E40 prepares sound32 after its non-null target check and before
/// damage, movement, or a possible projectile allocation.
pub(crate) fn prepare_callback_sound(
    arena: &SourceArena,
    slot: u16,
) -> Result<Option<SourceSound>, String> {
    let row = arena.entities.get(usize::from(slot))
        .filter(|row| row.active && (row.class, row.model) == (5, SOURCE_MODEL))
        .ok_or_else(|| format!("Spider sound lost current slot {slot}"))?;
    if row.action != 162 { return Ok(None); }
    let target = word(row, 154);
    if target == 0 { return Ok(None); }
    if usize::from(target) >= arena.entities.len() {
        return Err(format!("Spider sound target address {target} is outside the arena"));
    }
    Ok(Some(SourceSound { slot, position: row.position, source_id: row.source_id,
        control_flags: row.control_flags }))
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}
fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn sentinel(arena: &SourceArena) -> Option<Target> {
    let row = arena.entities.first()?;
    Some(Target { slot: 0, source_id: row.source_id, class: row.class, model: row.model,
        life: row.life, flags: row.control_flags, position: row.position,
        yaw: row.yaw, roll: row.source_roll(), ..Default::default() })
}

pub(crate) fn load(row: &EntitySnapshot, generation: u32) -> SpiderState {
    SpiderState {
        slot: row.index,
        generation,
        source_id: row.source_id,
        player: row.source_player_entity_index(),
        rng: row.entity_rng,
        active: row.active,
        action: row.action,
        control_flags: row.control_flags,
        position: row.position,
        yaw: row.yaw,
        pitch: row.pitch,
        roll: word(row, 36) as i16,
        fov: row.fov,
        speed: row.actual_speed,
        minimum_speed: word(row, 136) as i16,
        maximum_speed: word(row, 138) as i16,
        max_life: row.max_life,
        life: row.life,
        mana: row.mana,
        max_mana: row.max_mana,
        damage: row.subspell_index,
        attack_phase: row.record[74],
        attack_timer: row.source_dword_10(),
        pending_damage: i32::from_le_bytes(
            row.record[98..102]
                .try_into()
                .expect("source mailbox dword"),
        ),
        pending_attacker: word(row, 102),
        attacker: word(row, 42),
        killer: word(row, 40),
        target: word(row, 154),
        previous: word(row, 54),
        next: word(row, 56),
        phase: row.phase,
        proximity: row.record[61],
        visual: row.particle_parameter_index.unwrap_or(288),
        animation_frame: row.record[96],
        animation_limit: row.record[97],
        half_width: word(row, 88) as i16,
        half_height: word(row, 92) as i16,
        vertical_extent: word(row, 86) as i16,
        stage: StageState {
            index: row.record[76],
            mode: row.record[77],
            relation: word(row, 78),
            parent: word(row, 44),
            remaining: word(row, 50) as i16,
            duration: word(row, 52) as i16,
        },
    }
}

pub(crate) fn store(
    row: &mut EntitySnapshot,
    actor: SpiderState,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if row.index != actor.slot {
        return Err("Spider carrier address changed".to_owned());
    }
    if row.particle_parameter_index != Some(actor.visual) || actor.animation_limit == 0 {
        let particle = particles
            .get(usize::from(actor.visual))
            .ok_or_else(|| format!("Spider requires particle{}", actor.visual))?;
        row.record[97] = particle
            .animation_limit()
            .ok_or_else(|| format!("Spider particle{} has invalid animation mode", actor.visual))?;
    } else {
        row.record[97] = actor.animation_limit;
    }
    row.source_id = actor.source_id;
    put(row, 30, actor.source_id);
    row.active = actor.active;
    row.record[2] = u8::from(actor.active);
    row.set_entity_rng(actor.rng);
    row.set_source_action(actor.action);
    row.set_control_flags(actor.control_flags);
    row.set_position(actor.position);
    row.set_yaw(actor.yaw);
    row.set_pitch(actor.pitch);
    row.set_source_roll(actor.roll);
    row.set_fov(actor.fov);
    row.set_actual_speed(actor.speed);
    put(row, 136, actor.minimum_speed as u16);
    put(row, 138, actor.maximum_speed as u16);
    row.set_max_life(actor.max_life);
    row.set_life(actor.life);
    row.set_mana(actor.mana);
    row.max_mana = actor.max_mana;
    row.mana_cost = actor.max_mana;
    row.record[144..148].copy_from_slice(&actor.max_mana.to_le_bytes());
    row.set_subspell_index(actor.damage);
    row.record[74] = actor.attack_phase;
    row.set_source_dword_10(actor.attack_timer);
    row.record[98..102].copy_from_slice(&actor.pending_damage.to_le_bytes());
    for (offset, value) in [
        (102, actor.pending_attacker),
        (42, actor.attacker),
        (40, actor.killer),
        (154, actor.target),
        (54, actor.previous),
        (56, actor.next),
        (152, actor.player),
        (78, actor.stage.relation),
        (44, actor.stage.parent),
        (50, actor.stage.remaining as u16),
        (52, actor.stage.duration as u16),
        (88, actor.half_width as u16),
        (90, actor.half_width as u16),
        (92, actor.half_height as u16),
        (86, actor.vertical_extent as u16),
        (94, actor.visual),
    ] {
        put(row, offset, value);
    }
    row.active_value = actor.stage.remaining;
    row.activation_value = actor.stage.duration as u16;
    row.set_phase(actor.phase);
    row.record[61] = actor.proximity;
    row.particle_parameter_index = Some(actor.visual);
    row.record[96] = actor.animation_frame;
    row.record[76] = actor.stage.index;
    row.record[77] = actor.stage.mode;
    Ok(())
}

pub(crate) fn construct_row(
    slot: u16,
    generation: u32,
    seed: u16,
    position: Position,
    map_type: MapType,
    counter: &mut u8,
    particles: &[ParticleParameter],
) -> Result<Option<EntitySnapshot>, String> {
    let particle = particles
        .get(288)
        .ok_or_else(|| "Spider requires retained particle288".to_owned())?;
    if particle.animation_limit().is_none() {
        return Err("Spider particle288 has invalid animation mode".to_owned());
    }
    let mut actor = super::construct(slot, generation, seed, position, map_type, counter)
        .expect("Spider constructor");
    actor.control_flags |= 4;
    actor.vertical_extent = (particle.rotation_speed / 2) as i16;
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.parameter_relation = Some(DESCRIPTOR);
    put(&mut row, 0, slot);
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.record[69] = 3;
    row.record[70] = 255;
    row.record[71] = 10;
    row.record[60] = 1;
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR) + 1).to_le_bytes());
    row.record[168] = 2;
    row.record[172..176].copy_from_slice(&1_u32.to_le_bytes());
    store(&mut row, actor, particles)?;
    Ok(Some(row))
}

fn targets(arena: &SourceArena, direct: &std::collections::BTreeSet<u16>) -> Vec<Target> {
    let mut targets = super::super::creature_15::mc2::targets_in_arena(arena)
        .into_iter()
        .map(|row| Target {
            slot: row.slot,
            source_id: row.source_id,
            class: row.class,
            model: row.model,
            action: row.action,
            life: row.life,
            flags: row.control_flags,
            position: row.position,
            yaw: row.yaw as i16,
            roll: row.roll as i16,
            previous: row.previous,
            next: row.next,
            target: row.target,
            attacker: row.attacker,
            killer: row.killer,
            half_width: row.half_width,
            speed: 0,
            maximum_speed: 0,
            stage: row.stage,
        })
        .collect::<Vec<_>>();
    // Raw rows newly allocated by this family have nonzero generations and
    // must not disappear behind the imported-only filter in old family views.
    let typed = arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|row| {
        (row.active && row.class != 0 || direct.contains(&row.index))
            && row.index != 0
            && !typed.iter().any(|(slot, _)| *slot == row.index)
    }) {
        let actor = load(row, arena.slot_generation(row.index));
        let view = Target {
            slot: row.index,
            source_id: row.source_id,
            class: row.class,
            model: row.model,
            action: row.action,
            life: row.life,
            flags: row.control_flags,
            position: row.position,
            yaw: row.yaw,
            roll: actor.roll,
            previous: actor.previous,
            next: actor.next,
            target: actor.target,
            attacker: actor.attacker,
            killer: actor.killer,
            half_width: actor.half_width,
            speed: actor.speed,
            maximum_speed: actor.maximum_speed,
            stage: actor.stage,
        };
        if let Some(old) = targets.iter_mut().find(|t| t.slot == row.index) {
            *old = view;
        } else {
            targets.push(view);
        }
    }
    targets.sort_unstable_by_key(|t| t.slot);
    targets
}

pub(crate) fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|r| r.active && r.class == 5 && r.model == SOURCE_MODEL)
        .ok_or_else(|| format!("Spider StageVar bind lost slot{slot}"))?;
    let mut actor = load(row, arena.slot_generation(slot));
    bind_stage(&mut actor, index, runtime);
    store(&mut arena.entities[usize::from(slot)], actor, particles)
}

/// Ordinary sub_4A310 constructor. Allocation failure leaves the counter and
/// StageVar table untouched; later failures preserve already published work.
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
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "Spider source constructor counter is absent".to_owned())?[20];
    let Some(particle) = particles.get(288) else {
        return Err("Spider requires retained particle288".to_owned());
    };
    if particle.animation_limit().is_none() {
        return Err("Spider particle288 has invalid animation mode".to_owned());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = construct_row(
        slot,
        arena.slot_generation(slot),
        arena.new_event_rng(slot),
        position,
        terrain.map_type(),
        &mut counter,
        particles,
    )?
    .expect("Spider constructor");
    row.bind_frame_zero_max_life(max_life);
    // The immutable constructor default mana remains2750 when authored health
    // is changed, exactly as for the other typed creature health bindings.
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 20] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_authored_route(arena, runtime, slot, index, particles)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Spider constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Spider,
        crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "Spider record{authored_record} lost generation-bound origin"
        ));
    }
    Ok(Some(slot))
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &SessionTerrain,
    families: &[Vec<u16>; 29],
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if families[usize::from(SOURCE_MODEL)].is_empty() {
        return Ok(());
    }
    let targets = targets(arena, &Default::default());
    for slot in &families[usize::from(SOURCE_MODEL)] {
        let Some(row) = arena
            .entities
            .get(usize::from(*slot))
            .filter(|r| r.active && r.class == 5 && r.model == SOURCE_MODEL)
        else {
            continue;
        };
        let mut actor = load(row, arena.slot_generation(*slot));
        let mut context = SpiderContext {
            terrain,
            targets: &targets,
            sentinel: sentinel(arena),
            class3: &[],
            model20: &families[usize::from(SOURCE_MODEL)],
            class5: &[],
            stage_runtime: Some(runtime),
            #[cfg(test)]
            damage_trace: None,
        };
        stage_prepass(&mut actor, &mut context);
        store(&mut arena.entities[usize::from(*slot)], actor, particles)?;
    }
    Ok(())
}

/// Single-player sub_68BF0/68C70. This owns byte39/3A on the same raw rows;
/// the source scheduler supplies the frozen family and the local player pose.
pub(crate) fn proximity_prepass(arena: &mut SourceArena, frozen: &[u16], player: Position) {
    arena.step_raw_creature_proximity(frozen, player);
}

/// Apply this callback only. The orchestrator owns byte3E and next-pass reclaim.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    source_context: crate::native_world::source_target::Context<'_>,
    player_feedback: &mut crate::native_world::source_target::Creature23PlayerFeedback<'_>,
) -> Result<SpiderEffects, String> {
    let class5 = families.iter().flatten().copied().collect::<Vec<_>>();
    let generation = arena.slot_generation(slot);
    #[cfg(test)]
    let damage_trace = arena.handle(slot).map(|handle| (handle, arena.economy.events().len()));
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|r| r.active && r.class == 5 && r.model == SOURCE_MODEL)
        .ok_or_else(|| format!("Spider callback lost slot{slot}"))?;
    let initial_action = row.action;
    let mut actor = load(row, generation);
    let mut direct = crate::native_world::source_target::alliance_direct_references(arena, row)?;
    // Source action162 retains a fixed target address even if that record is
    // no longer an active member of the frozen search lists.
    if actor.target != 0 { direct.insert(actor.target); }
    if actor.previous != 0 { direct.insert(actor.previous); }
    if actor.next != 0 { direct.insert(actor.next); }
    if actor.stage.parent != 0 { direct.insert(actor.stage.parent); }
    if actor.attacker != 0 { direct.insert(actor.attacker); }
    if actor.killer != 0 { direct.insert(actor.killer); }
    let mut targets = targets(arena, &direct);
    if actor.action == 162 && actor.target != 0 {
        let current = crate::native_world::source_target::read(arena, player_feedback.context(source_context), actor.target)?
            .ok_or_else(|| format!("Spider target address {} is unavailable", actor.target))?;
        let mut target = targets.iter().find(|row| row.slot == actor.target).copied()
            .unwrap_or(Target { slot: actor.target, ..Default::default() });
        target.source_id = current.source_id;
        target.class = current.class;
        target.model = current.model;
        target.life = current.life;
        target.flags = current.control_flags;
        target.position = current.position;
        if let Some(row) = targets.iter_mut().find(|row| row.slot == actor.target) { *row = target; }
        else { targets.push(target); }
    }
    let mut context = SpiderContext {
        terrain,
        targets: &targets,
        sentinel: sentinel(arena),
        class3,
        model20: &families[usize::from(SOURCE_MODEL)],
        class5: &class5,
        stage_runtime: Some(runtime),
        #[cfg(test)]
        damage_trace,
    };
    let mut effects = super::step(&mut actor, &mut context)
        .ok_or_else(|| format!("unsupported Spider action{}", actor.action))?;
    store(&mut arena.entities[usize::from(slot)], actor, particles)?;
    arena.economy.source_rng.insert(slot, actor.rng);
    // 1B8C0's blocked-death population write precedes any ranged NewEvent.
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    // Linked writes from source1C560/1ED30 happen before any ranged NewEvent.
    for update in &effects.linked_updates {
        if matches!(initial_action, 162 | 167) {
            // Alliance blink follows fixed current addresses, including a
            // linked address now owned by another typed carrier.
            let before = crate::native_world::source_target::status_parent(
                arena, player_feedback.context(source_context), update.slot,
            )?.ok_or_else(|| format!("Spider linked address {} is unavailable", update.slot))?;
            let mut after = before.clone();
            after.set_control_flags(update.flags);
            player_feedback.commit_flags(arena, &before, &after)?;
        } else if arena.economy.live_arena_kind(update.slot).is_none() {
            let row = arena.entities.get_mut(usize::from(update.slot))
                .ok_or_else(|| format!("Spider linked address {} is outside the arena", update.slot))?;
            row.set_source_action(update.action);
            row.set_control_flags(update.flags);
            put(row, 154, update.target);
            put(row, 54, update.previous);
        } else {
            return Err(format!("Spider linked action address {} has an unsupported typed owner", update.slot));
        }
    }
    let mut launched = false;
    if let Some(target) = effects.web_target {
        launched = super::web::launch(arena, terrain, particles, slot, target, player_feedback.context(source_context))?.is_some();
        if launched {
            effects.web_launched_target = Some(target);
            player_feedback.apply(arena, source_context, target)?;
        }
    }
    if let Some(target) = effects.phase0_target {
        // 25E40 retains both fixed addresses through the actual NewEvent:
        // evaluate the current target after the call even when no shot was
        // allocated, then write the current actor address.
        let current_target = crate::native_world::source_target::read(arena, player_feedback.context(source_context), target)?
            .ok_or_else(|| format!("Spider attack lost current target address {target}"))?;
        let charge = if (current_target.class, current_target.model) == (3, 0) {
            player_feedback.spider_immobilize_counter(arena, source_context, target)? != 0
        } else { launched };
        let current = &mut arena.entities[usize::from(slot)];
        if charge { current.record[74] = 1; }
    }
    if effects.phase0_target.is_some() {
        let current = &mut arena.entities[usize::from(slot)];
        if current.action != 162 { current.set_actual_speed(word(current, 136) as i16); }
    }
    if effects.kill {
        kill(arena, terrain, particles, slot, player_feedback.context(source_context))?;
    }
    if effects.expire {
        expire(arena, terrain, particles, slot, effects.summon_expire, player_feedback.context(source_context))?;
    }
    Ok(effects)
}

fn hide_current(arena: &mut SourceArena, slot: u16) {
    if let Some(s) = arena
        .economy
        .spheres
        .iter_mut()
        .find(|s| s.active && s.source_entity_index == slot)
    {
        s.control_flags |= super::ENTITY_DRAW_DISABLED_BIT;
        s.pending_delete = true;
    } else {
        arena.entities[usize::from(slot)].disable_drawing();
    }
}

fn kill(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<(), String> {
    let actor = crate::native_world::source_target::status_parent(arena, context, slot)?
        .ok_or_else(|| format!("Spider death lost current actor {slot}"))?;
    let (position, yaw, mana, owner) =
        (actor.position, actor.yaw, actor.mana, actor.source_player_entity_index());
    if mana > 0 {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_entity_rng(super::level2_next_rng16(row.entity_rng));
        arena.economy.source_rng.insert(slot, row.entity_rng);
        let _ = crate::world::effects::spawn_death_mana_sphere(
            arena, terrain, slot, position, yaw, mana, owner,
        )?;
        // The source dereferences the same fixed address AFTER allocation.
        if !crate::world::effects::clear_replacement_mana_owner(arena, slot)? {
            arena.entities[usize::from(slot)].set_source_player_entity_index(0);
        }
    }
    let current = crate::native_world::source_target::read(arena, context, slot)?
        .ok_or_else(|| format!("Spider death lost current address {slot}"))?;
    let burst = if current.control_flags & 0x100000 == 0 {
        crate::world::effects::spawn_death_burst(
            arena, terrain, particles, slot, current.position, current.source_id,
        )?
    } else { None };
    if !crate::world::effects::hide_birth_replacement(arena, slot, burst)? {
        hide_current(arena, slot);
    }
    Ok(())
}

fn expire(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    summoned: bool,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<(), String> {
    let position = crate::native_world::source_target::read(arena, context, slot)?
        .ok_or_else(|| format!("Spider expiry lost current address {slot}"))?.position;
    if summoned {
        let visual = super::super::goat::runtime::spawn_expiry_visual(arena, terrain, position, particles)?;
        if visual == Some(slot) {
            let mut zero = Position::default();
            zero.z = terrain.ground_height(zero);
            arena.entities[usize::from(slot)].set_position(zero);
            terrain.set_source_predicted_position(zero);
            crate::world::effects::sync_position(arena, terrain, slot, zero, true, true)?;
        }
    }
    let position = crate::native_world::source_target::read(arena, context, slot)?
        .ok_or_else(|| format!("Spider expiry lost current address {slot}"))?.position;
    let child = crate::world::effects::allocate_from_source_position(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        slot,
        position,
        particles,
    )?;
    if !crate::world::effects::hide_birth_replacement(arena, slot, child)? {
        hide_current(arena, slot);
    }
    Ok(())
}
