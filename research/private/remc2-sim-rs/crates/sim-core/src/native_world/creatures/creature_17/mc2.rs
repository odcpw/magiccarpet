//! Model-17 bindings to the source arena, StageVar table and assets.

use super::{
    Creature17Context, Creature17Effects, Creature17State, SOURCE_CLASS, SOURCE_MODEL,
    Target,
};
use crate::Position;
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::{
    EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain, guided_projectile,
};

pub(super) const DESCRIPTOR: u16 = 85;
pub(super) const STRIKE_DESCRIPTOR: u16 = 87;
pub(super) const PARTICLE: u16 = 285;
const PROJECTILE_PARTICLE: u16 = 196;
const PROJECTILE_MODEL: u8 = 20;
const PROJECTILE_ACTION: u8 = 21;
const PROJECTILE_SPEED: i16 = 394;
const PROJECTILE_RANGE: i32 = 7680;
const PROJECTILE_DESCRIPTOR: u16 = 65;
pub(super) const ATTACK_SOUND: u16 = 58;

/// sub_24930 prepares its source sound before consuming the hit mailbox.
pub(crate) fn prepare_callback_sound(arena: &mut SourceArena, slot: u16) -> Option<u16> {
    arena.entities.get(usize::from(slot)).filter(|row| {
        row.active && (row.class, row.model, row.action) == (SOURCE_CLASS, SOURCE_MODEL, 138)
    })?;
    arena.economy.prepare_sound(slot, slot, -1, ATTACK_SOUND);
    Some(ATTACK_SOUND)
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn load(row: &EntitySnapshot, generation: u32) -> Creature17State {
    Creature17State {
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
        roll: row.source_roll(),
        fov: row.fov,
        speed: row.actual_speed,
        minimum_speed: word(row, 136) as i16,
        maximum_speed: word(row, 138) as i16,
        max_life: row.max_life,
        life: row.life,
        mana: row.mana,
        max_mana: row.max_mana,
        constructor_subspell: row.subspell_index,
        pending_damage: i32::from_le_bytes(
            row.record[98..102]
                .try_into()
                .expect("source damage mailbox"),
        ),
        pending_attacker: word(row, 102),
        attacker: word(row, 42),
        killer: word(row, 40),
        target: word(row, 154),
        linked_peer: word(row, 54),
        linked_child: word(row, 56),
        previous: word(row, 58),
        next: row.next_relation.unwrap_or(0),
        parent: word(row, 44),
        phase: row.phase,
        proximity: row.record[61],
        attack_phase: row.record[74],
        descriptor: row.parameter_relation.unwrap_or(DESCRIPTOR),
        timer: row.source_dword_10(),
        visual: word(row, 94),
        animation_frame: row.record[96],
        animation_limit: row.record[97],
        half_width: row.array_pitch(),
        half_height: row.array_fov(),
        vertical_extent: row.array_yaw(),
        route_index: row.record[76],
        route_mode: row.record[77],
        route_relation: word(row, 78),
        route_remaining: word(row, 50) as i16,
        route_duration: word(row, 52) as i16,
    }
}

pub(crate) fn store(
    row: &mut EntitySnapshot,
    actor: Creature17State,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if row.index != actor.slot {
        return Err(format!(
            "model-17 source carrier changed from {}",
            actor.slot
        ));
    }
    if row.particle_parameter_index != Some(actor.visual) || actor.animation_limit == 0 {
        let particle = particles
            .get(usize::from(actor.visual))
            .ok_or_else(|| format!("model-17 requires retained particle{}", actor.visual))?;
        row.record[97] = particle.animation_limit().ok_or_else(|| {
            format!(
                "model-17 particle{} has invalid animation mode",
                actor.visual
            )
        })?;
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
    row.set_max_life(actor.max_life);
    row.set_life(actor.life);
    row.set_mana(actor.mana);
    row.max_mana = actor.max_mana;
    row.mana_cost = actor.max_mana;
    row.record[144..148].copy_from_slice(&actor.max_mana.to_le_bytes());
    row.set_subspell_index(actor.constructor_subspell);
    row.record[98..102].copy_from_slice(&actor.pending_damage.to_le_bytes());
    row.set_source_dword_10(actor.timer);
    row.record[76] = actor.route_index;
    row.record[77] = actor.route_mode;
    row.record[96] = actor.animation_frame;
    row.record[97] = actor.animation_limit;
    row.record[74] = actor.attack_phase;
    row.set_phase(actor.phase);
    row.record[61] = actor.proximity;
    row.particle_parameter_index = Some(actor.visual);
    row.parameter_relation = Some(actor.descriptor);
    row.record[164..168].copy_from_slice(&(u32::from(actor.descriptor) + 1).to_le_bytes());
    put(row, 94, actor.visual);
    put(row, 136, actor.minimum_speed as u16);
    put(row, 138, actor.maximum_speed as u16);
    put(row, 40, actor.killer);
    put(row, 42, actor.attacker);
    put(row, 44, actor.parent);
    put(row, 50, actor.route_remaining as u16);
    put(row, 52, actor.route_duration as u16);
    put(row, 54, actor.linked_peer);
    put(row, 56, actor.linked_child);
    put(row, 58, actor.previous);
    put(row, 78, actor.route_relation);
    put(row, 86, actor.vertical_extent as u16);
    put(row, 88, actor.half_width as u16);
    put(row, 90, actor.half_width as u16);
    put(row, 92, actor.half_height as u16);
    put(row, 102, actor.pending_attacker);
    put(row, 152, actor.player);
    put(row, 154, actor.target);
    Ok(())
}

pub(crate) fn construct_row(
    slot: u16,
    generation: u32,
    seed: u16,
    position: Position,
    counter: &mut u8,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let mut actor = super::construct(slot, generation, seed, position, counter);
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = SOURCE_CLASS;
    row.model = SOURCE_MODEL;
    row.record[67] = SOURCE_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.record[69] = 3;
    row.record[60] = 1;
    row.record[61] = 64;
    actor.rng = crate::native_world::level2_next_rng16(seed);
    crate::world::effects::bind_particle(&mut row, PARTICLE, particles)?;
    actor.animation_limit = row.record[97];
    store(&mut row, actor, particles)?;
    Ok(row)
}

pub(crate) fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    crate::native_world::creatures::stage_route::bind_source(arena, runtime, slot, index)
}

/// Ordinary `sub_4C460` authored source producer. Allocation failure leaves
/// the constructor counter and runtime route binding untouched.
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
    let particle = particles
        .get(usize::from(PARTICLE))
        .ok_or_else(|| "model-17 requires retained particle285".to_owned())?;
    if particle.animation_limit().is_none() {
        return Err("model-17 particle285 has invalid animation mode".to_owned());
    }
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model-17 source constructor counter table is absent".to_owned())?
        [usize::from(SOURCE_MODEL)];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = construct_row(
        slot,
        arena.slot_generation(slot),
        arena.new_event_rng(slot),
        position,
        &mut counter,
        particles,
    )?;
    row.bind_frame_zero_max_life(max_life);
    runtime.bytes[crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL)] = counter;
    let seed = row.entity_rng;
    arena.economy.source_rng.insert(slot, seed);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_authored_route(arena, runtime, slot, index)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model-17 constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature17,
        CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "model-17 record{authored_record} lost generation-bound origin"
        ));
    }
    Ok(Some(slot))
}

fn targets(arena: &SourceArena, pointers: &[u16]) -> Vec<Target> {
    let pointers: std::collections::BTreeSet<_> = pointers.iter().copied().collect();
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
            half_width: row.half_width,
            speed: 0,
            maximum_speed: 0,
            previous: row.previous,
            next: row.next,
            linked_peer: 0,
            linked_child: 0,
            parent: row.parent,
            route_index: row.stage.index,
            route_mode: row.stage.mode,
            route_relation: row.stage.relation,
            target: row.target,
            attacker: row.attacker,
            killer: row.killer,
        })
        .collect::<Vec<_>>();
    let typed = arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|row| {
        (row.index == 0 || (row.active && row.class != 0) || pointers.contains(&row.index))
            && !typed.iter().any(|(slot, _)| *slot == row.index)
    }) {
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
            roll: row.source_roll(),
            half_width: row.array_pitch(),
            speed: row.actual_speed,
            maximum_speed: word(row, 138) as i16,
            previous: word(row, 58),
            next: row.next_relation.unwrap_or(0),
            linked_peer: word(row, 54),
            linked_child: word(row, 56),
            parent: word(row, 44),
            route_index: row.record[76],
            route_mode: row.record[77],
            route_relation: word(row, 78),
            target: word(row, 154),
            attacker: word(row, 42),
            killer: word(row, 40),
        };
        if let Some(old) = targets.iter_mut().find(|target| target.slot == row.index) {
            *old = view;
        } else {
            targets.push(view);
        }
    }
    targets.sort_unstable_by_key(|target| target.slot);
    targets
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    targets: crate::native_world::source_target::Context<'_>,
    frozen_family: &[u16],
) -> Result<(), String> {
    crate::native_world::creatures::stage_route::prepare_source_pass(
        arena,
        runtime,
        targets,
        frozen_family,
    )
}

/// Ordinary `sub_4DC40` class-9/model-20 construction followed by the exact
/// `sub_1D0E0` owner, target, descriptor, and damage overrides.
fn launch_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    actor_slot: u16,
    target_slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<Option<u16>, String> {
    let particle = particles
        .get(usize::from(PROJECTILE_PARTICLE))
        .ok_or("model-17 projectile requires retained particle196")?;
    if particle.animation_limit().is_none() {
        return Err(format!(
            "model-17 particle196 has unsupported render mode {}",
            particle.render_mode
        ));
    }
    let source_position = arena
        .entities
        .get(usize::from(actor_slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .ok_or("model-17 projectile source is absent")?
        .position;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    // NewEvent receives the source position by address. If it aliases the
    // source row, that address has already been cleared before insertion.
    let position = if slot == actor_slot {
        Position::default()
    } else {
        source_position
    };
    let seed = arena.new_event_rng(slot);
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = 9;
    row.model = PROJECTILE_MODEL;
    row.record[67] = 9;
    row.record[68] = PROJECTILE_MODEL;
    row.set_source_action(PROJECTILE_ACTION);
    row.set_actual_speed(PROJECTILE_SPEED);
    row.record[136..138].copy_from_slice(&(PROJECTILE_SPEED as u16).to_le_bytes());
    let max_life = PROJECTILE_RANGE / i32::from(PROJECTILE_SPEED);
    row.set_max_life(max_life);
    row.set_life(max_life);
    row.set_control_flags(row.control_flags & !8);
    crate::world::effects::bind_particle(&mut row, PROJECTILE_PARTICLE, particles)?;
    row.parameter_relation = Some(PROJECTILE_DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(PROJECTILE_DESCRIPTOR) + 1).to_le_bytes());
    arena.economy.source_rng.insert(slot, seed);
    arena.entities[usize::from(slot)] = row;

    // The source caller reads both fixed addresses after NewEvent, so a
    // reclaim that aliases either address changes the copied owner/selector.
    let source = arena
        .entities
        .get(usize::from(actor_slot))
        .ok_or("model-17 projectile source address is absent")?
        .clone();
    let target = crate::native_world::source_target::read(arena, context, target_slot)?
        .ok_or("model-17 projectile target address is absent")?;
    let origin = arena.entities[usize::from(slot)].position;
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(source.position, target.position);
    let mut position = origin;
    position.z = position.z.wrapping_add(source.array_fov());
    let projectile = &mut arena.entities[usize::from(slot)];
    projectile.source_id = source.source_id;
    put(projectile, 30, source.source_id);
    projectile.set_yaw(yaw);
    projectile.set_pitch(pitch);
    projectile.set_position(position);
    projectile.record[69] = target.class;
    projectile.record[70] = target.model;
    put(projectile, 154, word(&source, 154));
    projectile.record[71] = 10;
    projectile.record[72] = 65;
    projectile.parameter_relation = Some(PROJECTILE_DESCRIPTOR);
    projectile.record[164..168]
        .copy_from_slice(&(u32::from(PROJECTILE_DESCRIPTOR) + 1).to_le_bytes());
    projectile.set_subspell_index(super::RULES.projectile_damage);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(target) = arena.entities.get_mut(usize::from(target_slot)) {
        target.apply_source_sub_5ef70();
    }
    Ok(Some(slot))
}

/// Shared class-9/model-20/action-21 flight and class-10/model-65 impact.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<guided_projectile::StepOutcome, String> {
    guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        guided_projectile::Guidance {
            yaw_turn: 0,
            pitch_turn: 22,
        },
        None,
        context,
    )
}

/// Apply one source callback and return its attack producers to the shared
/// dispatcher. Family state and all local row mutations stay in this adapter.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    targets_context: crate::native_world::source_target::Context<'_>,
) -> Result<Creature17Effects, String> {
    let actor_row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model-17 callback lost slot{slot}"))?;
    let mut actor = load(actor_row, arena.slot_generation(slot));
    let mut pointers: Vec<_> =
        crate::native_world::source_target::alliance_direct_references(arena, actor_row)?
            .into_iter()
            .collect();
    pointers.extend(crate::native_world::source_target::summoned_direct_references(arena, actor_row)?);
    if actor.action == 139 && actor.linked_peer != 0
        && let Some(parent) = arena.entities.get(usize::from(actor.linked_peer))
        && parent.class == SOURCE_CLASS && parent.model == SOURCE_MODEL
        && parent.action == 139 && parent.life >= 0 && !parent.drawing_disabled()
    {
        // sub_1C560 then dereferences the parent's current word32, including a
        // retained inactive row; preserve that address in the source view.
        pointers.push(parent.word_32());
    }
    pointers.extend_from_slice(class3);
    for family in families { pointers.extend_from_slice(family); }
    let targets = targets(arena, &pointers);
    let callback_action = actor.action;
    let context = Creature17Context {
        terrain,
        runtime,
        targets: &targets,
        class3,
        model17: &families[usize::from(SOURCE_MODEL)],
        models: Some(families),
    };
    let mut effects = super::step(&mut actor, &context)
        .ok_or_else(|| format!("unsupported model-17 action{}", actor.action))?;
    store(&mut arena.entities[usize::from(slot)], actor, particles)?;
    arena.economy.source_rng.insert(slot, actor.rng);
    for (linked_slot, flags) in &effects.linked_blink_flags {
        if let Some(row) = arena.entities.get_mut(usize::from(*linked_slot)) {
            row.set_control_flags((row.control_flags & !0x0600_0000) | *flags);
        }
    }
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    if callback_action == 140 {
        effects.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 136)?;
        // The shared source-chain walk performs the same child action writes
        // and last-nonzero killer inheritance returned by the core intent.
        effects.linked_actions.clear();
    } else {
        for (linked_slot, action) in &effects.linked_actions {
            if let Some(row) = arena
                .entities
                .get_mut(usize::from(*linked_slot))
                .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
            {
                row.set_source_action(*action);
            }
        }
    }
    for update in &effects.linked_updates {
        if let Some(row) = arena
            .entities
            .get_mut(usize::from(update.slot))
            .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        {
            row.set_source_action(update.action);
            put(row, 54, update.linked_peer);
            put(row, 154, update.target);
        }
    }
    if let Some(intent) = effects.projectile.take() {
        effects.projectile_slot =
            launch_projectile(arena, terrain, particles, slot, intent.target, targets_context)?;
    }
    // NewEvent can replace the callback address itself. In that case the
    // projectile constructor already published the new occupant's position;
    // the old actor's local packet must not overwrite it afterwards.
    if effects.projectile_slot != Some(slot) {
        crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    }
    if let Some(summoned) = effects.stage_expiry.take() {
        super::super::creature_24::mc2::expire(arena, terrain, particles, slot, summoned)?;
    }
    if effects.kill {
        kill(arena, terrain, particles, slot)?;
    }
    if effects.route_hide {
        arena.entities[usize::from(slot)].disable_drawing();
        crate::world::effects::sync_position(arena, terrain, slot, actor.position, false, false)?;
    }
    Ok(effects)
}

fn kill(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    let original = arena.handle(slot).ok_or("model-17 death lost its original handle")?;
    let source = arena.entities[usize::from(slot)].clone();
    let (position, yaw, mana, owner) =
        (source.position, source.yaw, source.mana, source.source_player_entity_index());
    if mana > 0 {
        if let Some(row) = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active)
        {
            let next = crate::native_world::level2_next_rng16(row.entity_rng);
            row.set_entity_rng(next);
            arena.economy.source_rng.insert(slot, next);
        }
        crate::world::effects::spawn_death_mana_sphere(
            arena, terrain, slot, position, yaw, mana, owner,
        )?;
        if !crate::world::effects::clear_replacement_mana_owner(arena, slot)? {
            arena.entities[usize::from(slot)].record[152..154].fill(0);
        }
        arena.economy.source_owner.insert(slot, 0);
    }
    let current = crate::native_world::source_target::read(arena, Default::default(), slot)?
        .ok_or("model-17 death lost its current source address")?;
    let burst = if current.control_flags & 0x100000 == 0 {
        crate::world::effects::spawn_death_burst(
            arena, terrain, particles, slot, current.position, current.source_id,
        )?
    } else { None };
    if !crate::world::effects::hide_birth_replacement(arena, slot, burst)? {
        if arena.resolve(original).is_none() {
            return Err(format!("model-17 death cannot hide replacement at {slot}"));
        }
        let row = &mut arena.entities[usize::from(slot)];
        row.disable_drawing();
        let position = row.position;
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
    }
    Ok(())
}
