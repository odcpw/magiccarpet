//! Cymmerian bindings to the live MC2 source arena.

use super::{
    Actor, Context, DESCRIPTOR, Effects, PARTICLE, RULES, SOURCE_CLASS, SOURCE_MODEL,
    SPLIT_DESCRIPTOR, SPLIT_PARTICLE, Target,
};
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena, SourceEntityHandle};
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn target_at(
    arena: &SourceArena,
    context: crate::native_world::source_target::Context<'_>,
    slot: u16,
) -> Result<Option<Target>, String> {
    use crate::native_world::source_target;
    Ok(
        source_target::read(arena, context, slot)?.map(|view| Target {
            slot,
            source_id: view.source_id,
            class: view.class,
            model: view.model,
            life: view.life,
            flags: view.control_flags,
            position: view.position,
            yaw: view.yaw,
            roll: view.roll,
            pitch_extent: view.collision.map(|shape| shape.half_width),
            roll_extent: view.collision.map(|shape| shape.half_depth),
            attacker: view.relations.attacker,
            target: view.relations.target,
            stage_index: view.relations.stage_index,
            stage_mode: view.relations.stage_mode,
            parent: view.relations.parent,
            stage_relation: view.relations.stage_relation,
            next: view.relations.next,
            castle_slot: arena
                .economy
                .castle_for_owner(slot)
                .map_or(0, |castle| castle.source_entity_index),
            ..Target::default()
        }),
    )
}

pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    phase: u8,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    super::construct_row(slot, seed, position, phase, particles)
}

pub(crate) fn bind_authored_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    super::stage::bind_authored(arena, runtime, slot, index)
}

/// Reconcile model25 StageVar references before the source callback pass.
pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slots: &[u16],
    target_context: crate::native_world::source_target::Context<'_>,
) -> Result<(), String> {
    super::super::stage_route::prepare_source_pass(arena, runtime, target_context, slots)
}

/// Ordinary `sub_4CE00` authored producer. A failed allocation leaves the
/// source model counter and route table untouched.
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
    particles
        .get(usize::from(PARTICLE))
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| "Cymmerian requires retained particle290".to_owned())?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let phase_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
    let phase = *runtime
        .bytes
        .get(phase_at)
        .ok_or("Cymmerian source constructor counter table is absent")?;
    let mut row = construct_row(slot, arena.new_event_rng(slot), position, phase, particles)?;
    row.bind_frame_zero_max_life(max_life);
    runtime.bytes[phase_at] = phase.wrapping_add(1);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if let Some(index) = stage_index {
        bind_authored_route(arena, runtime, slot, index)?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Cymmerian constructor lost slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Cymmerian,
        CreatureRootOrigin::AuthoredRecord { authored_record },
    ) {
        return Err(format!(
            "Cymmerian authored record{authored_record} lost generation-bound origin"
        ));
    }
    Ok(Some(slot))
}

/// Source `sub_3A5B0`'s ordinary class-5 summon producer. The ring callback
/// owns its lifetime; this adapter constructs a descriptor-92 Cymmerian and
/// binds the ring holder as its generation-bound spell origin.
pub(crate) fn allocate_summoned(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    parent_slot: u16,
    owner_id: u16,
    holder_slot: u16,
) -> Result<Option<SourceEntityHandle>, String> {
    particles
        .get(usize::from(PARTICLE))
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| "Cymmerian Army summon requires retained particle290".to_owned())?;

    let counter_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
    let phase = *runtime
        .bytes
        .get(counter_at)
        .ok_or("Cymmerian source constructor counter table is absent")?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };

    let mut row = construct_row(slot, arena.new_event_rng(slot), position, phase, particles)?;
    row.set_source_action(8 * SOURCE_MODEL + 7);
    row.record[77] = crate::native_world::summon_army::SUMMONED_STAGE_MODE;
    put(&mut row, 44, parent_slot);
    let remaining_lifetime = crate::native_world::summon_army::SUMMONED_REMAINING_LIFETIME;
    put(&mut row, 50, remaining_lifetime);
    row.active_value = i16::from_le_bytes(remaining_lifetime.to_le_bytes());
    row.record[154..156].fill(0);
    row.set_mana(0);
    row.max_mana = 0;
    row.mana_cost = 0;
    row.record[144..148].fill(0);
    row.source_id = owner_id;
    put(&mut row, 30, owner_id);

    // Source array_0x10[25] is a byte counter and increments modulo 256.
    runtime.bytes[counter_at] = phase.wrapping_add(1);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;

    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Cymmerian Army summon lost allocated slot{slot}"))?;
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Cymmerian,
        CreatureRootOrigin::SummonedSpell { holder_slot },
    ) {
        return Err(format!(
            "Cymmerian Army summon at slot{slot} lost generation-bound origin"
        ));
    }
    if let Some(parent) = arena.handle(parent_slot).filter(|parent| *parent != handle) {
        if !arena.bind_creature_parent(handle, parent) {
            return Err(format!(
                "Cymmerian Army summon at slot{slot} lost its parent binding"
            ));
        }
    }
    Ok(Some(handle))
}

fn launch_attack(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    attack: super::SourceAttack,
) -> Result<Option<u16>, String> {
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let payload =
        crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload {
            origin: crate::native_world::FireballProjectileOrigin::Cymmerian,
            position: attack.constructor_position,
            yaw: attack.yaw,
            pitch: attack.pitch,
            axis_position: Position::default(),
            source_id_entity_index: attack.owner_id,
            holder_entity_index: 0,
            source_subspell_index: attack.parameter,
            source_mana: 0,
            source_dword_0x10: 0,
            target_source_entity_index: attack.target,
            target_class: attack.target_class,
            target_model: attack.target_model,
        };
    if arena
        .economy
        .initialize_fireball_projectile(slot, payload, 0)
        .is_none()
    {
        return Err(format!(
            "Cymmerian Fireball map linkage failed at allocated slot{slot}"
        ));
    }
    crate::world::effects::sync_position(
        arena,
        terrain,
        slot,
        attack.constructor_position,
        true,
        false,
    )?;
    crate::world::effects::lighting::admit_source(
        arena,
        terrain,
        slot,
        attack.constructor_position,
        crate::world::effects::lighting::SourceLight::Fireball,
    );
    arena
        .economy
        .fireball_projectiles
        .iter_mut()
        .find(|projectile| projectile.active && projectile.source_entity_index == slot)
        .ok_or_else(|| format!("Cymmerian Fireball lost allocated slot{slot}"))?
        .position = attack.position;
    let _ = crate::native_world::Level2EconomyState::apply_creature_fireball_target_feedback(
        &mut arena.entities,
        attack.target,
    );
    Ok(Some(slot))
}

fn apply_castle_contact(
    arena: &mut SourceArena,
    source_id: u16,
    hit: super::CastleHit,
) -> Result<(), String> {
    let damage = u16::try_from(hit.damage).map_err(|_| {
        format!(
            "Cymmerian castle damage {} is outside source word range",
            hit.damage
        )
    })?;
    if arena
        .economy
        .apply_source_castle_contact(hit.castle_slot, source_id, damage)
    {
        Ok(())
    } else {
        Err(format!(
            "Cymmerian castle contact lost castle slot{}",
            hit.castle_slot
        ))
    }
}

fn child_row(
    slot: u16,
    seed: u16,
    position: Position,
    phase: u8,
    parent_owner: u16,
    mana: i32,
    particles: &[ParticleParameter],
) -> Result<EntitySnapshot, String> {
    let particle = particles
        .get(usize::from(SPLIT_PARTICLE))
        .ok_or("Cymmerian split requires retained particle314")?;
    if particle.animation_limit().is_none() {
        return Err("Cymmerian split particle314 has invalid animation mode".to_owned());
    }
    let mut row = crate::native_world::cave_features::new_event(slot, seed, position);
    row.class = SOURCE_CLASS;
    row.model = SOURCE_MODEL;
    row.record[67] = SOURCE_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.record[69] = 3;
    row.record[60] = 1;
    row.record[61] = 64;
    row.record[74] = 3;
    row.set_source_action(200);
    row.set_max_life(RULES.split_max_life);
    row.set_life(RULES.split_max_life);
    row.set_mana(mana);
    row.set_actual_speed(
        i16::from(RULES.split_minimum_speed).saturating_add(RULES.split_speed_bonus),
    );
    put(&mut row, 136, RULES.split_minimum_speed as u16);
    put(&mut row, 138, RULES.split_maximum_speed as u16);
    row.set_subspell_index(RULES.split_attack_damage);
    let rng = crate::native_world::level2_next_rng16(seed);
    row.set_entity_rng(rng);
    let angle = (rng & 2047).wrapping_sub(1) as i16;
    row.set_yaw(angle);
    row.set_source_roll(angle);
    row.set_phase(phase);
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(SPLIT_DESCRIPTOR) + 1).to_le_bytes());
    crate::world::effects::bind_particle(&mut row, SPLIT_PARTICLE, particles)?;
    row.set_source_overlap_half_extents(RULES.split_half_extent, RULES.split_half_extent);
    put(&mut row, 92, RULES.split_half_extent as u16);
    put(&mut row, 40, parent_owner);
    // The source leaves byte46 at 1 only after all three allocations finish.
    row.record[74] = 1;
    Ok(row)
}

fn split(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parent_slot: u16,
) -> Result<Vec<u16>, String> {
    particles
        .get(usize::from(SPLIT_PARTICLE))
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| "Cymmerian split requires valid retained particle314".to_owned())?;
    let parent = arena.entities[usize::from(parent_slot)].clone();
    let share = parent.mana / i32::from(RULES.split_count);
    let mut remainder = parent.mana;
    let mut last_attempt = None;
    let mut spawned = Vec::with_capacity(usize::from(RULES.split_count));
    for _ in 0..RULES.split_count {
        let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
            last_attempt = None;
            continue;
        };
        let phase_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
        let phase = *runtime
            .bytes
            .get(phase_at)
            .ok_or("Cymmerian source constructor counter table is absent")?;
        let row = child_row(
            slot,
            arena.new_event_rng(slot),
            parent.position,
            phase,
            word(&parent, 42),
            share,
            particles,
        )?;
        runtime.bytes[phase_at] = phase.wrapping_add(1);
        let counter = row.entity_rng;
        arena.entities[usize::from(slot)] = row;
        arena.economy.source_rng.insert(slot, counter);
        crate::world::effects::sync_position(arena, terrain, slot, parent.position, true, false)?;
        let handle = arena
            .handle(slot)
            .ok_or_else(|| format!("Cymmerian split lost allocated slot{slot}"))?;
        if !arena.bind_raw_creature_origin(
            handle,
            crate::level_definition::CreatureKind::Cymmerian,
            CreatureRootOrigin::CreatureConversion {
                producer_slot: parent_slot,
            },
        ) {
            return Err(format!(
                "Cymmerian split child at slot{slot} lost generation-bound origin"
            ));
        }
        remainder = remainder.wrapping_sub(share);
        last_attempt = Some(slot);
        spawned.push(slot);
    }
    // Source adds the remainder only to the third attempted allocation when
    // that final `NewEvent` succeeded. A failed final attempt drops the rest.
    if let Some(slot) = last_attempt {
        arena.entities[usize::from(slot)].set_mana(
            arena.entities[usize::from(slot)]
                .mana
                .wrapping_add(remainder),
        );
    }
    Ok(spawned)
}

/// Runs one model25 action and its immediate source-owned constructors. The
/// shared dispatcher still owns the source action table and pass tail.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    target_context: crate::native_world::source_target::Context<'_>,
) -> Result<Effects, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("Cymmerian callback lost slot{slot}"))?
        .clone();
    let source_id = row.source_id;
    let parent_slot = word(&row, 44);
    let parent_current = row.record[77] == 14 || arena.handle(slot).is_some_and(|child| {
        // Imported reset rows have generation zero and predate the runtime
        // association. Every allocated Army or boss child binds a parent.
        if arena.has_creature_parent_binding(child) {
            arena
                .creature_parent(child)
                .is_some_and(|parent| parent.source_slot == parent_slot)
        } else {
            child.generation == 0
        }
    });
    let mut context = Context {
        terrain,
        targets: &[],
        class3,
        model25: &families[usize::from(SOURCE_MODEL)],
        reader: Some((arena, target_context)),
        read_error: std::cell::Cell::new(None),
    };
    let mut effects = super::step(&mut Actor { row: &mut row }, &mut context)?;
    let stage_expiry = if effects.stage_callback {
        effects.stage_callback = false;
        super::stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            families,
            parent_current,
            &mut effects,
        )?
    } else {
        None
    };
    if let Some(slot) = context.read_error.get() {
        return Err(format!(
            "Cymmerian cannot read current source target slot{slot}"
        ));
    }
    drop(context);
    let position = row.position;
    if let Some(visual) = effects.visual {
        crate::world::effects::bind_particle(&mut row, visual, particles)?;
    }
    // sub_28CE0 keeps action204 while it allocates children and its burst;
    // the source pointer receives action205 only after those constructors.
    if effects.split {
        row.set_source_action(204);
    }
    row.record[66] = row.phase;
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    for (linked_slot, flags) in &effects.linked_blink_flags {
        if let Some(linked) = arena
            .entities
            .get_mut(usize::from(*linked_slot))
            .filter(|row| row.active)
        {
            linked.set_control_flags((linked.control_flags & !0x0600_0000) | *flags);
        }
    }

    if !effects.split {
        for _ in 0..effects.population_increments {
            arena.economy.increment_source_creature_population();
        }
    }
    for _ in 0..effects.population_decrements {
        arena.economy.decrement_source_creature_population();
    }
    if effects.prekill {
        effects.kill_credit =
            crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 200)?;
        effects.prekill = false;
    }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;

    if let Some(summoned) = stage_expiry {
        super::super::creature_24::mc2::expire(arena, terrain, particles, slot, summoned)?;
    }

    if effects.split {
        if arena.economy.free_slot_count() <= 1 {
            let parent = &arena.entities[usize::from(slot)];
            let position = parent.position;
            let yaw = parent.yaw;
            let mana = parent.mana;
            let owner = parent.source_player_entity_index();
            if mana > 0 {
                let rng = crate::native_world::level2_next_rng16(parent.entity_rng);
                arena.entities[usize::from(slot)].set_entity_rng(rng);
                arena.economy.source_rng.insert(slot, rng);
                crate::world::effects::spawn_death_mana_sphere(
                    arena, terrain, slot, position, yaw, mana, owner,
                )?;
                if !crate::world::effects::clear_replacement_mana_owner(arena, slot)? {
                    arena.entities[usize::from(slot)].set_source_player_entity_index(0);
                }
            }
        } else {
            let _ = split(arena, runtime, terrain, particles, slot)?;
        }
        let current = crate::native_world::source_target::read(
            arena,
            crate::native_world::source_target::Context::default(),
            slot,
        )?
        .ok_or_else(|| format!("Cymmerian split lost source address {slot}"))?;
        crate::world::effects::spawn_death_burst(
            arena,
            terrain,
            particles,
            slot,
            current.position,
            current.source_id,
        )?;
        // NewEvent may replace the fixed source address during mana birth.
        // The source's post-constructor write then belongs to that sphere,
        // not to the stale raw actor. Other typed occupants require their own
        // authoritative writer instead of silently updating a raw shadow.
        if !arena.economy.set_mana_sphere_action(slot, 205) {
            if arena.economy.live_arena_cursor_row(slot).is_some() {
                return Err(format!(
                    "Cymmerian split source address {slot} has an unsupported typed replacement"
                ));
            }
            arena.entities[usize::from(slot)].set_source_action(205);
        }
        for _ in 0..effects.population_increments {
            arena.economy.increment_source_creature_population();
        }
    }

    if effects.kill {
        kill_summoned_child(arena, terrain, particles, slot)?;
    }
    if let Some(attack) = effects.attack.take() {
        let _ = launch_attack(arena, terrain, attack)?;
    }
    if let Some(hit) = effects.castle_hit.take() {
        apply_castle_contact(arena, source_id, hit)?;
    }
    Ok(effects)
}

fn kill_summoned_child(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<(), String> {
    let Some(row) = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == SOURCE_CLASS && row.model == SOURCE_MODEL)
        .cloned()
    else {
        return Ok(());
    };
    if row.phase & 7 != 0 {
        return Ok(());
    }
    if row.mana > 0 {
        arena.entities[usize::from(slot)]
            .set_entity_rng(crate::native_world::level2_next_rng16(row.entity_rng));
        arena
            .economy
            .source_rng
            .insert(slot, arena.entities[usize::from(slot)].entity_rng);
        crate::world::effects::spawn_death_mana_sphere(
            arena,
            terrain,
            slot,
            row.position,
            row.yaw,
            row.mana,
            row.source_player_entity_index(),
        )?;
        if !crate::world::effects::clear_replacement_mana_owner(arena, slot)? {
            arena.entities[usize::from(slot)].set_source_player_entity_index(0);
        }
    }
    let current = crate::native_world::source_target::read(
        arena,
        crate::native_world::source_target::Context::default(),
        slot,
    )?
    .ok_or_else(|| format!("Cymmerian death lost source address {slot}"))?;
    let burst = if current.control_flags & 0x10_0000 == 0 {
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
        arena.entities[usize::from(slot)].disable_drawing();
    }
    Ok(())
}
