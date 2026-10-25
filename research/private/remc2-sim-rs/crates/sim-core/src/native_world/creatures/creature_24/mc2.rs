//! Lossless MC2 adapter. Callback-local typed state is committed to the same
//! arena row before any allocation can replace its fixed address.

use super::{
    Creature24Context, Creature24Effects, Creature24State, DESCRIPTOR, SOURCE_MODEL, StageState,
    Target, bind_stage, stage_prepass,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    EntitySnapshot, MapType, ParticleParameter, RuntimeSnapshot, SessionTerrain,
};
use crate::{FlightWorld, Position};

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}
fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn load(row: &EntitySnapshot, generation: u32) -> Creature24State {
    Creature24State {
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
        visual: row.particle_parameter_index.unwrap_or(335),
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
    actor: Creature24State,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    if row.index != actor.slot {
        return Err("cave creature carrier address changed".to_owned());
    }
    if row.particle_parameter_index != Some(actor.visual) || actor.animation_limit == 0 {
        let particle = particles
            .get(usize::from(actor.visual))
            .ok_or_else(|| format!("cave creature requires particle{}", actor.visual))?;
        row.record[97] = particle.animation_limit().ok_or_else(|| {
            format!(
                "cave creature particle{} has invalid animation mode",
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
    put(row, 136, actor.minimum_speed as u16);
    put(row, 138, actor.maximum_speed as u16);
    row.set_max_life(actor.max_life);
    row.set_life(actor.life);
    row.set_mana(actor.mana);
    row.max_mana = actor.max_mana;
    row.mana_cost = actor.max_mana;
    row.record[144..148].copy_from_slice(&actor.max_mana.to_le_bytes());
    row.set_subspell_index(actor.damage);
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
    // Validate retained constructor dependencies before consuming allocation's
    // model counter. Non-Cave rejection does not require those assets.
    if map_type != MapType::Cave {
        return Ok(None);
    }
    let particle = particles
        .get(335)
        .ok_or_else(|| "cave creature requires retained particle335".to_owned())?;
    if particle.animation_limit().is_none() {
        return Err("cave creature particle335 has invalid animation mode".to_owned());
    }
    let mut actor = super::construct(slot, generation, seed, position, map_type, counter)
        .expect("Cave constructor");
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
            half_width: row.half_width,
            stage: row.stage,
        })
        .collect::<Vec<_>>();
    // Raw rows newly allocated by this family have nonzero generations and
    // must not disappear behind the imported-only filter in old family views.
    let typed = arena.economy.live_arena_slots();
    for row in arena.entities.iter().filter(|row| {
        row.active
            && row.class != 0
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
        .ok_or_else(|| format!("cave creature StageVar bind lost slot{slot}"))?;
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
    if terrain.map_type() != MapType::Cave {
        return Ok(None);
    }
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "cave creature source constructor counter is absent".to_owned())?[24];
    let Some(particle) = particles.get(335) else {
        return Err("cave creature requires retained particle335".to_owned());
    };
    if particle.animation_limit().is_none() {
        return Err("cave creature particle335 has invalid animation mode".to_owned());
    }
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let mut row = construct_row(
        slot,
        arena.slot_generation(slot),
        arena.new_event_rng(slot),
        position,
        MapType::Cave,
        &mut counter,
        particles,
    )?
    .expect("validated Cave constructor");
    row.bind_frame_zero_max_life(max_life);
    // The immutable constructor default mana remains8000 when authored health
    // is changed, exactly as for the other typed creature health bindings.
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 24] = counter;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_authored_route(arena, runtime, slot, index, particles)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("cave creature constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Creature24,
        crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "cave creature record{authored_record} lost generation-bound origin"
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
    let targets = targets(arena);
    for slot in &families[usize::from(SOURCE_MODEL)] {
        let Some(row) = arena
            .entities
            .get(usize::from(*slot))
            .filter(|r| r.active && r.class == 5 && r.model == SOURCE_MODEL)
        else {
            continue;
        };
        let mut actor = load(row, arena.slot_generation(*slot));
        let mut context = Creature24Context {
            terrain,
            targets: &targets,
            class3: &[],
            model24: &families[usize::from(SOURCE_MODEL)],
            class5: &[],
            stage_runtime: Some(runtime),
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
) -> Result<Creature24Effects, String> {
    let targets = targets(arena);
    let class5 = families.iter().flatten().copied().collect::<Vec<_>>();
    let generation = arena.slot_generation(slot);
    let row = arena
        .entities
        .get(usize::from(slot))
        .filter(|r| r.active && r.class == 5 && r.model == SOURCE_MODEL)
        .ok_or_else(|| format!("cave creature callback lost slot{slot}"))?;
    let mut actor = load(row, generation);
    let mut context = Creature24Context {
        terrain,
        targets: &targets,
        class3,
        model24: &families[usize::from(SOURCE_MODEL)],
        class5: &class5,
        stage_runtime: Some(runtime),
    };
    let effects = super::step(&mut actor, &mut context)
        .ok_or_else(|| format!("unsupported cave creature action{}", actor.action))?;
    store(&mut arena.entities[usize::from(slot)], actor, particles)?;
    arena.economy.source_rng.insert(slot, actor.rng);
    for update in &effects.linked_updates {
        if let Some(row) = arena
            .entities
            .get_mut(usize::from(update.slot))
            .filter(|r| r.active && r.class == 5 && r.model == SOURCE_MODEL)
        {
            row.set_source_action(update.action);
            row.set_control_flags(update.flags);
            put(row, 154, update.target);
            put(row, 54, update.previous);
        }
    }
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    crate::world::effects::sync_position(arena, terrain, slot, actor.position, true, false)?;
    if effects.kill {
        kill(arena, terrain, particles, slot)?;
    }
    if effects.expire {
        expire(arena, terrain, particles, slot, effects.summon_expire)?;
    }
    Ok(effects)
}

fn current(arena: &SourceArena, slot: u16) -> (Position, u16, i16, i32, u16, u32) {
    if let Some(s) = arena.spheres.iter().find(|s| s.source_entity_index == slot) {
        return (
            s.position,
            slot,
            s.yaw,
            s.mana,
            s.player_entity_index,
            s.control_flags,
        );
    }
    let row = &arena.entities[usize::from(slot)];
    (
        row.position,
        row.source_id,
        row.yaw,
        row.mana,
        row.source_player_entity_index(),
        row.control_flags,
    )
}
fn hide_current(arena: &mut SourceArena, slot: u16) {
    if let Some(s) = arena
        .economy
        .spheres
        .iter_mut()
        .find(|s| s.source_entity_index == slot)
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
) -> Result<(), String> {
    let (position, _, yaw, mana, owner, _) = current(arena, slot);
    if mana > 0 {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_entity_rng(super::level2_next_rng16(row.entity_rng));
        arena.economy.source_rng.insert(slot, row.entity_rng);
        spawn_mana(arena, terrain, slot, position, yaw, mana, owner)?;
        // The source dereferences the same fixed address AFTER allocation.
        if let Some(s) = arena
            .economy
            .spheres
            .iter_mut()
            .find(|s| s.source_entity_index == slot)
        {
            s.player_entity_index = 0;
            s.owner_entity_index = 0;
        } else {
            arena.entities[usize::from(slot)].set_source_player_entity_index(0);
        }
    }
    let (position, _, _, _, _, flags) = current(arena, slot);
    if flags & 0x100000 == 0 {
        let child = crate::world::effects::allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::RadialExplosion,
            ),
            position,
            particles,
        )?;
        if let Some(child) = child {
            let id = current(arena, slot).1;
            let row = &mut arena.entities[usize::from(child)];
            row.source_id = id;
            put(row, 30, id);
            if child == slot {
                row.set_position(Position::default());
                crate::world::effects::sync_position(
                    arena,
                    terrain,
                    slot,
                    Position::default(),
                    true,
                    true,
                )?;
            }
        }
    }
    hide_current(arena, slot);
    Ok(())
}

pub(crate) fn spawn_mana(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    parent: u16,
    position: Position,
    yaw: i16,
    mana: i32,
    owner: u16,
) -> Result<(), String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(());
    };
    let self_replaced = slot == parent;
    let position = if self_replaced {
        Position::default()
    } else {
        position
    };
    let seed = arena.new_event_rng(slot);
    // 50080/500C0 and SetManaSphereColorAndRot do not advance the stream.
    let yaw_rng = super::level2_next_rng16(seed);
    let source_yaw = if self_replaced { 0 } else { yaw };
    let yaw = (i32::from(yaw_rng % 113) + i32::from(source_yaw) - 56) as i16 & 2047;
    let speed_rng = super::level2_next_rng16(yaw_rng);
    let speed = (speed_rng % 48 + 16) as i16;
    let mut velocity = Position::default();
    crate::move_entity(&mut velocity, yaw, 0, speed);
    let ground = terrain.ground_height(position);
    let difference = 1024 - (i32::from(position.z) - i32::from(ground));
    let vertical_velocity = (difference / 8) as i16;
    let sphere = crate::level1_mission::ManaSphere {
        id: u32::from(slot),
        source_entity_index: slot,
        entity_rng: speed_rng,
        position,
        velocity_x: velocity.x as i16,
        velocity_y: velocity.y as i16,
        vertical_velocity,
        yaw,
        max_life: 300,
        life: 300,
        mana: if self_replaced { 512 } else { mana },
        owner_entity_index: if self_replaced { 0 } else { owner },
        player_entity_index: if self_replaced { 0 } else { owner },
        incoming_owner_entity_index: 0,
        incoming_possession_payload: 0,
        ordinary_damage_payload: 0,
        ordinary_damage_owner: 0,
        attraction_source_entity_index: 0,
        attraction_step: 0,
        attraction_distance_high: 0,
        assigned_carrier_id: 0,
        assigned_defender_slot: 0,
        control_flags: 12,
        phase: slot as u8,
        active: true,
        pending_delete: false,
    };
    arena.economy.spheres.push(sphere);
    arena.economy.source_rng.insert(slot, speed_rng);
    arena.economy.refresh_owner_mana_rows();
    arena
        .economy
        .events
        .push(crate::native_world::Level2EconomyEvent::ManaSphereSpawned {
            sphere_id: u32::from(slot),
            source_entity_index: parent,
            mana: sphere.mana,
        });
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
}

fn expire(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    summoned: bool,
) -> Result<(), String> {
    let position = current(arena, slot).0;
    let generation = arena.slot_generation(slot);
    if summoned {
        super::super::goat::runtime::spawn_expiry_visual(arena, terrain, position, particles)?;
    }
    if summoned && arena.slot_generation(slot) != generation {
        let mut zero = Position::default();
        zero.z = terrain.ground_height(zero);
        arena.entities[usize::from(slot)].set_position(zero);
        crate::world::effects::sync_position(arena, terrain, slot, zero, true, true)?;
    }
    let position = if arena.slot_generation(slot) != generation {
        current(arena, slot).0
    } else {
        position
    };
    let child = crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        position,
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
    hide_current(arena, slot);
    Ok(())
}
