//! Model21 bindings to MC2 source records, typed properties and particles.

use super::{Actor, Context, DESCRIPTOR, Effects, RULES, SOURCE_MODEL, Strike, Target};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

const SOURCE_VISUALS: [u16; 8] = [311, 308, 309, 310, 305, 306, 307, 312];

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn validate_visuals(particles: &[ParticleParameter]) -> Result<(), String> {
    for index in SOURCE_VISUALS {
        let particle = particles
            .get(usize::from(index))
            .ok_or_else(|| format!("model21 requires retained particle{index}"))?;
        if particle.animation_limit().is_none() {
            return Err(format!(
                "model21 particle{index} has unsupported render mode {}",
                particle.render_mode
            ));
        }
    }
    Ok(())
}

pub(super) fn launch_strike(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    strike: Strike,
) -> Result<bool, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(false);
    };
    let payload =
        crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
            origin: crate::native_world::FireballProjectileOrigin::Creature21,
            position: strike.initial_position,
            yaw: strike.yaw,
            pitch: strike.pitch,
            axis_position: Position::default(),
            source_id_entity_index: strike.source_id,
            holder_entity_index: 0,
            source_subspell_index: strike.damage as u16,
            source_mana: 50,
            source_dword_0x10: 0,
            target_source_entity_index: strike.target_slot,
            target_class: strike.target_class,
            target_model: strike.target_model,
        };
    if arena
        .economy
        .initialize_fireball_projectile(slot, payload, 0)
        .is_none()
    {
        return Err(format!(
            "model21 Fireball map linkage failed at allocated slot{slot}"
        ));
    }
    crate::world::effects::sync_position(
        arena,
        terrain,
        slot,
        strike.initial_position,
        true,
        false,
    )?;
    crate::world::effects::lighting::admit_source(
        arena,
        terrain,
        slot,
        strike.initial_position,
        crate::world::effects::lighting::SourceLight::Fireball,
    );
    arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|projectile| projectile.source_entity_index == slot)
        .ok_or_else(|| format!("model21 Fireball lost allocated slot{slot}"))?
        .position = strike.position;
    Ok(
        crate::native_world::Level2EconomyState::apply_creature_fireball_target_feedback(
            &mut arena.entities,
            strike.target_slot,
        ),
    )
}

pub(super) fn materialize_water_splash(
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
            "model21 water splash map linkage failed at allocated slot{slot}"
        ));
    };
    Ok(Some(splash.source_entity_index))
}

pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    phase: u8,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    validate_visuals(particles)?;
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(169);
    row.set_max_life(RULES.max_life);
    row.set_life(RULES.max_life);
    row.set_mana(RULES.initial_mana);
    row.set_actual_speed(RULES.maximum_speed);
    put(&mut row, 136, RULES.minimum_speed as u16);
    put(&mut row, 138, RULES.maximum_speed as u16);
    row.set_entity_rng(crate::native_world::level2_next_rng16(seed));
    let angle = (row.entity_rng & 2047).wrapping_sub(1) as i16;
    row.set_yaw(angle);
    row.set_pitch(angle);
    row.set_source_roll(angle);
    row.set_fov(0);
    row.set_subspell_index(RULES.constructor_subspell);
    row.record[60] = 1;
    row.record[61] = 64;
    row.record[69] = 3;
    row.record[70] = u8::MAX;
    row.record[71] = 0;
    row.record[72] = 0;
    row.record[74] = 0;
    row.set_phase(phase);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR) + 1).to_le_bytes());
    crate::world::effects::bind_particle(&mut row, SOURCE_VISUALS[0], particles)?;
    row.set_source_overlap_half_extents(RULES.collision_half_width, RULES.collision_half_width);
    put(&mut row, 92, RULES.collision_half_height as u16);
    Ok(row)
}

fn edit_route(
    row: &mut EntitySnapshot,
    model: u8,
    runtime: &mut dyn ArcherStageRuntime,
    bind: Option<u8>,
) {
    let mut action = row.action;
    let mut index = row.record[76] as i8;
    let mut mode = row.record[77] as i8;
    let mut relation = word(row, 78);
    let mut route = crate::native_world::creatures::stage_route::Route {
        model,
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

pub(crate) fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model21 StageVar bind lost slot{slot}"))?;
    edit_route(row, SOURCE_MODEL, runtime, Some(index));
    Ok(())
}

/// Ordinary source constructor sub_4C8F0, before the authored StageVar tail.
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
    validate_visuals(particles)?;
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model21 source constructor counters are absent".to_owned())?[21];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let phase = counter;
    counter = counter.wrapping_add(1);
    let mut row = construct_row(slot, arena.new_event_rng(slot), position, phase, particles)?;
    row.bind_frame_zero_max_life(max_life);
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 21] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_authored_route(arena, runtime, slot, index)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model21 authored constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature21,
        CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "model21 record{authored_record} lost its source origin"
        ));
    }
    Ok(Some(slot))
}

/// Boss/special StageVar producer entry point. It uses the ordinary model21
/// constructor, then installs source mode17 on that same allocated row.
pub(crate) fn allocate_summoned(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    parent_slot: u16,
    target_slot: u16,
    player_slot: u16,
) -> Result<Option<u16>, String> {
    validate_visuals(particles)?;
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent.active)
        .cloned()
        .ok_or_else(|| format!("model21 summon parent slot{parent_slot} is absent"))?;
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model21 source constructor counters are absent".to_owned())?[21];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let phase = counter;
    counter = counter.wrapping_add(1);
    let mut row = construct_row(slot, arena.new_event_rng(slot), position, phase, particles)?;
    row.source_id = parent.source_id;
    put(&mut row, 30, parent.source_id);
    put(&mut row, 152, player_slot);
    row.set_source_action(175);
    row.record[77] = 17;
    put(&mut row, 44, parent_slot);
    put(&mut row, 50, 250);
    put(&mut row, 154, target_slot);
    row.set_actual_speed(320);
    row.set_yaw(parent.yaw);
    row.set_source_roll(parent.yaw);
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 21] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("model21 summon constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature21,
        CreatureRootOrigin::CreatureConversion {
            producer_slot: parent_slot,
        },
    ) {
        return Err(format!(
            "model21 summon at slot{slot} lost its source origin"
        ));
    }
    Ok(Some(slot))
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    targets: crate::native_world::source_target::Context<'_>,
    family: &[u16],
) -> Result<(), String> {
    crate::native_world::creatures::stage_route::prepare_source_pass(
        arena, runtime, targets, family,
    )
}

fn targets(arena: &SourceArena) -> Vec<Target> {
    let mut rows = super::super::creature_15::mc2::targets_in_arena(arena);
    let typed = arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|row| {
        row.active
            && row.class != 0
            && row.index != 0
            && !typed.iter().any(|(slot, _)| *slot == row.index)
    }) {
        let target = Target {
            slot: row.index,
            source_id: row.source_id,
            class: row.class,
            model: row.model,
            action: row.action,
            life: row.life,
            control_flags: row.control_flags,
            position: row.position,
            previous: word(row, 54),
            next: word(row, 56),
            attacker: word(row, 42),
            killer: word(row, 40),
            target: word(row, 154),
            yaw: row.yaw as u16,
            roll: row.source_roll() as u16,
            speed: row.actual_speed,
            maximum_speed: word(row, 138) as i16,
            half_width: word(row, 88) as i16,
            parent: word(row, 44),
            stage: super::super::archer::ArcherStageState {
                index: row.record[76],
                mode: row.record[77],
                relation: word(row, 78),
                parent: word(row, 44),
                remaining: word(row, 50) as i16,
                duration: word(row, 52) as i16,
            },
        };
        if let Some(existing) = rows.iter_mut().find(|existing| existing.slot == row.index) {
            *existing = target;
        } else {
            rows.push(target);
        }
    }
    rows.sort_unstable_by_key(|row| row.slot);
    rows
}

pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
) -> Result<Effects, String> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model21 callback lost slot{slot}"))?
        .clone();
    let mut actor_row = row;
    let targets = targets(arena);
    let mut context = Context {
        terrain,
        targets: &targets,
        class3,
        model21: &families[usize::from(SOURCE_MODEL)],
        families,
        runtime,
    };
    let mut effects = super::step(
        &mut Actor {
            row: &mut actor_row,
        },
        &mut context,
    )?;
    let position = actor_row.position;
    if let Some(index) = actor_row.particle_parameter_index {
        if arena.entities[usize::from(slot)].particle_parameter_index != Some(index) {
            crate::world::effects::bind_particle(&mut actor_row, index, particles)?;
            actor_row.set_source_overlap_half_extents(
                RULES.collision_half_width,
                RULES.collision_half_width,
            );
            put(&mut actor_row, 92, RULES.collision_half_height as u16);
        }
    }
    arena.economy.source_rng.insert(slot, actor_row.entity_rng);
    arena.entities[usize::from(slot)] = actor_row;
    // Apply the captured linked rows before any callback allocation can reuse
    // a source address; the snapshots and these writes share one visit.
    for (linked_slot, flags) in effects.linked_blink_flags.drain(..) {
        if let Some(row) = arena
            .entities
            .get_mut(usize::from(linked_slot))
            .filter(|row| row.active)
        {
            row.set_control_flags((row.control_flags & !0x0600_0000) | flags);
        }
    }
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    if effects.prekill {
        effects.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 168)?;
    }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(position) = effects.water_splash.take() {
        let _ = materialize_water_splash(arena, terrain, position)?;
    }
    if let Some(strike) = effects.strike.take()
        && launch_strike(arena, terrain, strike)?
    {
        effects.target_music = Some(strike.target_slot);
    }
    if effects.kill {
        super::super::creature_24::mc2::kill(arena, terrain, particles, slot)?;
    }
    if effects.expire {
        super::super::creature_24::mc2::expire(
            arena,
            terrain,
            particles,
            slot,
            effects.summon_expire,
        )?;
        // sub_26470 runs the jump/visual tail after sub_1E580 has
        // allocated expiry children and hidden the current slot. Allocation
        // can replace that address, so use its current occupant.
        let mut current = arena.entities[usize::from(slot)].clone();
        let mut tail = Effects::default();
        if matches!(current.record[77], 13 | 16) {
            current.record[71] = 0;
            let targets = self::targets(arena);
            let context = Context {
                terrain,
                targets: &targets,
                class3,
                model21: &families[usize::from(SOURCE_MODEL)],
                families,
                runtime,
            };
            let previous_visual = current.particle_parameter_index;
            super::jump(&mut Actor { row: &mut current }, &context, &mut tail);
            if current.action != 175 {
                let action = current.action;
                Actor { row: &mut current }.set_action(action);
            }
            if let Some(index) = current.particle_parameter_index
                && previous_visual != Some(index)
            {
                crate::world::effects::bind_particle(&mut current, index, particles)?;
                current.set_source_overlap_half_extents(
                    RULES.collision_half_width,
                    RULES.collision_half_width,
                );
                put(&mut current, 92, RULES.collision_half_height as u16);
            }
            let position = current.position;
            arena.economy.source_rng.insert(slot, current.entity_rng);
            arena.entities[usize::from(slot)] = current;
            crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
            if let Some(position) = tail.water_splash {
                let _ = materialize_water_splash(arena, terrain, position)?;
            }
            if tail.sound.is_some() {
                effects.sound = tail.sound;
            }
        }
    }
    Ok(effects)
}

/// Compact post-allocation feedback for the shared sound/music tail.
pub(crate) fn target_music(effects: &Effects) -> Option<u16> {
    effects.target_music
}
