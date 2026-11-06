//! Model-18 bindings to the shared source arena, StageVar table and assets.

use super::{
    Creature18Context, Creature18Effects, Creature18State, SOURCE_CLASS, SOURCE_MODEL, Target,
};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena};
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

pub(super) const DESCRIPTOR: u16 = 86;
pub(super) const PARTICLE: u16 = 286;

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct SentinelFanShotReceipt {
    pub offset: i16,
    pub current_actor: crate::campaign::source_arena::SourceEntityHandle,
    pub current_target: crate::campaign::source_arena::SourceEntityHandle,
    pub source_packet: crate::native_world::source_target::View,
    pub target_packet: crate::native_world::source_target::View,
    pub projectile: crate::campaign::source_arena::SourceEntityHandle,
    pub packet: crate::native_world::Level2FireballProjectile,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct SentinelFanReceipt {
    pub actor: crate::campaign::source_arena::SourceEntityHandle,
    pub selected_target: crate::campaign::source_arena::SourceEntityHandle,
    pub shots: Vec<SentinelFanShotReceipt>,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct SentinelCallbackReceipt {
    pub actor: crate::campaign::source_arena::SourceEntityHandle,
    pub before: Creature18State,
    pub entry_writer: Option<crate::native_world::area_damage::PlayerMailboxWrite>,
    pub after: Option<Creature18State>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct SentinelKillCreditReceipt {
    pub source: crate::campaign::source_arena::SourceEntityHandle,
    pub killer: u16,
    pub before_count: u32,
    pub after_count: u32,
}

#[cfg(test)]
thread_local! {
    static FAN_BIRTH_TRACE: std::cell::RefCell<Option<Vec<SentinelFanReceipt>>> =
        const { std::cell::RefCell::new(None) };
    static CALLBACK_TRACE: std::cell::RefCell<Option<Vec<SentinelCallbackReceipt>>> =
        const { std::cell::RefCell::new(None) };
    static KILL_CREDIT_TRACE: std::cell::RefCell<Option<Vec<SentinelKillCreditReceipt>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn start_fan_birth_trace() {
    FAN_BIRTH_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_fan_birth_trace() -> Vec<SentinelFanReceipt> {
    FAN_BIRTH_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(crate) fn start_callback_trace() {
    CALLBACK_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_callback_trace() -> Vec<SentinelCallbackReceipt> {
    CALLBACK_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(crate) fn start_kill_credit_trace() {
    KILL_CREDIT_TRACE.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_kill_credit_trace() -> Vec<SentinelKillCreditReceipt> {
    KILL_CREDIT_TRACE.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
pub(crate) fn kill_credit_trace_enabled() -> bool {
    KILL_CREDIT_TRACE.with(|trace| trace.borrow().as_ref().is_some_and(|rows| rows.len() < 64))
}

#[cfg(test)]
pub(crate) fn record_kill_credit_receipt(receipt: SentinelKillCreditReceipt) {
    KILL_CREDIT_TRACE.with(|trace| {
        if let Some(rows) = trace.borrow_mut().as_mut().filter(|rows| rows.len() < 64) {
            rows.push(receipt);
        }
    });
}

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

fn targets(
    arena: &SourceArena,
    pointers: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> Result<Vec<Target>, String> {
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
        ((row.active && row.class != 0) || pointers.contains(&row.index))
            && !typed.iter().any(|(slot, _)| *slot == row.index)
            && arena.economy.retained_projectile_kind(row.index).is_none()
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
    // Frozen and directly referenced addresses can now contain a typed
    // projectile absent from the defender projection. Read that CURRENT
    // owner's fields; never revive its old imported packet.
    let referenced = pointers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for slot in referenced {
        let Some(current) = crate::native_world::source_target::read(arena, context, slot)? else {
            continue;
        };
        let index = if let Some(index) = targets.iter().position(|row| row.slot == slot) {
            index
        } else {
            targets.push(Target {
                slot,
                ..Target::default()
            });
            targets.len() - 1
        };
        let row = &mut targets[index];
        row.source_id = current.source_id;
        row.class = current.class;
        row.model = current.model;
        row.life = current.life;
        row.flags = current.control_flags;
        row.position = current.position;
        if let Some(kind) = arena.economy.live_arena_kind(slot)
            && let Some((_, action)) = arena.economy.live_arena_class_action(slot, kind)
        {
            row.action = action;
        }
        if let Some(yaw) = current.yaw {
            row.yaw = yaw;
        }
        if let Some(roll) = current.roll {
            row.roll = roll;
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
        }
        if let Some(next) = current.relations.next {
            row.linked_child = next;
        }
        if let Some(index) = current.relations.stage_index {
            row.route_index = index;
        }
        if let Some(mode) = current.relations.stage_mode {
            row.route_mode = mode;
        }
        if let Some(relation) = current.relations.stage_relation {
            row.route_relation = relation;
        }
    }
    targets.sort_unstable_by_key(|target| target.slot);
    Ok(targets)
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

/// Materialize one `sub_1D460` fan member in the shared typed Fireball arena.
/// The source caller replaces constructor descriptor64 with descriptor61;
/// `Creature18` is the typed carrier identity for that descriptor behavior.
fn launch_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    actor_slot: u16,
    target_slot: u16,
    offset: i16,
    context: crate::native_world::source_target::Context<'_>,
    #[cfg(test)] receipt: Option<&mut SentinelFanReceipt>,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    // NewEvent clears its fixed row before AddEventToMap reads the source
    // position by address. Source aliasing therefore inserts at zero.
    let initial_position = if slot == actor_slot {
        Position::default()
    } else {
        crate::native_world::source_target::read(arena, context, actor_slot)?
            .ok_or("model-18 fan source address is absent")?
            .position
    };
    let payload =
        crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
            origin: crate::native_world::FireballProjectileOrigin::Creature18,
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
    arena
        .economy
        .initialize_fireball_projectile(slot, payload, 0)
        .ok_or_else(|| format!("model-18 Fireball map linkage failed at slot{slot}"))?;
    crate::world::effects::sync_position(arena, terrain, slot, initial_position, true, false)?;
    crate::world::effects::lighting::admit_source(
        arena,
        terrain,
        slot,
        initial_position,
        crate::world::effects::lighting::SourceLight::Fireball,
    );
    // sub_1D460 reads both fixed addresses only after this member's
    // constructor. Earlier members may also have replaced either address.
    let source = crate::native_world::source_target::read(arena, context, actor_slot)?
        .ok_or("model-18 fan lost its current source address")?;
    let target = crate::native_world::source_target::read(arena, context, target_slot)?
        .ok_or("model-18 fan lost its current target address")?;
    let guidance = source
        .relations
        .target
        .ok_or_else(|| format!("model-18 current source {actor_slot} lacks word96"))?;
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(source.position, target.position);
    let projectile = arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|projectile| projectile.source_entity_index == slot)
        .ok_or_else(|| format!("model-18 Fireball lost allocated slot{slot}"))?;
    projectile.target_class = target.class;
    projectile.target_model = target.model;
    projectile.source_subspell_index = super::RULES.projectile_damage;
    projectile.payload = i32::from(super::RULES.projectile_damage);
    projectile.owner_entity_index = source.source_id;
    // sub_1D460 stores the signed sum; sub_65610 masks it only on flight.
    projectile.yaw = yaw.wrapping_add(offset);
    projectile.pitch = pitch;
    projectile.position.z = projectile.position.z.wrapping_add(super::RULES.fan_lift);
    projectile.target_source_entity_index = guidance;
    arena.economy.source_owner.insert(slot, source.source_id);
    #[cfg(test)]
    if let Some(receipt) = receipt {
        let packet = *arena
            .economy
            .fireball_projectiles
            .iter()
            .find(|row| row.source_entity_index == slot)
            .ok_or("model-18 fan receipt lost published projectile")?;
        receipt.shots.push(SentinelFanShotReceipt {
            offset,
            current_actor: crate::campaign::source_arena::SourceEntityHandle {
                source_slot: actor_slot,
                generation: arena.slot_generation(actor_slot),
            },
            current_target: crate::campaign::source_arena::SourceEntityHandle {
                source_slot: target_slot,
                generation: arena.slot_generation(target_slot),
            },
            source_packet: source,
            target_packet: target,
            projectile: arena
                .handle(slot)
                .ok_or("model-18 fan birth lost its handle")?,
            packet,
        });
    }
    Ok(Some(slot))
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
    targets_context: crate::native_world::source_target::Context<'_>,
) -> Result<Creature18Effects, String> {
    let actor_row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model-18 callback lost slot{slot}"))?;
    let mut actor = load(actor_row, arena.slot_generation(slot));
    #[cfg(test)]
    let callback_receipt = CALLBACK_TRACE.with(|trace| {
        trace
            .borrow()
            .as_ref()
            .filter(|rows| rows.len() < 64)
            .map(|_| {
                let handle = crate::campaign::source_arena::SourceEntityHandle {
                    source_slot: slot,
                    generation: actor.generation,
                };
                SentinelCallbackReceipt {
                    actor: handle,
                    before: actor,
                    entry_writer: crate::native_world::area_damage::current_actor_mailbox_writer(
                        handle,
                        (actor.pending_damage, actor.pending_attacker),
                    ),
                    after: None,
                }
            })
    });
    // These callbacks dereference fixed targets, attackers and child chains
    // independently of route mode or current frozen-list membership.
    let stage_target = runtime
        .stage_variables
        .get(usize::from(actor.route_index))
        .filter(|_| matches!(actor.route_mode, 3..=5))
        .map_or(0, |stage| {
            if stage.flags as u8 & 2 != 0 {
                actor.route_relation
            } else {
                match stage.relation {
                    crate::native_world::RuntimeRelation::LiveEntity(slot)
                    | crate::native_world::RuntimeRelation::LiveEntityIndex(slot) => slot,
                    _ => 0,
                }
            }
        });
    let mut pointers: Vec<_> = crate::native_world::source_target::direct_creature_references(
        arena,
        targets_context,
        actor_row,
        &[stage_target],
    )?
    .into_iter()
    .collect();
    // Source lists freeze addresses, not the eligibility or fields of their
    // later occupants. Exact dereferences also retain the real sentinel row;
    // the core's ordinary target lookup still treats zero as no target.
    pointers.extend_from_slice(class3);
    pointers.extend(families.iter().flatten().copied());
    pointers.push(0);
    let targets = targets(arena, &pointers, targets_context)?;
    let callback_action = actor.action;
    let context = Creature18Context {
        terrain,
        runtime,
        targets: &targets,
        class3,
        model18: &families[usize::from(SOURCE_MODEL)],
        families,
    };
    let mut effects = super::step(&mut actor, &context)
        .ok_or_else(|| format!("unsupported model-18 action{}", actor.action))?;
    let row = &mut arena.entities[usize::from(slot)];
    store(row, actor, particles)?;
    arena.economy.source_rng.insert(slot, actor.rng);
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    for (linked_slot, flags) in &effects.linked_blink_flags {
        if let Some(row) = arena
            .entities
            .get_mut(usize::from(*linked_slot))
            .filter(|row| row.active)
        {
            row.set_control_flags((row.control_flags & !0x0600_0000) | *flags);
        }
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
    if let Some(target) = effects.fan_target.take() {
        #[cfg(test)]
        let mut receipt = FAN_BIRTH_TRACE.with(|trace| {
            trace
                .borrow()
                .as_ref()
                .filter(|rows| rows.len() < 64)
                .map(|_| SentinelFanReceipt {
                    actor: crate::campaign::source_arena::SourceEntityHandle {
                        source_slot: slot,
                        generation: arena.slot_generation(slot),
                    },
                    selected_target: crate::campaign::source_arena::SourceEntityHandle {
                        source_slot: target,
                        generation: arena.slot_generation(target),
                    },
                    shots: Vec::new(),
                })
        });
        for offset in super::RULES.fan_yaw_offsets {
            if let Some(projectile_slot) = launch_projectile(
                arena,
                terrain,
                slot,
                target,
                offset,
                targets_context,
                #[cfg(test)]
                receipt.as_mut(),
            )? {
                effects.projectile_slots.push(projectile_slot);
            }
        }
        #[cfg(test)]
        if let Some(receipt) = receipt {
            FAN_BIRTH_TRACE.with(|trace| {
                if let Some(rows) = trace.borrow_mut().as_mut() {
                    rows.push(receipt);
                }
            });
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
    #[cfg(test)]
    if let Some(mut receipt) = callback_receipt {
        receipt.after = (arena.handle(slot) == Some(receipt.actor))
            .then(|| &arena.entities[usize::from(slot)])
            .filter(|row| (row.class, row.model) == (SOURCE_CLASS, SOURCE_MODEL))
            .map(|row| load(row, receipt.actor.generation));
        CALLBACK_TRACE.with(|trace| {
            if let Some(rows) = trace.borrow_mut().as_mut() {
                rows.push(receipt);
            }
        });
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
    super::super::creature_24::mc2::expire(arena, terrain, particles, slot, summoned)?;
    // sub_25550 resumes after sub_1E580, including its allocations and hide.
    // These writes belong to that CURRENT address, never the old actor copy.
    let generation = arena.slot_generation(slot);
    let row = &mut arena.entities[usize::from(slot)];
    let mut position = row.position;
    position.z = terrain.ground_height(position);
    row.set_position(position);
    if row.action == 146 {
        let mut current = load(row, generation);
        super::request_action(&mut current, 2, 0);
        row.set_entity_rng(current.rng);
        row.set_source_dword_10(current.timer);
        row.set_source_action(current.action);
        row.record[record::ATTACK_PHASE] = current.attack_phase;
        arena.economy.source_rng.insert(slot, current.rng);
    }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(())
}

fn kill(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    let original = arena
        .handle(slot)
        .ok_or("model-18 death lost its original handle")?;
    let source = arena.entities[usize::from(slot)].clone();
    let (position, yaw, mana, owner) = (
        source.position,
        source.yaw,
        source.mana,
        source.source_player_entity_index(),
    );
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
        .ok_or("model-18 death lost its current source address")?;
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
            return Err(format!("model-18 death cannot hide replacement at {slot}"));
        }
        let row = &mut arena.entities[usize::from(slot)];
        // DisableEntityDrawing04_57F10 marks drawing only. Its map links
        // remain current until the following source reclaim prologue.
        row.disable_drawing();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_fan_replacement_reads_current_actor_and_target_after_new_event() {
        // Explicit allocator-pressure and callback-state controls over the
        // actual constructors. No ordinary mission pressure is claimed.
        let package = std::env::var_os("MC_LEVEL24_PACKAGE").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
                    "../../../generated/ribbon-20261104-integration/current-owner-package/level24",
                )
            },
            std::path::PathBuf::from,
        );
        let session = crate::campaign::CampaignSession::open(
            package,
            crate::campaign::spec::CampaignSelection::ordinary(24).unwrap(),
        )
        .expect("whole campaign runtime retains the source constructor counters")
        .simulation()
        .clone();
        for replace_actor in [true, false] {
            let mut arena = session.source_arena().clone();
            let mut runtime = session.runtime().clone();
            let mut terrain = session.canonical_terrain().clone();
            let particles = &session.session.definition.presentation.particles;
            let player = session.state.entity_index;
            let original_target = arena.handle(player).unwrap();
            let mut position = arena.entities[usize::from(player)].position;
            position.x = position.x.wrapping_add(1024);
            let actor = allocate_authored(
                &mut arena,
                &mut runtime,
                &mut terrain,
                particles,
                position,
                super::super::RULES.max_life,
                None,
                601,
            )
            .unwrap()
            .unwrap();
            position.x = position.x.wrapping_add(1024);
            let follower = allocate_authored(
                &mut arena,
                &mut runtime,
                &mut terrain,
                particles,
                position,
                super::super::RULES.max_life,
                None,
                602,
            )
            .unwrap()
            .unwrap();
            let original_actor = arena.handle(actor).unwrap();
            {
                let row = &mut arena.entities[usize::from(actor)];
                row.set_source_action(146);
                row.set_phase(0);
                row.set_source_dword_10(10);
                row.record[record::ATTACK_PHASE] = 1;
                put(row, 154, player);
                // A different owner makes the post-NewEvent ID copy visible.
                row.source_id = player;
                put(row, 30, player);
            }
            {
                let row = &mut arena.entities[usize::from(follower)];
                row.set_source_action(151);
                row.set_phase(1);
                row.record[77] = 16;
                put(row, 44, actor);
                put(row, 50, 32);
            }
            for row in &mut arena.entities {
                row.set_control_flags(
                    row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                );
            }
            let replaced = if replace_actor { actor } else { player };
            let row = &mut arena.entities[usize::from(replaced)];
            row.set_control_flags(
                row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
            arena.economy.exhaust_new_event_capacity_for_test();
            arena.rebuild_source_free_arena();
            let mut families = std::array::from_fn(|_| Vec::new());
            families[usize::from(SOURCE_MODEL)] = vec![actor, follower];
            start_fan_birth_trace();
            let outcome = step_row(
                &mut arena,
                &mut runtime,
                &mut terrain,
                particles,
                actor,
                &[player],
                &families,
                Default::default(),
            )
            .unwrap();
            let receipts = take_fan_birth_trace();
            let receipt = &receipts[0];
            assert_eq!(
                (receipt.actor, receipt.selected_target),
                (original_actor, original_target)
            );
            assert_eq!(outcome.projectile_slots, [replaced]);
            assert_eq!(
                receipt.shots.len(),
                1,
                "the other four attempts lack capacity"
            );
            let shot = &receipt.shots[0];
            assert_eq!((shot.offset, shot.projectile.source_slot), (-226, replaced));
            assert_eq!(arena.handle(replaced), Some(shot.projectile));
            assert_eq!(shot.current_actor, arena.handle(actor).unwrap());
            assert_eq!(shot.current_target, arena.handle(player).unwrap());
            let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
                shot.source_packet.position,
                shot.target_packet.position,
            );
            assert_eq!(
                (shot.packet.yaw, shot.packet.pitch),
                (yaw.wrapping_sub(226), pitch)
            );
            assert_eq!((shot.packet.roll, shot.packet.fov), (0, 0));
            if replace_actor {
                assert!(arena.resolve(original_actor).is_none());
                assert_eq!(shot.source_packet.position, Position::default());
                assert_eq!(shot.packet.owner_entity_index, actor);
                assert_eq!(shot.packet.target_source_entity_index, 0);
                assert_eq!(
                    shot.packet.position,
                    Position {
                        z: super::super::RULES.fan_lift,
                        ..Position::default()
                    }
                );
                assert_eq!(
                    arena.economy.map_chains.tile_of(actor),
                    0,
                    "the old actor packet must not republish over its newborn"
                );
                // A valid current projectile parent is absent from the old
                // defender projection. Source1E580 still follows that address.
                let followed = step_row(
                    &mut arena,
                    &mut runtime,
                    &mut terrain,
                    particles,
                    follower,
                    &[],
                    &families,
                    Default::default(),
                )
                .unwrap();
                assert!(!followed.expire && !followed.route_hide);
                assert_eq!(arena.entities[usize::from(follower)].record[77], 16);
                assert!(!arena.entities[usize::from(follower)].drawing_disabled());
                assert_eq!(arena.handle(actor), Some(shot.projectile));
            } else {
                assert!(arena.resolve(original_target).is_none());
                assert_eq!((shot.target_packet.class, shot.target_packet.model), (9, 0));
                assert_eq!((shot.packet.target_class, shot.packet.target_model), (9, 0));
                assert_eq!(shot.source_packet.position, shot.target_packet.position);
                assert_eq!(shot.packet.owner_entity_index, player);
                assert_eq!(shot.packet.target_source_entity_index, player);
                assert_eq!(arena.handle(actor), Some(original_actor));
            }
        }
    }
}
