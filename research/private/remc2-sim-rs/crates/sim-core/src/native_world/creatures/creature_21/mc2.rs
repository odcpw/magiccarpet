//! Model21 bindings to MC2 source records, typed properties and particles.

use super::{Actor, Context, Effects, RULES, SOURCE_MODEL, Target};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

pub(super) const DESCRIPTOR: u16 = 96;
pub(super) const SOURCE_VISUALS: [u16; 8] = [311, 308, 309, 310, 305, 306, 307, 312];
pub(super) const JUMP_SOUND: u8 = 42;

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct Model21CallbackReceipt {
    pub actor: crate::campaign::source_arena::SourceEntityHandle,
    pub before: EntitySnapshot,
    pub entry_parent: Option<Target>,
    pub entry_target: Option<Target>,
    pub entry_writer: Option<crate::native_world::area_damage::PlayerMailboxWrite>,
    pub after: Option<EntitySnapshot>,
    pub projectile: Option<crate::campaign::source_arena::SourceEntityHandle>,
    pub projectile_packet: Option<crate::native_world::Level2FireballProjectile>,
    pub expiry_effect: Option<(crate::campaign::source_arena::SourceEntityHandle, EntitySnapshot)>,
}

#[cfg(test)]
thread_local! {
    static CALLBACK_TRACE: std::cell::RefCell<Option<Vec<Model21CallbackReceipt>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_callback_trace() {
    CALLBACK_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_callback_trace() -> Vec<Model21CallbackReceipt> {
    CALLBACK_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

/// Source1B8C0 can resume on the newborn descriptor65 flyer if allocation
/// replaces its caller. Keep that descriptor's rules at the MC2 boundary.
pub(super) fn motion_override(
    row: &EntitySnapshot,
) -> Option<(
    crate::ground_creature_motion::GroundCreatureMotionConfig,
    u8,
)> {
    (row.parameter_relation == Some(65)).then(|| {
        (
            crate::ground_creature_motion::GroundCreatureMotionConfig {
                half_width: row.array_pitch().max(row.array_roll()),
                half_height: row.array_fov(),
                ground_clearance: 0,
                descent: -4,
                turn_limit: 0,
                terrain_mask: u32::MAX,
                maximum_tile_slope: 256,
            },
            0,
        )
    })
}

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
    source_slot: u16,
    target_slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> Result<(Option<u16>, bool), String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok((None, false));
    };
    let initial_position = if slot == source_slot {
        Position::default()
    } else {
        crate::native_world::source_target::read(arena, context, source_slot)?
            .ok_or("model21 strike source address is absent")?
            .position
    };
    let payload =
        crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
            origin: crate::native_world::FireballProjectileOrigin::Creature21,
            position: initial_position,
            yaw: 0,
            pitch: 0,
            axis_position: Position::default(),
            source_id_entity_index: slot,
            holder_entity_index: 0,
            source_subspell_index: 0,
            source_mana: 50,
            source_dword_0x10: 0,
            target_source_entity_index: 0,
            target_class: 0,
            target_model: 0,
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
    crate::world::effects::sync_position(arena, terrain, slot, initial_position, true, false)?;
    crate::world::effects::lighting::admit_source(
        arena,
        terrain,
        slot,
        initial_position,
        crate::world::effects::lighting::SourceLight::Fireball,
    );
    let source = crate::native_world::source_target::read(arena, context, source_slot)?
        .ok_or("model21 strike lost its current source address")?;
    let target = crate::native_world::source_target::read(arena, context, target_slot)?
        .ok_or("model21 strike lost its current target address")?;
    let height = source
        .collision
        .ok_or("model21 strike current source lacks collision axes")?
        .half_height;
    let guidance = source
        .relations
        .target
        .ok_or("model21 strike current source lacks word96")?;
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(source.position, target.position);
    let projectile = arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|projectile| projectile.source_entity_index == slot)
        .ok_or_else(|| format!("model21 Fireball lost allocated slot{slot}"))?;
    projectile.owner_entity_index = source.source_id;
    projectile.yaw = yaw;
    projectile.pitch = pitch;
    projectile.position.z = projectile.position.z.wrapping_add(height);
    projectile.target_source_entity_index = guidance;
    projectile.target_class = target.class;
    projectile.target_model = target.model;
    projectile.source_subspell_index = RULES.attack_damage as u16;
    projectile.payload = RULES.attack_damage;
    arena.economy.source_owner.insert(slot, source.source_id);
    Ok((
        Some(slot),
        crate::native_world::Level2EconomyState::apply_creature_fireball_target_feedback(
            &mut arena.entities,
            target_slot,
        ),
    ))
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
    // sub_4C8F0 calls sub_268F0(1) after the first visual: action169,
    // byte43=64 and a cleared target, before its explicit collision extents.
    Actor { row: &mut row }.set_action(169);
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
    launch_yaw: i16,
) -> Result<Option<u16>, String> {
    validate_visuals(particles)?;
    arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent.active)
        .ok_or_else(|| format!("model21 summon parent slot{parent_slot} is absent"))?;
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "model21 source constructor counters are absent".to_owned())?[21];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let phase = counter;
    counter = counter.wrapping_add(1);
    let row = construct_row(slot, arena.new_event_rng(slot), position, phase, particles)?;
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 21] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    // The source passes a stack-local launch position, then reads the boss's
    // fixed ID after the constructor. Aliasing therefore sees the newborn ID.
    let source_id = arena.entities[usize::from(parent_slot)].source_id;
    let row = &mut arena.entities[usize::from(slot)];
    row.source_id = source_id;
    put(row, 30, source_id);
    row.set_source_action(175);
    row.record[77] = 17;
    put(row, 44, parent_slot);
    put(row, 50, 250);
    put(row, 154, target_slot);
    row.set_actual_speed(320);
    row.set_yaw(launch_yaw);
    row.set_source_roll(launch_yaw);
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

fn targets(
    arena: &SourceArena,
    pointers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> Result<Vec<Target>, String> {
    let mut rows = super::super::creature_15::mc2::targets_in_arena(arena);
    let typed = arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|row| {
        ((row.active && row.class != 0) || pointers.contains(&row.index))
            && !typed.iter().any(|(slot, _)| *slot == row.index)
            && arena.economy.retained_projectile_kind(row.index).is_none()
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
    for slot in pointers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        let Some(current) = crate::native_world::source_target::read(arena, context, slot)? else {
            continue;
        };
        let index = if let Some(index) = rows.iter().position(|row| row.slot == slot) {
            index
        } else {
            rows.push(Target {
                slot,
                ..Target::default()
            });
            rows.len() - 1
        };
        let row = &mut rows[index];
        row.source_id = current.source_id;
        row.class = current.class;
        row.model = current.model;
        row.life = current.life;
        row.control_flags = current.control_flags;
        row.position = current.position;
        if let Some(kind) = arena.economy.live_arena_kind(slot)
            && let Some((_, action)) = arena.economy.live_arena_class_action(slot, kind)
        {
            row.action = action;
        }
        if let Some(yaw) = current.yaw {
            row.yaw = yaw as u16;
        }
        if let Some(roll) = current.roll {
            row.roll = roll as u16;
        }
        if let Some(shape) = current.collision {
            row.half_width = shape.half_width;
        }
        if let Some(target) = current.relations.target {
            row.target = target;
        }
        if let Some(attacker) = current.relations.attacker {
            row.attacker = attacker;
        }
        if let Some(parent) = current.relations.parent {
            row.parent = parent;
            row.stage.parent = parent;
        }
        if let Some(next) = current.relations.next {
            row.next = next;
        }
        if let Some(index) = current.relations.stage_index {
            row.stage.index = index;
        }
        if let Some(mode) = current.relations.stage_mode {
            row.stage.mode = mode;
        }
        if let Some(relation) = current.relations.stage_relation {
            row.stage.relation = relation;
        }
    }
    rows.sort_unstable_by_key(|row| row.slot);
    Ok(rows)
}

fn current_callback_packet(
    arena: &SourceArena,
    particles: &[ParticleParameter],
    slot: u16,
    targets_context: crate::native_world::source_target::Context<'_>,
) -> Result<EntitySnapshot, String> {
    let flyer = arena
        .economy
        .fireball_projectiles
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .copied();
    if let Some(flyer) = flyer {
        let mut row =
            crate::native_world::source_target::status_parent(arena, targets_context, slot)?
                .ok_or("model21 attack tail lost its current flyer")?;
        row.record[67] = flyer.source_class;
        row.record[68] = flyer.source_model;
        row.set_source_action(flyer.action);
        row.set_max_life(i32::from(flyer.max_life));
        row.set_entity_rng(flyer.entity_rng);
        row.set_phase(flyer.phase);
        row.set_fov(flyer.fov);
        row.set_source_roll(flyer.roll);
        row.record[69] = flyer.target_class;
        row.record[70] = flyer.target_model;
        row.record[71] = 10;
        row.record[72] = 0;
        row.set_subspell_index(flyer.source_subspell_index);
        put(&mut row, 154, flyer.target_source_entity_index);
        put(&mut row, 48, flyer.source_word_2c as u16);
        row.parameter_relation = Some(65);
        crate::world::effects::bind_particle(&mut row, flyer.visual_index, particles)?;
        let shape = crate::native_world::source_target::read(arena, targets_context, slot)?
            .and_then(|view| view.collision)
            .ok_or("model21 attack tail flyer lacks current axes")?;
        for (offset, value) in [
            (86, shape.vertical_offset),
            (88, shape.half_width),
            (90, shape.half_depth),
            (92, shape.half_height),
        ] {
            put(&mut row, offset, value as u16);
        }
        return Ok(row);
    }
    if let Some(splash) = arena
        .economy
        .authored_model5_water_splashes
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let mut row =
            crate::native_world::cave_features::new_event(slot, splash.entity_rng, splash.position);
        row.class = splash.source_class;
        row.model = splash.model;
        row.record[67] = splash.source_class;
        row.record[68] = splash.model;
        row.source_id = splash.entity_id;
        put(&mut row, 30, splash.entity_id);
        row.set_source_action(splash.action);
        row.set_max_life(i32::from(splash.max_life));
        row.set_life(i32::from(splash.life));
        row.set_subspell_index(splash.subspell_index);
        row.set_source_dword_10(splash.dword_0x10);
        row.set_control_flags(splash.control_flags);
        row.set_actual_speed(splash.actual_speed);
        row.set_phase(splash.phase);
        crate::world::effects::bind_particle(&mut row, splash.sprite_index, particles)?;
        row.record[96] = splash.animation_frame;
        if let Some(limit) = splash.animation_limit {
            row.record[97] = limit;
        }
        if let Some(axes) = splash.collision_axes {
            for (offset, value) in [86, 88, 90, 92].into_iter().zip(axes) {
                put(&mut row, offset, value as u16);
            }
        }
        return Ok(row);
    }
    Ok(arena.entities[usize::from(slot)].clone())
}

fn publish_callback_packet(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    mut current: EntitySnapshot,
) -> Result<(), String> {
    let position = current.position;
    arena.economy.source_rng.insert(slot, current.entity_rng);
    if let Some(projectile) = arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        projectile.position = current.position;
        projectile.yaw = current.yaw;
        projectile.roll = current.source_roll();
        projectile.actual_speed = current.actual_speed;
        projectile.life = i16::try_from(current.life)
            .map_err(|_| "model21 attack tail flyer life exceeds source range")?;
        projectile.control_flags = current.control_flags;
        projectile.entity_rng = current.entity_rng;
        projectile.source_word_2c = word(&current, 48) as i16;
        projectile.visual_index = current
            .particle_parameter_index
            .ok_or("model21 attack tail flyer lacks a visual")?;
        projectile.collision_axes = Some([
            current.array_yaw(),
            current.array_pitch(),
            current.array_roll(),
            current.array_fov(),
        ]);
        // Retain this generation's otherwise unowned scalar/animation bytes
        // without creating a live imported fallback for the typed owner.
        current.reclaim_source_row();
    } else if let Some(splash) = arena
        .economy
        .authored_model5_water_splashes
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        splash.action = current.action;
        splash.entity_rng = current.entity_rng;
        splash.position = current.position;
        splash.actual_speed = current.actual_speed;
        splash.control_flags = current.control_flags;
        splash.sprite_index = current
            .particle_parameter_index
            .ok_or("model21 splash tail lacks visual")?;
        splash.animation_frame = current.record[96];
        splash.animation_limit = Some(current.record[97]);
        splash.collision_axes = Some([
            current.array_yaw(),
            current.array_pitch(),
            current.array_roll(),
            current.array_fov(),
        ]);
        current.reclaim_source_row();
    }
    arena.entities[usize::from(slot)] = current;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
}

fn finish_water_splash_current(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    targets_context: crate::native_world::source_target::Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    let Some(position) = effects.water_splash.take() else {
        return Ok(());
    };
    let (hold, continuation) = effects
        .water_splash_tail
        .take()
        .ok_or("model21 splash request lacks its source continuation")?;
    let _ = materialize_water_splash(arena, terrain, position)?;
    let mut current = current_callback_packet(arena, particles, slot, targets_context)?;
    let mut pointers = crate::native_world::source_target::direct_creature_references(
        arena,
        targets_context,
        &current,
        &[],
    )?
    .into_iter()
    .collect::<Vec<_>>();
    pointers.extend_from_slice(class3);
    pointers.extend(families.iter().flatten().copied());
    pointers.push(0);
    let targets = targets(arena, &pointers, targets_context)?;
    let previous_visual = current.particle_parameter_index;
    super::finish_water_splash(
        &mut Actor { row: &mut current },
        &Context {
            terrain,
            targets: &targets,
            class3,
            model21: &families[usize::from(SOURCE_MODEL)],
            families,
            runtime,
        },
        hold,
        continuation,
    );
    if let Some(index) = current.particle_parameter_index
        && previous_visual != Some(index)
    {
        crate::world::effects::bind_particle(&mut current, index, particles)?;
    }
    publish_callback_packet(arena, terrain, slot, current)
}

fn finish_attack_current(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    targets_context: crate::native_world::source_target::Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    let mut current = current_callback_packet(arena, particles, slot, targets_context)?;
    let previous_visual = current.particle_parameter_index;
    let mut tail = Effects::default();
    super::finish_attack(
        &mut Actor { row: &mut current },
        &Context {
            terrain,
            targets: &[],
            class3,
            model21: &families[usize::from(SOURCE_MODEL)],
            families,
            runtime,
        },
        &mut tail,
    );
    if let Some(index) = current.particle_parameter_index
        && previous_visual != Some(index)
    {
        crate::world::effects::bind_particle(&mut current, index, particles)?;
    }
    publish_callback_packet(arena, terrain, slot, current)?;
    for _ in 0..tail.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    finish_water_splash_current(
        arena,
        runtime,
        terrain,
        particles,
        slot,
        class3,
        families,
        targets_context,
        &mut tail,
    )?;
    if tail.sound.is_some() {
        effects.sound = tail.sound;
        effects.sound_position = tail.sound_position;
        effects.sound_identity = tail.sound_identity;
    }
    Ok(())
}

pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    targets_context: crate::native_world::source_target::Context<'_>,
) -> Result<Effects, String> {
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model21 callback lost slot{slot}"))?
        .clone();
    let mut actor_row = row;
    #[cfg(test)]
    let mut callback_receipt = CALLBACK_TRACE.with(|trace| {
        trace
            .borrow()
            .as_ref()
            .filter(|rows| rows.len() < 64)
            .map(|_| {
                let actor = crate::campaign::source_arena::SourceEntityHandle {
                    source_slot: slot,
                    generation: arena.slot_generation(slot),
                };
                Model21CallbackReceipt {
                    actor,
                    before: actor_row.clone(),
                    entry_parent: None,
                    entry_target: None,
                    entry_writer: crate::native_world::area_damage::current_actor_mailbox_writer(
                        actor,
                        (
                            i32::from_le_bytes(actor_row.record[98..102].try_into().unwrap()),
                            word(&actor_row, 102),
                        ),
                    ),
                    after: None,
                    projectile: None,
                    projectile_packet: None,
                    expiry_effect: None,
                }
            })
    });
    let stage_target = runtime
        .stage_variables
        .get(usize::from(actor_row.record[76]))
        .filter(|_| matches!(actor_row.record[77], 3..=5))
        .map_or(0, |stage| {
            if stage.flags as u8 & 2 != 0 {
                word(&actor_row, 78)
            } else {
                match stage.relation {
                    crate::native_world::RuntimeRelation::LiveEntity(slot)
                    | crate::native_world::RuntimeRelation::LiveEntityIndex(slot) => slot,
                    _ => 0,
                }
            }
        });
    let mut pointers = crate::native_world::source_target::direct_creature_references(
        arena,
        targets_context,
        &actor_row,
        &[stage_target],
    )?
    .into_iter()
    .collect::<Vec<_>>();
    pointers.extend_from_slice(class3);
    pointers.extend(families.iter().flatten().copied());
    pointers.push(0);
    let targets = targets(arena, &pointers, targets_context)?;
    #[cfg(test)]
    if let Some(receipt) = callback_receipt.as_mut() {
        receipt.entry_parent = targets.iter().find(|target| target.slot == word(&actor_row, 44)).copied();
        receipt.entry_target = targets.iter().find(|target| target.slot == word(&actor_row, 154)).copied();
    }
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
    finish_water_splash_current(
        arena,
        runtime,
        terrain,
        particles,
        slot,
        class3,
        families,
        targets_context,
        &mut effects,
    )?;
    if let Some(target) = effects.strike_target.take() {
        let (projectile, feedback) = launch_strike(arena, terrain, slot, target, targets_context)?;
        if feedback {
            effects.target_music = Some(target);
        }
        #[cfg(test)]
        if let Some(receipt) = callback_receipt.as_mut() {
            receipt.projectile = projectile.and_then(|slot| arena.handle(slot));
            receipt.projectile_packet = projectile.and_then(|slot| {
                arena
                    .economy
                    .fireball_projectiles
                    .iter()
                    .find(|row| row.source_entity_index == slot)
                    .copied()
            });
        }
        let _ = projectile;
        finish_attack_current(
            arena,
            runtime,
            terrain,
            particles,
            slot,
            class3,
            families,
            targets_context,
            &mut effects,
        )?;
    }
    if effects.kill {
        kill(arena, terrain, particles, slot)?;
    }
    if effects.expire {
        #[cfg(test)]
        let previous_impacts = callback_receipt.as_ref().map(|_| arena.entities.iter()
            .filter(|row| row.active && (row.class, row.model) == (10, 0))
            .filter_map(|row| arena.handle(row.index)).collect::<Vec<_>>());
        super::super::creature_24::mc2::expire(
            arena,
            terrain,
            particles,
            slot,
            effects.summon_expire,
        )?;
        #[cfg(test)]
        if let (Some(receipt), Some(previous)) = (callback_receipt.as_mut(), previous_impacts) {
            // This synchronous mode16 helper makes exactly one Impact birth;
            // no other actor callback can interleave between these snapshots.
            receipt.expiry_effect = arena.entities.iter()
                .filter(|row| row.active && (row.class, row.model) == (10, 0))
                .find_map(|row| arena.handle(row.index)
                    .filter(|handle| !previous.contains(handle)).map(|handle| (handle, row.clone())));
        }
        // sub_26470 runs the jump/visual tail after sub_1E580 has
        // allocated expiry children and hidden the current slot. Allocation
        // can replace that address, so use its current occupant.
        let mut current = arena.entities[usize::from(slot)].clone();
        let mut tail = Effects::default();
        if matches!(current.record[77], 13 | 16) {
            current.record[71] = 0;
            let targets = self::targets(arena, &pointers, targets_context)?;
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
            if let Some((_, continuation)) = tail.water_splash_tail.as_mut() {
                *continuation = super::WaterContinuation::Stage;
            } else if current.action != 175 {
                let action = current.action;
                Actor { row: &mut current }.set_action(action);
            }
            if let Some(index) = current.particle_parameter_index
                && previous_visual != Some(index)
            {
                crate::world::effects::bind_particle(&mut current, index, particles)?;
            }
            let position = current.position;
            arena.economy.source_rng.insert(slot, current.entity_rng);
            arena.entities[usize::from(slot)] = current;
            crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
            finish_water_splash_current(
                arena,
                runtime,
                terrain,
                particles,
                slot,
                class3,
                families,
                targets_context,
                &mut tail,
            )?;
            if tail.sound.is_some() {
                effects.sound = tail.sound;
                effects.sound_position = tail.sound_position;
                effects.sound_identity = tail.sound_identity;
            }
        }
    }
    #[cfg(test)]
    if let Some(mut receipt) = callback_receipt {
        receipt.after = (arena.handle(slot) == Some(receipt.actor))
            .then(|| &arena.entities[usize::from(slot)])
            .filter(|row| row.class == 5 && row.model == SOURCE_MODEL)
            .cloned();
        CALLBACK_TRACE.with(|trace| {
            if let Some(rows) = trace.borrow_mut().as_mut() {
                rows.push(receipt);
            }
        });
    }
    Ok(effects)
}

/// Compact post-allocation feedback for the shared sound/music tail.
pub(crate) fn target_music(effects: &Effects) -> Option<u16> {
    effects.target_music
}

fn kill(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    let original = arena
        .handle(slot)
        .ok_or("model21 death lost its original handle")?;
    let source = arena.entities[usize::from(slot)].clone();
    if source.mana > 0 {
        let next = crate::native_world::level2_next_rng16(source.entity_rng);
        arena.entities[usize::from(slot)].set_entity_rng(next);
        arena.economy.source_rng.insert(slot, next);
        crate::world::effects::spawn_death_mana_sphere(
            arena,
            terrain,
            slot,
            source.position,
            source.yaw,
            source.mana,
            source.source_player_entity_index(),
        )?;
        if !crate::world::effects::clear_replacement_mana_owner(arena, slot)? {
            arena.entities[usize::from(slot)].set_source_player_entity_index(0);
        }
        arena.economy.source_owner.insert(slot, 0);
    }
    let current = crate::native_world::source_target::read(arena, Default::default(), slot)?
        .ok_or("model21 death lost its current source address")?;
    let burst = if current.control_flags & 0x100000 == 0 {
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
    if !crate::world::effects::hide_birth_replacement(arena, slot, burst)? {
        if arena.resolve(original).is_none() {
            return Err(format!("model21 death cannot hide replacement at {slot}"));
        }
        // Source57F10 marks drawing; the next source prologue unlinks it.
        arena.entities[usize::from(slot)].disable_drawing();
    }
    Ok(())
}
