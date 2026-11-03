//! Model23 source-record constructor and same-row callback adapter.

use super::{
    Actor, Castle, Context, DESCRIPTOR, Effects, RULES, SOURCE_MODEL, Target, level2_next_rng16,
};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

const SOURCE_PARTICLE: u16 = 289;

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
    request: super::LightningRequest,
) -> Result<bool, String> {
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
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 9 && row.model == 9)
        .ok_or_else(|| format!("model23 Lightning constructor lost ray slot{slot}"))?;
    row.source_id = request.source_id;
    put(row, 30, request.source_id);
    row.set_yaw(request.yaw);
    row.set_pitch(request.pitch);
    row.record[65] = request.target_class;
    row.record[66] = request.target_model;
    row.record[71] = 10;
    row.record[72] = 23;
    put(row, 154, request.target_slot);
    row.set_subspell_index(request.damage as u16);
    crate::world::effects::sync_position(arena, terrain, slot, request.position, true, false)?;

    let Some(target) = arena
        .entities
        .get_mut(usize::from(request.target_slot))
        .filter(|target| {
            target.active
                && target.life >= 0
                && target.control_flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
        })
    else {
        return Ok(false);
    };
    let player_target = target.class == 3 && target.model == 0;
    target.apply_source_sub_5ef70();
    Ok(player_target)
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

fn target_views(entities: &[EntitySnapshot], arena: &SourceArena) -> Vec<Target> {
    entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.index != 0)
        .map(|row| {
            let castle_slot = word(row, 42);
            let raw_castle = entities.get(usize::from(castle_slot)).filter(|castle| {
                castle_slot != 0 && castle.active && castle.class == 3 && castle.model == 2
            });
            let typed_castle = arena
                .economy
                .castle()
                .filter(|castle| castle.active && castle.source_entity_index == castle_slot);
            let castle = raw_castle.map(|castle| Castle {
                position: typed_castle.map_or(castle.position, |state| state.position),
                half_x: word(castle, 88) as i16,
                half_y: word(castle, 90) as i16,
            });
            Target {
                slot: row.index,
                source_id: row.source_id,
                class: row.class,
                model: row.model,
                life: row.life,
                mana: row.mana,
                control_flags: row.control_flags,
                position: row.position,
                half_x: word(row, 88) as i16,
                half_y: word(row, 90) as i16,
                castle,
            }
        })
        .collect()
}

/// One source callback, committing its own row after sphere-link updates.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    mana_slots: &[u16],
) -> Result<Effects, String> {
    let Some(mut actor_row) = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .cloned()
    else {
        return Err(format!("model23 callback lost active slot{slot}"));
    };
    let source_prekill = actor_row.action == 188;
    let targets = target_views(&arena.entities, arena);
    let mut mana_candidates = Vec::with_capacity(mana_slots.len());
    for &candidate_slot in mana_slots {
        if let Some(current) = crate::native_world::source_target::read(
            arena, crate::native_world::source_target::Context::default(), candidate_slot,
        )? && current.model == 39 {
            mana_candidates.push(super::ManaCandidate {
                slot: candidate_slot,
                position: current.position,
            });
        }
    }
    let source_generation = arena.slot_generation(slot);
    let mut effects = {
        let (entities, economy) = (&mut arena.entities, &mut arena.economy);
        let mut context = Context {
            terrain,
            targets: &targets,
            mana_candidates: &mana_candidates,
            class3,
            families,
            entities,
            spheres: economy.spheres_mut(),
            runtime,
        };
        super::step(
            &mut Actor {
                row: &mut actor_row,
            },
            &mut context,
        )?
    };
    if arena.slot_generation(slot) != source_generation
        || !arena.entities[usize::from(slot)].active
        || arena.entities[usize::from(slot)].model != SOURCE_MODEL
    {
        return Err(format!(
            "model23 source slot{slot} was reclaimed during callback"
        ));
    }
    arena.entities[usize::from(slot)] = actor_row;
    if source_prekill {
        effects.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 184)?;
    }
    arena
        .economy
        .source_rng
        .insert(slot, arena.entities[usize::from(slot)].entity_rng);
    let position = arena.entities[usize::from(slot)].position;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(request) = effects.lightning.take()
        && launch_lightning(arena, terrain, particles, request)?
    {
        effects.target_music = Some(request.target_slot);
    }
    if effects.kill {
        super::super::creature_24::mc2::kill(arena, terrain, particles, slot)?;
    }
    Ok(effects)
}
