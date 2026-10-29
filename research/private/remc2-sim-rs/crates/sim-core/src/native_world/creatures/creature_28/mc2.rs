//! Source-row constructor and callback adapter for class5/model28.

use super::{
    Actor, Effects, Frame, SOURCE_ATTACK_VISUAL, SOURCE_CLASS, SOURCE_DESCRIPTOR,
    SOURCE_IDLE_ACTION, SOURCE_IDLE_VISUAL, SOURCE_MODEL, SOURCE_PROPERTIES,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

const SOURCE_ROW_MIN_LEN: usize = 176;

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
            route_duration: word(row, 52) as i16,
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

fn targets(arena: &SourceArena) -> Vec<super::Target> {
    let mut targets = super::super::creature_15::mc2::targets_in_arena(arena)
        .into_iter()
        .filter_map(|target| {
            let row = arena
                .entities
                .get(usize::from(target.slot))
                .filter(|row| row.active && row.record.len() >= SOURCE_ROW_MIN_LEN)?;
            Some(super::Target {
                slot: target.slot,
                source_id: target.source_id,
                class: target.class,
                model: target.model,
                action: target.action,
                phase: row.record[74],
                flags: target.control_flags,
                life: target.life,
                position: target.position,
                yaw: target.yaw as i16,
                half_width: word(row, 88) as i16,
                next: target.next,
                attacker: target.attacker,
                killer: target.killer,
                route_index: row.record[76],
                route_relation: word(row, 78),
                route_target: word(row, 154),
            })
        })
        .collect::<Vec<_>>();
    targets.sort_unstable_by_key(|target| target.slot);
    targets
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
    slot: u16,
    same_model: &[u16],
) -> Result<Effects, String> {
    let targets = targets(arena);
    let frame = Frame {
        terrain,
        targets: &targets,
        same_model,
    };
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .cloned()
        .ok_or_else(|| format!("model28 callback lost slot{slot}"))?;
    let mut actor = load(&row).ok_or_else(|| format!("model28 row{slot} is malformed"))?;
    let source_counter = actor.source_counter;
    let mut callback = super::step(&mut actor, &frame, runtime, particles)?;
    let position = actor.position;
    let rng = actor.rng;
    store(&mut row, actor, particles)?;
    arena.economy.source_rng.insert(slot, rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if callback.water_splash {
        let _ = materialize_water_splash(arena, terrain, position)?;
    }
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
        let position = arena.entities[usize::from(slot)].position;
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
    }
    Ok(callback.notices)
}
