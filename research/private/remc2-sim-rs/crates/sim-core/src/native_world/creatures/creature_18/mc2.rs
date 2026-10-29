//! Model-18 bindings to the shared source arena, StageVar table and assets.

use super::{
    Creature18Context, Creature18Effects, Creature18State, DESCRIPTOR, SOURCE_CLASS, SOURCE_MODEL,
    Target,
};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

const PARTICLE: u16 = 286;

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn load(row: &EntitySnapshot, generation: u32) -> Creature18State {
    Creature18State {
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
    actor: Creature18State,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if row.index != actor.slot {
        return Err(format!(
            "model-18 source carrier changed from {}",
            actor.slot
        ));
    }
    if row.particle_parameter_index != Some(actor.visual) || actor.animation_limit == 0 {
        let particle = particles
            .get(usize::from(actor.visual))
            .ok_or_else(|| format!("model-18 requires retained particle{}", actor.visual))?;
        row.record[97] = particle.animation_limit().ok_or_else(|| {
            format!(
                "model-18 particle{} has invalid animation mode",
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
    row.record[record::ATTACK_PHASE] = actor.attack_phase;
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

mod record {
    pub(super) const ATTACK_PHASE: usize = 74;
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

/// Ordinary `sub_4C590` authored source producer. A rejected allocator leaves
/// the model counter, source RNG and route table untouched.
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
        .ok_or_else(|| "model-18 requires retained particle286".to_owned())?;
    if particle.animation_limit().is_none() {
        return Err("model-18 particle286 has invalid animation mode".to_owned());
    }
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model-18 source constructor counter table is absent".to_owned())?
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
        .ok_or_else(|| format!("model-18 constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature18,
        CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "model-18 record{authored_record} lost generation-bound origin"
        ));
    }
    Ok(Some(slot))
}

fn targets(arena: &SourceArena) -> Vec<Target> {
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
            linked_peer: 0,
            linked_child: 0,
            parent: row.parent,
            route_index: row.stage.index,
            route_mode: row.stage.mode,
            route_relation: row.stage.relation,
        })
        .collect::<Vec<_>>();
    let typed = arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|row| {
        row.active
            && row.class != 0
            && row.index != 0
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
            previous: word(row, 58),
            next: row.next_relation.unwrap_or(0),
            target: word(row, 154),
            attacker: word(row, 42),
            killer: word(row, 40),
            linked_peer: word(row, 54),
            linked_child: word(row, 56),
            parent: word(row, 44),
            route_index: row.record[76],
            route_mode: row.record[77],
            route_relation: word(row, 78),
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
    frozen_family: &[u16],
) {
    crate::native_world::creatures::stage_route::prepare_source_pass(arena, runtime, frozen_family);
}

/// Materialize one `sub_1D460` fan member in the shared typed Fireball arena.
/// The source caller replaces constructor descriptor64 with descriptor61;
/// `Creature18` is the typed carrier identity for that descriptor behavior.
fn launch_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    launch: super::ProjectileIntent,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let payload =
        crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
            origin: crate::native_world::FireballProjectileOrigin::Creature18,
            position: launch.position,
            yaw: launch.yaw,
            pitch: launch.pitch,
            axis_position: Position::default(),
            source_id_entity_index: launch.source_id,
            holder_entity_index: 0,
            source_subspell_index: launch.subspell,
            source_mana: 50,
            source_dword_0x10: 0,
            target_source_entity_index: launch.source_target,
            target_class: launch.target_class,
            target_model: launch.target_model,
        };
    arena
        .economy
        .initialize_fireball_projectile(slot, payload, 0)
        .map(Some)
        .ok_or_else(|| format!("model-18 Fireball map linkage failed at slot{slot}"))
}

/// Apply one source callback. The shared root keeps dispatch, same-pass child
/// tail work and next-pass reclamation outside this family adapter.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
) -> Result<Creature18Effects, String> {
    let targets = targets(arena);
    let actor_row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model-18 callback lost slot{slot}"))?;
    let mut actor = load(actor_row, arena.slot_generation(slot));
    let callback_action = actor.action;
    let context = Creature18Context {
        terrain,
        runtime,
        targets: &targets,
        class3,
        model18: &families[usize::from(SOURCE_MODEL)],
    };
    let mut effects = super::step(&mut actor, &context)
        .ok_or_else(|| format!("unsupported model-18 action{}", actor.action))?;
    let row = &mut arena.entities[usize::from(slot)];
    store(row, actor, particles)?;
    arena.economy.source_rng.insert(slot, actor.rng);
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    if callback_action == 148 {
        effects.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 144)?;
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
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    for launch in effects.projectiles.drain(..) {
        if let Some(projectile_slot) = launch_projectile(arena, terrain, launch)? {
            effects.projectile_slots.push(projectile_slot);
        }
    }
    if effects.kill {
        kill(arena, terrain, particles, slot)?;
    }
    if effects.expire {
        expire(arena, terrain, particles, slot, effects.summon_expire)?;
    }
    if effects.route_hide {
        arena.entities[usize::from(slot)].disable_drawing();
    }
    Ok(effects)
}

fn expire(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    summoned: bool,
) -> Result<(), String> {
    let position = arena.entities[usize::from(slot)].position;
    let generation = arena.slot_generation(slot);
    if summoned {
        super::super::goat::runtime::spawn_expiry_visual(arena, terrain, position, particles)?;
    }
    if arena.slot_generation(slot) != generation {
        let mut zero = Position::default();
        zero.z = terrain.ground_height(zero);
        arena.entities[usize::from(slot)].set_position(zero);
        crate::world::effects::sync_position(arena, terrain, slot, zero, true, true)?;
    }
    let current_position = arena.entities[usize::from(slot)].position;
    let child = crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        current_position,
        particles,
    )?;
    if child == Some(slot) {
        arena.entities[usize::from(slot)].set_position(Position::default());
        crate::world::effects::sync_position(
            arena,
            terrain,
            slot,
            Position::default(),
            true,
            true,
        )?;
    }
    let row = &mut arena.entities[usize::from(slot)];
    row.disable_drawing();
    let mut position = row.position;
    position.z = terrain.ground_height(position);
    row.set_position(position);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(())
}

fn kill(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    let (position, yaw, mana, owner) = if let Some(sphere) = arena
        .economy
        .spheres
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == slot)
    {
        (
            sphere.position,
            sphere.yaw,
            sphere.mana,
            sphere.player_entity_index,
        )
    } else {
        let row = &arena.entities[usize::from(slot)];
        (
            row.position,
            row.yaw,
            row.mana,
            row.source_player_entity_index(),
        )
    };
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
        crate::native_world::creatures::spider::mc2::spawn_mana(
            arena, terrain, slot, position, yaw, mana, owner,
        )?;
        if let Some(sphere) = arena
            .economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == slot)
        {
            sphere.player_entity_index = 0;
            sphere.owner_entity_index = 0;
        } else if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
            row.record[152..154].fill(0);
        }
    }
    let (current_position, current_flags) = if let Some(sphere) = arena
        .economy
        .spheres
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == slot)
    {
        (sphere.position, sphere.control_flags)
    } else {
        let row = &arena.entities[usize::from(slot)];
        (row.position, row.control_flags)
    };
    if current_flags & 0x100000 == 0 {
        let child = crate::world::effects::allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::RadialExplosion,
            ),
            current_position,
            particles,
        )?;
        if let Some(child) = child {
            // The source reads the parent address after allocation: the
            // allocator is allowed to replace that same address.
            let id = arena
                .economy
                .spheres
                .iter()
                .find(|sphere| sphere.active && sphere.source_entity_index == slot)
                .map_or_else(|| arena.entities[usize::from(slot)].source_id, |_| slot);
            let row = &mut arena.entities[usize::from(child)];
            row.source_id = id;
            put(row, 30, id);
            if child == slot {
                let zero = Position::default();
                row.set_position(zero);
                crate::world::effects::sync_position(arena, terrain, slot, zero, true, true)?;
            }
        }
    }
    if let Some(sphere) = arena
        .economy
        .spheres
        .iter_mut()
        .find(|sphere| sphere.source_entity_index == slot)
    {
        sphere.control_flags |= crate::native_world::ENTITY_DRAW_DISABLED_BIT;
        sphere.pending_delete = true;
    } else if let Some(row) = arena.entities.get_mut(usize::from(slot)) {
        row.disable_drawing();
    }
    Ok(())
}
