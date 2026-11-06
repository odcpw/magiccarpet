//! Source-row constructor and lossless MC2 adapter for class5/model10.

use super::{
    Actor, Effects, Frame, SOURCE_DESCRIPTOR, SOURCE_FEATURE_MASK, SOURCE_INITIAL_ACTION,
    SOURCE_INITIAL_LIFE, SOURCE_INITIAL_MANA, SOURCE_MODEL, SOURCE_PARTICLE, SummonKind,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncounterAction {
    Approach(u8),
    Refresh,
    KillCreatures { player: u16 },
}

type ApplyEncounterAction<'a> =
    dyn FnMut(&mut SourceArena, EncounterAction) -> Result<(), String> + 'a;

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn particle_animation_limit(particles: &[ParticleParameter]) -> Result<u8, String> {
    particles
        .get(usize::from(SOURCE_PARTICLE))
        .ok_or_else(|| "final encounter requires retained particle341".to_owned())?
        .animation_limit()
        .ok_or_else(|| "final encounter particle341 has invalid animation mode".to_owned())
}

pub(crate) fn load(row: &EntitySnapshot) -> Option<Actor> {
    (row.active && row.class == 5 && row.model == SOURCE_MODEL && row.record.len() >= 176).then(
        || Actor {
            slot: row.index,
            source_id: row.source_id,
            action: row.action,
            // byte_0x46_70 is four bytes into the exported source record.
            phase: row.record[74],
            flags: row.control_flags,
            damage_enabled: row.record[61] != 0,
            position: row.position,
            rng: row.entity_rng,
            yaw: row.yaw,
            roll: word(row, 36) as i16,
            life: row.life,
            max_life: row.max_life,
            timer: row.source_dword_10(),
            attack_remaining: word(row, 40),
            source_animation: word(row, 48),
            subspell_index: row.subspell_index,
            summon_mode: row.record[71],
            summon_variant: row.record[72],
            summon_angle_step: word(row, 78),
            target: word(row, 154),
            visual: word(row, 94),
            animation_frame: row.record[96],
            animation_limit: row.record[97],
            current_attacker: word(row, 42),
            pending_damage: i32::from_le_bytes(
                row.record[98..102].try_into().expect("fixed entity record"),
            ),
            pending_attacker: word(row, 102),
            source_counter: row.record[66],
        },
    )
}

fn store(row: &mut EntitySnapshot, actor: Actor) {
    row.set_source_action(actor.action);
    row.set_control_flags(actor.flags);
    row.set_position(actor.position);
    row.set_entity_rng(actor.rng);
    row.set_yaw(actor.yaw);
    row.set_source_roll(actor.roll);
    row.set_life(actor.life);
    row.set_source_dword_10(actor.timer);
    row.record[40..42].copy_from_slice(&actor.attack_remaining.to_le_bytes());
    put_word(row, 48, actor.source_animation);
    row.set_subspell_index(actor.subspell_index);
    row.set_source_byte_46(actor.phase);
    row.record[71] = actor.summon_mode;
    row.record[72] = actor.summon_variant;
    put_word(row, 78, actor.summon_angle_step);
    put_word(row, 154, actor.target);
    put_word(row, 94, actor.visual);
    row.particle_parameter_index = Some(actor.visual);
    row.record[96] = actor.animation_frame;
    row.record[97] = actor.animation_limit;
    put_word(row, 42, actor.current_attacker);
    row.record[98..102].copy_from_slice(&actor.pending_damage.to_le_bytes());
    put_word(row, 102, actor.pending_attacker);
    row.set_phase(actor.source_counter);
}

fn materialize_source_effects(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    effects: &mut Effects,
) -> Result<(), String> {
    for request in effects.source_effects.drain(..) {
        match (request.source_type, request.source_subtype) {
            (10, 17) => {
                let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
                    continue;
                };
                let source = arena.economy.initialize_boss_meteor(
                    slot,
                    request.position,
                    request.owner_source_id,
                );
                crate::world::effects::sync_position(
                    arena,
                    terrain,
                    slot,
                    source.position,
                    true,
                    false,
                )?;
            }
            (10, 9) => {
                effects.final_death_reward_slot =
                    crate::native_world::final_volcano::materialize_boss_death_reward(
                        arena,
                        terrain,
                        runtime,
                        &mut effects.final_death_reward_pending,
                        request.position,
                        request.owner_source_id,
                    )?;
                // Source phase15 consumes its single constructor attempt even
                // when the arena returns None; it clears word36548 and hides
                // the boss unconditionally after this call.
                effects.final_death_reward_pending = false;
                effects.final_death_reward_attempted = true;
                if effects.final_death_reward_slot.is_none() {
                    continue;
                }
            }
            selector => {
                return Err(format!(
                    "unsupported final encounter source effect{selector:?}"
                ));
            }
        }
    }
    Ok(())
}

/// Materialize the model21 StageVar child through its canonical family
/// constructor.
fn materialize_model21_summon(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    request: super::SummonRequest,
) -> Result<Option<u8>, String> {
    if !matches!(
        (request.source_type, request.source_subtype, request.action),
        (5, 0, 175)
    ) {
        return Err("final encounter model21 request has an invalid source selector".to_owned());
    }
    let parent_slot = request
        .parent_slot
        .ok_or("final encounter model21 request has no boss parent slot")?;
    let player_slot = request
        .target_player_slot
        .ok_or("final encounter model21 request has no player target slot")?;
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("final encounter model21 parent slot{parent_slot} is absent"))?;
    if parent.source_id != request.owner {
        return Err("final encounter model21 owner differs from its boss parent".to_owned());
    }
    let parent_handle = arena
        .handle(parent_slot)
        .ok_or("final encounter model21 parent lost its source generation")?;

    let Some(slot) = crate::native_world::creatures::creature_21::mc2::allocate_summoned(
        arena,
        runtime,
        terrain,
        particles,
        request.position,
        parent_slot,
        player_slot,
        request.yaw.ok_or("final encounter model21 request lacks its captured launch yaw")?,
    )?
    else {
        return Ok(None);
    };

    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == 21)
        .ok_or_else(|| format!("final encounter model21 constructor lost slot{slot}"))?;
    row.set_source_action(request.action);
    if let Some(stage) = request.source_stage_variable {
        row.record[77] = stage;
    }
    if let Some(timer) = request.source_timer {
        row.record[50..52].copy_from_slice(&timer.to_le_bytes());
    }
    if let Some(speed) = request.actual_speed {
        row.set_actual_speed(speed);
    }
    if let Some(yaw) = request.yaw {
        row.set_yaw(yaw);
    }
    if let Some(pitch) = request.pitch {
        row.set_pitch(pitch);
    }
    if let Some(roll) = request.roll {
        row.set_source_roll(roll);
    }
    let child_handle = arena
        .handle(slot)
        .ok_or("final encounter model21 child lost its source generation")?;
    if !arena.bind_carried_creature_parent(child_handle, parent_handle) {
        return Err(
            "final encounter model21 could not carry its boss parent generation".to_owned(),
        );
    }
    Ok(request.success_sound)
}

fn summon_parent_and_player(
    arena: &SourceArena,
    request: &super::SummonRequest,
) -> Result<(u16, u16), String> {
    let parent_slot = request
        .parent_slot
        .ok_or("final encounter child request has no boss parent slot")?;
    let player_slot = request
        .target_player_slot
        .ok_or("final encounter child request has no player target slot")?;
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("final encounter child parent slot{parent_slot} is absent"))?;
    if parent.source_id != request.owner {
        return Err("final encounter child owner differs from its boss parent".to_owned());
    }
    Ok((parent_slot, player_slot))
}

fn validate_stage_child_request(
    request: &super::SummonRequest,
    expected_action: u8,
) -> Result<(u16, u16, u8, u16, i16), String> {
    if !matches!((request.source_type, request.source_subtype), (5, 0))
        || request.action != expected_action
    {
        return Err("final encounter child request has an invalid source selector".to_owned());
    }
    let stage = request
        .source_stage_variable
        .filter(|stage| *stage == 17)
        .ok_or("final encounter child request is missing source StageVar17")?;
    let timer = request
        .source_timer
        .filter(|timer| *timer == 250)
        .ok_or("final encounter child request is missing source lifetime250")?;
    let speed = request
        .actual_speed
        .filter(|speed| *speed == 320)
        .ok_or("final encounter child request is missing source speed320")?;
    let parent_slot = request
        .parent_slot
        .ok_or("final encounter child request has no boss parent slot")?;
    let player_slot = request
        .target_player_slot
        .ok_or("final encounter child request has no player target slot")?;
    Ok((parent_slot, player_slot, stage, timer, speed))
}

fn materialize_model25_summon(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    request: super::SummonRequest,
) -> Result<Option<u8>, String> {
    if request.success_sound != Some(37) {
        return Err("final encounter model25 request has an invalid success sound".to_owned());
    }
    let (parent_slot, player_slot, stage, timer, speed) =
        validate_stage_child_request(&request, 207)?;
    summon_parent_and_player(arena, &request)?;
    let parent_handle = arena
        .handle(parent_slot)
        .ok_or("final encounter model25 parent lost its source generation")?;

    // The family allocator owns the real source sub_4CE00 constructor,
    // model25 counter, RNG, map link, and failure behavior. Its Army-specific
    // StageVar/origin writes are replaced below with the boss's source writes.
    let Some(handle) = crate::native_world::creatures::creature_25::mc2::allocate_summoned(
        arena,
        runtime,
        terrain,
        particles,
        request.position,
        parent_slot,
        request.owner,
        parent_slot,
    )?
    else {
        return Ok(None);
    };
    let slot = handle.source_slot;
    if !arena.bind_carried_creature_parent(handle, parent_handle) {
        return Err(
            "final encounter model25 could not carry its boss parent generation".to_owned(),
        );
    }
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == 25)
        .ok_or_else(|| format!("final encounter model25 constructor lost slot{slot}"))?;
    row.source_id = request.owner;
    row.record[30..32].copy_from_slice(&request.owner.to_le_bytes());
    row.set_source_action(request.action);
    row.record[77] = stage;
    put_word(row, 44, parent_slot);
    put_word(row, 50, timer);
    put_word(row, 154, player_slot);
    row.set_actual_speed(speed);
    if let Some(yaw) = request.yaw {
        row.set_yaw(yaw);
    }
    if let Some(pitch) = request.pitch {
        row.set_pitch(pitch);
    }
    if let Some(roll) = request.roll {
        row.set_source_roll(roll);
    }
    if !arena.bind_raw_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Cymmerian,
        crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion {
            producer_slot: parent_slot,
        },
    ) {
        return Err(format!(
            "final encounter model25 child at slot{slot} lost boss generation origin"
        ));
    }
    Ok(request.success_sound)
}

fn materialize_firefly_summon(
    arena: &mut SourceArena,
    _runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    request: super::SummonRequest,
) -> Result<Option<u8>, String> {
    if request.success_sound != Some(44) {
        return Err("final encounter Firefly request has an invalid success sound".to_owned());
    }
    let (parent_slot, player_slot, stage, timer, speed) =
        validate_stage_child_request(&request, 159)?;
    summon_parent_and_player(arena, &request)?;
    let parent_handle = arena
        .handle(parent_slot)
        .ok_or("final encounter Firefly parent lost its source generation")?;

    // The boss invokes the ordinary Firefly constructor, then applies its
    // own stage and launch fields without the Army's zero-mana override.
    let Some(handle) = crate::native_world::level2_class5::firefly::allocate_source_default(
        arena,
        terrain,
        particles,
        request.position,
    )?
    else {
        return Ok(None);
    };
    let slot = handle.source_slot;
    let source = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|source| source.active && source.source_entity_index == slot && source.model == 19)
        .ok_or_else(|| format!("final encounter Firefly constructor lost slot{slot}"))?;
    source.position = request.position;
    source.action = request.action;
    source.word_0x96_150 = player_slot;
    source.actual_speed = speed;
    if let Some(yaw) = request.yaw {
        source.yaw = yaw;
    }
    if let Some(pitch) = request.pitch {
        source.pitch = pitch;
    }
    if let Some(roll) = request.roll {
        source.roll = roll;
    }
    let state = arena
        .economy
        .authored_fireflies
        .iter_mut()
        .find(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("final encounter Firefly runtime lost slot{slot}"))?;
    state.parent_entity_index = parent_slot;
    state.parent_generation = parent_handle.generation;
    state.entity_id = request.owner;
    state.word_0x2e = timer;
    state.stage_var1 = 0;
    state.stage_var2 = stage;
    if !arena.bind_dynamic_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Firefly,
        crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion {
            producer_slot: parent_slot,
        },
    ) {
        return Err(format!(
            "final encounter Firefly child at slot{slot} lost boss generation origin"
        ));
    }
    if !arena.bind_carried_creature_parent(handle, parent_handle) {
        return Err(
            "final encounter Firefly could not carry its boss parent generation".to_owned(),
        );
    }
    Ok(request.success_sound)
}

fn materialize_summons(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    effects: &mut Effects,
) -> Result<(), String> {
    for request in effects.summons.drain(..) {
        let sound = match request.kind {
            SummonKind::Model21 => {
                materialize_model21_summon(arena, runtime, terrain, particles, request)?
            }
            SummonKind::Model25 => {
                materialize_model25_summon(arena, runtime, terrain, particles, request)?
            }
            SummonKind::Firefly => {
                materialize_firefly_summon(arena, runtime, terrain, particles, request)?
            }
            SummonKind::Dragon => {
                let (parent_slot, player_slot, _, _, _) =
                    validate_stage_child_request(&request, 7)?;
                summon_parent_and_player(arena, &request)?;
                let parent = arena
                    .handle(parent_slot)
                    .ok_or("final encounter Dragon parent lost its arena generation")?;
                crate::native_world::level2_class5::dragon::allocate_boss_summoned(
                    arena,
                    terrain,
                    request.position,
                    parent,
                    request.owner,
                    player_slot,
                    request.yaw.unwrap_or(0),
                )?
                .and(request.success_sound)
            }
            SummonKind::Class9Projectile { subtype } => {
                use crate::native_world::guided_projectile::boss::{self, Launch, Product};
                let product = match subtype {
                    0 => Product::FinalBossFireball,
                    9 => Product::Lightning,
                    26 => Product::Whirlwind,
                    3 => Product::Meteor,
                    _ => {
                        return Err(format!(
                            "final encounter projectile selector9/{subtype} is invalid"
                        ));
                    }
                };
                boss::construct(
                    arena,
                    terrain,
                    particles,
                    Launch {
                        product,
                        position: request.position,
                        owner: request.owner,
                        yaw: request.yaw.unwrap_or(0),
                        pitch: request.pitch.unwrap_or(0),
                        target_class: request.target_player_class.unwrap_or(0),
                        target_model: request.target_player_model.unwrap_or(0),
                        target: 0,
                        damage: request.subspell_index.unwrap_or(0),
                        effect_life: request.action,
                        descriptor: request.descriptor,
                    },
                )?
                .and(request.success_sound)
            }
        };
        if let Some(sound) = sound {
            effects.sounds.push(sound);
        }
    }
    Ok(())
}

/// Return the exact SEARCH.DAT bitmap class sequence consumed by `sub_10130`.
/// `source_search_offsets` is cumulative and class-major, so subtracting the
/// preceding prefix yields one bitmap's row-major coordinates. The source
/// loop accepts only return value 1; its final return value 2 means the last
/// emitted pair is deliberately omitted here.
fn source_bitmap_class_offsets(class: u8) -> Result<Vec<(i8, i8)>, String> {
    if class > 15 {
        return Err(format!(
            "final encounter SEARCH.DAT class{class} is invalid"
        ));
    }
    let mut offsets = crate::level1_mission::source_search_offsets(class);
    let prefix = if class == 0 {
        0
    } else {
        crate::level1_mission::source_search_offsets(class - 1).len()
    };
    if prefix > offsets.len() {
        return Err(format!(
            "final encounter SEARCH.DAT class{class} prefix is malformed"
        ));
    }
    offsets.drain(..prefix);
    offsets.pop();
    Ok(offsets)
}

fn source_tile_center(position: Position) -> (i16, i16) {
    (
        i16::from(((u32::from(position.x) + 128) >> 8) as u8),
        i16::from(((u32::from(position.y) + 128) >> 8) as u8),
    )
}

/// Source `sub_21490`, called once in the phase-0 pass and once on every
/// phase-1 pass. Its completion result is derived from the live source state;
/// it is never supplied as a Frame success flag.
fn approach_step(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    actor: &mut Actor,
    frame: Frame,
    effects: &mut Effects,
    apply_world: &mut ApplyEncounterAction<'_>,
) -> Result<bool, String> {
    let flags = actor.subspell_index;
    let mut continue_mana_effects = true;
    let mut creature_mode = 0;
    let mut close = false;

    if flags & 8 != 0 {
        let (center_x, center_y) = source_tile_center(actor.position);
        if actor.timer < 0 {
            // sub_10130's final return=2 pair is not visited by the source
            // `while (... == 1)` and is therefore absent from this scan.
            let offsets = source_bitmap_class_offsets(7)?;
            let blocked = offsets.into_iter().any(|(dx, dy)| {
                let x = center_x.wrapping_add(i16::from(dx)) as u8;
                let y = center_y.wrapping_add(i16::from(dy)) as u8;
                let tile = usize::from(x) + 256 * usize::from(y);
                terrain.planes().terrain_types[tile] != 0
            });
            if !blocked {
                actor.timer = 70;
                actor.subspell_index = (actor.subspell_index | 4) & !8;
            } else {
                actor.timer = 15;
            }
        } else {
            effects.sounds.push(10);
            let class = u8::try_from(15_i32.wrapping_sub(actor.timer))
                .ok()
                .filter(|class| *class <= 15)
                .ok_or_else(|| {
                    format!(
                        "final encounter approach timer{} selects an invalid SEARCH.DAT class",
                        actor.timer
                    )
                })?;
            for (dx, dy) in source_bitmap_class_offsets(class)? {
                let x = center_x.wrapping_add(i16::from(dx));
                let y = center_y.wrapping_add(i16::from(dy));
                let _ = crate::world::terrain::disasters::relative_height(terrain, x, y, -1, false);
            }
            actor.timer = actor.timer.wrapping_sub(1);
        }
    } else if flags & 4 != 0 {
        effects.kill_all_creatures = true;
        apply_world(
            arena,
            EncounterAction::KillCreatures {
                player: frame.player_slot,
            },
        )?;
        let old_timer = actor.timer;
        actor.timer = actor.timer.wrapping_sub(1);
        if old_timer < 17 {
            if old_timer == 0 {
                actor.subspell_index = (actor.subspell_index | 0x10) & !4;
                actor.timer = 1;
                actor.flags &= !1;
            } else if old_timer == 1 {
                creature_mode = 3;
            }
        } else if old_timer == 17 {
            creature_mode = 2;
        } else if old_timer == 35 {
            creature_mode = 1;
        } else if old_timer == 70 {
            effects.clear_stage_variable_count = true;
        }
    } else if flags & 0x10 != 0 {
        if actor.timer == 1 {
            actor.timer = 0;
            actor.subspell_index &= !0x40;
        } else if flags & 0x40 != 0 {
            if crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
                frame.player_position,
                actor.position,
            )) >= 0x0a00
            {
                actor.subspell_index &= !0x40;
            } else {
                actor.timer = 30;
                actor.subspell_index = (actor.subspell_index | 0x20) & !0x10;
                actor.flags &= !0x0080_0000;
            }
        }
    } else if flags & 0x20 != 0 {
        if actor.timer >= 600 {
            continue_mana_effects = false;
        }
        actor.timer = actor.timer.wrapping_add(30);
        if actor.timer >= 1200 {
            actor.timer = 1200;
            actor.subspell_index &= !0x20;
        }
        let progress = i16::try_from(actor.timer)
            .map_err(|_| "final encounter progress exceeds its source word".to_owned())?;
        let bytes = runtime
            .bytes
            .get_mut(0x87a..0x87c)
            .ok_or_else(|| "final encounter progress word0x87a is absent".to_owned())?;
        bytes.copy_from_slice(&progress.to_le_bytes());
    } else {
        // sub_21490's close result is true only in the no-mode branch.
        close = true;
    }

    if creature_mode != 0 {
        // Source v29 mutates only the frozen dword38523 chain (class10
        // models39/40/57). The world owner consumes this exact-list intent;
        // neither the player nor the boss is part of that chain.
        effects.approach_creature_mode = Some(creature_mode);
        apply_world(arena, EncounterAction::Approach(creature_mode))?;
    }

    if continue_mana_effects {
        actor.attack_remaining = actor.attack_remaining.wrapping_add(96) & 0x07ff;
        for index in 0..4_u16 {
            let heading = actor.attack_remaining.wrapping_add(index * 512) & 0x07ff;
            let mut position = actor.position;
            crate::move_entity(&mut position, heading as i16, 0, 192);
            let Some(slot) = arena
                .economy
                .allocate_model13_spawned_child(0, 0, position, 0)
            else {
                continue;
            };
            actor.rng = actor.rng.wrapping_mul(9_377).wrapping_add(9_439);
            if let Some(child) = arena
                .economy
                .model13_spawned_children
                .iter_mut()
                .find(|child| child.active && child.source_entity_index == slot)
            {
                child.life = i16::try_from((actor.rng & 7) + 8)
                    .expect("final encounter effect lifetime fits i16");
            } else {
                return Err(format!(
                    "final encounter 10/14 constructor lost source slot{slot}"
                ));
            }
        }
    }

    Ok(close)
}

/// Exact `sub_4BD00` row defaults. The required feature bit is checked before
/// slot allocation; a rejected mission or full source arena creates no boss.
pub(crate) fn construct_row(
    slot: u16,
    seed: u16,
    position: Position,
    feature_byte: u8,
    source_counter: u8,
    terrain: &SessionTerrain,
    particles: &[ParticleParameter],
) -> Result<Option<EntitySnapshot>, String> {
    if feature_byte & SOURCE_FEATURE_MASK == 0 {
        return Ok(None);
    }
    let animation_limit = particle_animation_limit(particles)?;

    let mut row = crate::world::terrain::cave_features::new_event(slot, seed, position);
    row.class = 5;
    row.model = SOURCE_MODEL;
    row.record[67] = 5;
    row.record[68] = SOURCE_MODEL;
    row.set_source_action(SOURCE_INITIAL_ACTION);
    row.record[60] = 1;
    row.record[61] = 64;
    row.set_phase(source_counter);
    row.record[69] = 3;
    row.set_control_flags(row.control_flags | 0x4880_0001);
    row.set_subspell_index(0);
    row.set_source_dword_10(0);
    row.set_source_byte_46(0);
    row.bind_frame_zero_max_life(SOURCE_INITIAL_LIFE);
    row.set_mana(SOURCE_INITIAL_MANA);
    row.set_position(Position {
        z: terrain.ground_height(position),
        ..position
    });
    // sub_4BD00's array yaw and SetEntityShiftRot(1024,1280). The source
    // entity FOV is separate and stays at NewEvent's default.
    put_word(&mut row, 86, 512);
    put_word(&mut row, 88, 1024);
    put_word(&mut row, 90, 1024);
    put_word(&mut row, 92, 1280);
    put_word(&mut row, 94, SOURCE_PARTICLE);
    row.record[97] = animation_limit;
    row.particle_parameter_index = Some(SOURCE_PARTICLE);
    row.parameter_relation = Some(SOURCE_DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(SOURCE_DESCRIPTOR) + 1).to_le_bytes());
    Ok(Some(row))
}

/// Final Level25/internal24 authored record379 (`type5/subtype10`) producer.
/// Eruption91 is a different class10/model10 reward jar and does not call here.
pub(crate) fn allocate_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    feature_byte: u8,
) -> Result<Option<u16>, String> {
    if feature_byte & SOURCE_FEATURE_MASK == 0 {
        return Ok(None);
    }
    let _ = particle_animation_limit(particles)?;
    let mut counter = runtime
        .constructor_counters()
        .ok_or_else(|| "final encounter source constructor counter is absent".to_owned())?[10];
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let seed = arena.new_event_rng(slot);
    let Some(row) = construct_row(
        slot,
        seed,
        position,
        feature_byte,
        counter,
        terrain,
        particles,
    )?
    else {
        return Ok(None);
    };
    counter = counter.wrapping_add(1);
    runtime.bytes[crate::native_world::RUNTIME_BYTES + 10] = counter;
    arena.economy.source_rng.insert(slot, seed);
    let live_position = row.position;
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, live_position, true, false)?;
    Ok(Some(slot))
}

pub(crate) fn write(arena: &mut SourceArena, actor: Actor) -> bool {
    let Some(row) = arena
        .entities
        .get_mut(usize::from(actor.slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
    else {
        return false;
    };
    store(row, actor);
    true
}

/// Integrator seam for the actual UpdateEntities source row. Typed state is
/// decoded for one callback and committed before returned producers enter the
/// shared same-pass tail.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    frame: Frame,
    apply_world: &mut ApplyEncounterAction<'_>,
) -> Result<Effects, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .cloned()
        .ok_or_else(|| format!("final encounter callback lost slot{slot}"))?;
    let mut actor = load(&row).ok_or_else(|| format!("final encounter row{slot} is malformed"))?;
    if actor.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT != 0 {
        return Ok(Effects::default());
    }
    match actor.action {
        84 => {
            let kill_credit =
                crate::native_world::creatures::raw_lifecycle::prekill(arena, slot, 80)?;
            return Ok(Effects {
                kill_credit,
                ..Effects::default()
            });
        }
        85 => {
            crate::native_world::creatures::creature_24::mc2::kill(
                arena, terrain, particles, slot,
            )?;
            return Ok(Effects {
                hide: true,
                ..Effects::default()
            });
        }
        _ => {}
    }
    let mut effects = super::step(&mut actor, frame, particles)?;
    if effects.kill_all_creatures {
        apply_world(
            arena,
            EncounterAction::KillCreatures {
                player: frame.player_slot,
            },
        )?;
    }
    if effects.refresh_creatures {
        apply_world(arena, EncounterAction::Refresh)?;
    }
    if effects.request_approach_step {
        effects.request_approach_step = false;
        if approach_step(
            arena,
            runtime,
            terrain,
            &mut actor,
            frame,
            &mut effects,
            apply_world,
        )? {
            actor.phase = 4;
            actor.subspell_index |= 0x80;
        }
    }
    let position = actor.position;
    let rng = actor.rng;
    store(&mut row, actor);
    arena.economy.source_rng.insert(slot, rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    materialize_summons(arena, runtime, terrain, particles, &mut effects)?;
    materialize_source_effects(arena, runtime, terrain, &mut effects)?;
    if effects.clear_stage_variable_count && !runtime.clear_active_stage_variable_count() {
        return Err("final encounter could not clear the active StageVar count".to_owned());
    }
    if let Some(counter) = effects.final_boss_active_counter
        && !runtime.set_final_boss_active_counter(counter)
    {
        return Err("final encounter source active-counter word is absent".to_owned());
    }
    Ok(effects)
}

/// Source sub_223E0 and sub_21F60 run immediately before action80. The
/// population lists are frozen at pass start; callbacks still use the live
/// numeric arena scan, so a higher newly allocated slot can run this pass.
pub(crate) fn scheduled_step(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    shared_spells: &mut crate::native_world::Level2SharedSpellResetSession,
    particles: &[ParticleParameter],
    animations: &crate::source_animation::SourceAnimationResidency,
    slot: u16,
    player_slot: u16,
    families: &[Vec<u16>; 29],
    class9: &[u16],
    setting_30: u8,
    apply_world: &mut ApplyEncounterAction<'_>,
) -> Result<Effects, String> {
    let player = arena
        .entities
        .get(usize::from(player_slot))
        .filter(|row| row.active && row.class == 3)
        .ok_or_else(|| format!("final encounter player slot{player_slot} is absent"))?;
    let mut frame = Frame {
        player_slot,
        player_source_id: player.source_id,
        player_position: player.position,
        player_alive: player.life >= 0 && !player.drawing_disabled(),
        player_class: player.class,
        player_model: player.model,
        source_setting: u16::from(setting_30),
        pressure_speed: runtime
            .final_boss_pressure_speed()
            .ok_or("final encounter pressure-speed word is absent")?,
        // The source intentionally scans the model0 chain for all three
        // counters; changing these to19/21 would alter summon selection.
        model0_count: families[0].len() as u16,
        model19_count: families[0].len() as u16,
        model21_count: families[0].len() as u16,
        model25_count: families[25]
            .iter()
            .filter(|&&slot| {
                arena
                    .entities
                    .get(usize::from(slot))
                    .is_some_and(|row| row.action != 200)
            })
            .count() as u16,
        ..Frame::default()
    };
    if arena
        .entities
        .get(usize::from(slot))
        .is_some_and(|row| row.action == 80)
    {
        frame.close_creature =
            super::protection::apply(arena, terrain, slot, shared_spells, particles, class9)?;
    }
    // sub_221F0 reads only the animation reached by this actual callback.
    // An unrelated resident entry may have an undefined lookahead and must
    // not make a different phase fail before it could perform the read.
    let timer_particle = arena
        .entities
        .get(usize::from(slot))
        .and_then(load)
        .and_then(|actor| {
            if actor.action != SOURCE_INITIAL_ACTION
                || actor.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT != 0
            {
                return None;
            }
            match actor.phase {
                6 => Some(343),
                10 => Some(344),
                13 if actor.timer.wrapping_sub(1) <= 0 => Some(345),
                _ => None,
            }
        });
    if let Some(index) = timer_particle {
        let particle = particles
            .get(index)
            .ok_or_else(|| format!("final encounter requires retained particle{index}"))?;
        particle
            .animation_limit()
            .ok_or_else(|| format!("final encounter particle{index} has invalid animation mode"))?;
        frame.source_animation_timer = animations.boss_timer_read(particle.base_tmap)?;
    }
    let effects = step_row(arena, runtime, terrain, particles, slot, frame, apply_world)?;
    if let Some(pressure) = effects.player_pressure
        && !runtime.set_final_boss_pressure_speed(pressure.speed)
    {
        return Err("final encounter could not store its pressure speed".to_owned());
    }
    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::super::FINAL_REWARD_PHASE;
    use super::*;
    use crate::campaign::source_arena::SourceArena;

    #[test]
    fn level25_boss_naturally_produces_model21_then_parent_loss_reclaims_it() {
        use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::Mc2DispatchEvent;
        use crate::native_world::creatures::creature_21::mc2 as model21;
        use std::collections::BTreeMap;
        use std::path::PathBuf;

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map_or_else(
            || generated.join("mc-bhq10-owner-v3-level25-20261029"), PathBuf::from,
        );
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map_or_else(
            || generated.join("dragon-bee-20261105-integration/current-level25-environment"),
            PathBuf::from,
        );
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache)
            .expect("qualified Level25 environment assets");
        let mut campaign = CampaignSession::open_with_owner_cache(
            &package, &cache, CampaignSelection::ordinary(25).unwrap(),
        ).expect("whole Level25 package and its matching owner cache");
        campaign.step(crate::PlayerCommand::default(), 0);
        // Declared upstream progression prerequisite, as in the delivered
        // Firefly producer check. No boss state, RNG, timer or child is forced.
        campaign.simulation_mut().activate_authored_group_prerequisite_for_test(29)
            .expect("actual group29 boss producer");
        let simulation = campaign.simulation();
        let boss = simulation.entities().iter().find(|row|
            row.active && row.class == 5 && row.model == 10 && !row.drawing_disabled())
            .expect("group29 publishes the real final boss").index;
        let parent = simulation.source_arena().handle(boss).unwrap();
        let owner = simulation.entities()[usize::from(boss)].source_id;
        let player = simulation.state.entity_index;
        let mut approach = simulation.entities()[usize::from(boss)].position;
        crate::move_entity(&mut approach, simulation.state.yaw.wrapping_add(1024) & 2047, 0, 2048);
        approach.z = simulation.canonical_terrain().ground_height(approach).wrapping_add(768);
        campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
        {
            // Declared player-only survival budget for this bounded encounter.
            // The retained 200,000-life negative died before the rare source
            // selection69; it did not exercise the model21 producer. This
            // larger entry budget changes no boss/child state or later damage.
            let live = &mut campaign.simulation_mut().session.state.live;
            let player_row = &mut live.economy.entities[usize::from(player)];
            player_row.set_max_life(1_000_000);
            player_row.set_life(1_000_000);
            let player_row = player_row.clone();
            live.player_life = player_row.life;
            live.shared_spells.sync_player_entity_from_authority(&player_row);
        }
        let mut child = None;
        let mut initial_position = None;
        let mut saw_launch = false;
        let mut saw_follow = false;
        let mut airborne = false;
        let mut landed = false;
        let mut shot = false;
        let mut last_boss = None;
        let mut selections = Vec::new();
        for turn in 0..4096 {
            // Declared stable player-position control. Source pressure really
            // moves the player within a turn; restore the setup between turns
            // so its range gate can keep admitting ordinary boss selections.
            // No boss/child field, RNG, phase or timer is changed.
            campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
            let before_selection = load(&campaign.simulation().entities()[usize::from(boss)])
                .filter(|actor| actor.phase == 8)
                .map(|actor| (actor.rng, campaign.simulation().source_arena()
                    .rebuilt_class5_model_slots()[0].len()));
            model21::start_callback_trace();
            campaign.step(crate::PlayerCommand::default(), 0);
            let callbacks = model21::take_callback_trace();
            assert!(!campaign.simulation().dispatch_events().iter().any(|event|
                matches!(event, Mc2DispatchEvent::Error { .. })),
                "boss model21 turn {turn}: {:?}", campaign.simulation().dispatch_events());
            assert!(campaign.simulation().session.state.live.player_life >= 0,
                "declared boss-producer survival budget exhausted at turn{turn}; no positive producer result; selections={selections:?}");
            let generation = campaign.generation();
            let frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
                campaign.inspection(), campaign.simulation(), &assets, generation,
            ).expect("actual Level25 frame");
            frame.apply_primary_visibility(campaign.simulation_mut(), generation).unwrap();
            let simulation = campaign.simulation();
            let arena = simulation.source_arena();
            last_boss = load(&simulation.entities()[usize::from(boss)])
                .map(|actor| (actor.phase, actor.summon_mode, actor.timer, actor.rng, actor.life));
            if let Some(before) = before_selection {
                if selections.len() == 16 { selections.remove(0); }
                selections.push((turn, before, last_boss));
            }
            if child.is_none() {
                let census = arena.creature_runtime_census(&BTreeMap::new(), &BTreeMap::new());
                child = census.entries().iter().find(|entry|
                    entry.raw_model == Some(21) && entry.role == CreatureRuntimeRole::Root
                        && entry.origin == CreatureRootOrigin::CreatureConversion { producer_slot: boss }
                        && arena.creature_parent_binding(entry.handle) == Some(parent))
                    .map(|entry| entry.handle);
                if let Some(handle) = child {
                    let row = &simulation.entities()[usize::from(handle.source_slot)];
                    assert_eq!((row.class, row.model, row.source_id, row.max_life, row.mana),
                        (5, 21, owner, 1000, 1000));
                    assert_eq!(word(row, 152), 0, "source leaves NewEvent player owner untouched");
                    assert_eq!(word(row, 44), boss);
                    initial_position = Some(row.position);
                }
            }
            if let Some(handle) = child {
                assert!(arena.resolve(handle).is_some(),
                    "actual boss model21 vanished before connected follow/attack: turn{turn}");
                let row = &simulation.entities()[usize::from(handle.source_slot)];
                saw_follow |= row.record[77] == 16;
                for callback in callbacks.iter().filter(|receipt| receipt.actor == handle) {
                    saw_launch |= callback.before.record[77] == 17;
                    if let Some(after) = &callback.after {
                        airborne |= after.position.z > simulation.canonical_terrain().ground_height(after.position)
                            && (3..=8).contains(&after.record[74]);
                        landed |= airborne && matches!(after.record[74], 9 | 10);
                    }
                    shot |= callback.projectile_packet.is_some_and(|packet|
                        (packet.source_class, packet.source_model, packet.source_subspell_index,
                            packet.owner_entity_index, packet.target_source_entity_index)
                            == (9, 0, 500, owner, player));
                }
                if saw_launch && saw_follow && airborne && landed && shot {
                    assert_ne!(Some(row.position), initial_position);
                    assert!(simulation.campaign_entity_presentations(0).unwrap().iter()
                        .any(|view| view.entity_index == handle.source_slot));
                    // Configured parent-death branch, isolated from the
                    // natural producer/combat run. A real shot can leave170
                    // active:26220 does not read the parent, and12500 leaves
                    // attack actions alone. Retreat only the player so the
                    // next32-callback range check permits170→169→175/1E580.
                    let loss_entry = row.clone();
                    assert!(loss_entry.life >= 0);
                    assert_eq!(loss_entry.record[77], 16);
                    let rules = crate::level_definition::CREATURE21_CLASS_PROPERTIES;
                    let callback_bound = usize::from(rules.attack_cadence_mask) + 2;
                    let mut retreat = loss_entry.position;
                    retreat.x = retreat.x.wrapping_add(u16::try_from(4 * rules.target_range).unwrap());
                    retreat.z = simulation.canonical_terrain().ground_height(retreat).wrapping_add(768);
                    let mut loss = campaign.clone();
                    loss.simulation_mut().session.state.live.economy.entities
                        [usize::from(boss)].set_life(-1);
                    let mut loss_visits = Vec::new();
                    let mut attack_reset = false;
                    let mut expired = false;
                    for wait in 0..callback_bound {
                        loss.simulation_mut().set_level2_entity_position(player, retreat).unwrap();
                        model21::start_callback_trace();
                        loss.step(crate::PlayerCommand::default(), 0);
                        let visits = model21::take_callback_trace();
                        assert!(!loss.simulation().dispatch_events().iter().any(|event|
                            matches!(event, Mc2DispatchEvent::Error { .. })),
                            "parent loss wait{wait}: {:?}", loss.simulation().dispatch_events());
                        assert!(loss.simulation().session.state.live.player_life >= 0,
                            "parent-loss control exhausted the existing player budget");
                        let mut visits = visits.iter().filter(|visit| visit.actor == handle);
                        let visit = visits.next().unwrap_or_else(|| panic!(
                            "original model21 missed its scheduled parent-loss visit: entry={loss_entry:?} visits={loss_visits:?}"));
                        assert!(visits.next().is_none(), "one source callback per ordinary outer turn");
                        let after = visit.after.as_ref().expect("the original generation survives this callback");
                        let target_distance = visit.entry_target.map(|target|
                            crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
                                visit.before.position, target.position)));
                        loss_visits.push((wait, visit.before.action, visit.before.phase,
                            visit.entry_parent.map(|parent| (parent.slot, parent.life, parent.control_flags)),
                            target_distance, after.action, after.drawing_disabled()));
                        assert!(visit.before.life >= 0 && after.life >= 0,
                            "parent expiry must not be a combat death: {loss_visits:?}");
                        assert_eq!(word(&visit.before, 44), boss);
                        assert_eq!(visit.before.record[77], 16);
                        assert!(loss.simulation().source_arena().resolve(handle).is_some(),
                            "the original child remains for the same-pass tail");
                        if visit.before.action == 170 {
                            let target = visit.entry_target.expect("retained actual attack target");
                            assert_eq!(target.slot, player);
                            assert!(target.life >= 0 && target.control_flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0);
                            assert!(target_distance.unwrap() >= rules.target_range,
                                "declared player retreat must reach the actual callback range read: {loss_visits:?}");
                            assert!(!after.drawing_disabled() && visit.expiry_effect.is_none(),
                                "170 must not invent a parent-life read: {loss_visits:?}");
                            if visit.before.phase & rules.attack_cadence_mask == 0 {
                                assert_eq!(after.action, 169, "real range cadence resets the attack");
                                attack_reset = true;
                            } else {
                                assert_eq!(after.action, 170, "attack persists until its source cadence");
                            }
                            continue;
                        }
                        assert_eq!(visit.before.action, 175, "source prepass restores the StageVar callback");
                        if loss_entry.action == 170 { assert!(attack_reset); }
                        let parent_read = visit.entry_parent.expect("actual fixed parent record at1E580");
                        assert_eq!(parent_read.slot, boss);
                        assert!(parent_read.life < 0 || parent_read.control_flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT != 0,
                            "actual parent read must cause the expiry: {loss_visits:?}");
                        assert!((word(&visit.before, 50) as i16) > 0);
                        assert_eq!(word(after, 50), 0, "1E580 zeroes the remaining lifetime");
                        assert!(after.drawing_disabled());
                        let (effect, packet) = visit.expiry_effect.as_ref().expect("actual1E580 Impact allocation");
                        assert_eq!((packet.class, packet.model, packet.index), (10, 0, effect.source_slot));
                        assert_eq!(packet.position, visit.before.position, "expiry birth precedes the jump tail");
                        assert_eq!(loss.simulation().entities()[usize::from(handle.source_slot)].phase,
                            visit.before.phase.wrapping_add(1), "hidden original receives its same-pass tail");
                        expired = true;
                        break;
                    }
                    assert!(expired, "source32-attack-callback+1-stage bound: entry={loss_entry:?} visits={loss_visits:?}");
                    let hidden = loss.simulation().source_arena();
                    assert!(hidden.resolve(handle).is_some());
                    assert!(hidden.entities[usize::from(handle.source_slot)].drawing_disabled());
                    assert!(!loss.simulation().campaign_entity_presentations(0).unwrap().iter()
                        .any(|view| view.entity_index == handle.source_slot));
                    loss.step(crate::PlayerCommand::default(), 0);
                    assert!(!loss.simulation().dispatch_events().iter().any(|event|
                        matches!(event, Mc2DispatchEvent::Error { .. })));
                    assert!(loss.simulation().source_arena().resolve(handle).is_none());
                    eprintln!("Level25 boss leaping creature: parent={parent:?} child={handle:?} connected at turn{turn}; parent-loss entryaction={} visits={loss_visits:?}", loss_entry.action);
                    return;
                }
            }
        }
        panic!("ordinary boss model21 path incomplete: boss={last_boss:?} child={child:?} launch={saw_launch} follow={saw_follow} air={airborne} land={landed} shot={shot} player_life={} selections={selections:?}", campaign.simulation().session.state.live.player_life);
    }

    #[test]
    fn level25_boss_naturally_produces_firefly_then_parent_loss_reclaims_it() {
        use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::Mc2DispatchEvent;
        use std::collections::BTreeMap;
        use std::path::PathBuf;

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map_or_else(
            || generated.join("mc-bhq10-owner-v3-level25-20261029"), PathBuf::from,
        );
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map_or_else(
            || generated.join("dragon-bee-20261105-integration/current-level25-environment"),
            PathBuf::from,
        );
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache)
            .expect("qualified Level25 environment assets");
        let mut campaign = CampaignSession::open_with_owner_cache(
            &package, &cache, CampaignSelection::ordinary(25).unwrap(),
        ).expect("whole Level25 package and its matching owner cache");
        campaign.step(crate::PlayerCommand::default(), 0);
        // Declared upstream progression boundary: this does not claim public
        // demolition of the preceding BUILD68/69/72/73 chain. The existing
        // Level25 package has an empty collision-mask bank for that chain.
        campaign.simulation_mut().activate_authored_group_prerequisite_for_test(29)
            .expect("actual authored group29 constructs the boss");
        let simulation = campaign.simulation();
        let boss = simulation.entities().iter().find(|row|
            row.active && row.class == 5 && row.model == 10 && !row.drawing_disabled())
            .expect("group29 publishes the real final boss").index;
        let parent = simulation.source_arena().handle(boss).expect("boss generation");
        let boss_owner = simulation.entities()[usize::from(boss)].source_id;
        let player = simulation.state.entity_index;
        let mut approach = simulation.entities()[usize::from(boss)].position;
        crate::move_entity(&mut approach, simulation.state.yaw.wrapping_add(1024) & 2047, 0, 2048);
        approach.z = simulation.canonical_terrain().ground_height(approach).wrapping_add(768);
        campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
        {
            // Declared player-only survival resource, allowing natural boss
            // selection and flight to proceed without scripted combat.
            let live = &mut campaign.simulation_mut().session.state.live;
            live.player_life = 200_000;
            live.shared_spells.set_player_entity_life(200_000);
            live.economy.entities[usize::from(player)].set_life(200_000);
        }

        let mut child = None;
        let mut first_position = None;
        let mut last_boss = None;
        for turn in 0..4096 {
            campaign.step(crate::PlayerCommand::default(), 0);
            assert!(!campaign.simulation().dispatch_events().iter().any(|event|
                matches!(event, Mc2DispatchEvent::Error { .. })),
                "boss Firefly turn {turn}: {:?}", campaign.simulation().dispatch_events());
            let generation = campaign.generation();
            let frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
                campaign.inspection(), campaign.simulation(), &assets, generation,
            ).expect("actual Level25 frame");
            frame.apply_primary_visibility(campaign.simulation_mut(), generation)
                .expect("actual frame visibility feedback");
            let simulation = campaign.simulation();
            let arena = simulation.source_arena();
            last_boss = load(&simulation.entities()[usize::from(boss)])
                .map(|actor| (actor.phase, actor.summon_mode, actor.timer, actor.rng, actor.life));
            if child.is_none() {
                let census = arena.creature_runtime_census(&BTreeMap::new(), &BTreeMap::new());
                child = census.entries().iter().find(|entry|
                    entry.raw_model == Some(19)
                        && entry.role == CreatureRuntimeRole::Root
                        && entry.origin == CreatureRootOrigin::CreatureConversion { producer_slot: boss }
                        && arena.creature_parent_binding(entry.handle) == Some(parent))
                    .map(|entry| entry.handle);
                if let Some(handle) = child {
                    let root = arena.economy.authored_class5().iter().find(|row|
                        row.source_entity_index == handle.source_slot).unwrap();
                    let state = arena.economy.authored_fireflies().iter().find(|row|
                        row.source_entity_index == handle.source_slot).unwrap();
                    assert_eq!((root.model, root.max_life, root.life, root.mana, root.subspell_index),
                        (19, 600, 600, 300, 300));
                    assert_eq!((state.stage_var2, state.parent_entity_index, state.entity_id),
                        (17, boss, boss_owner));
                    assert_eq!(arena.creature_parent(handle), Some(parent));
                    assert!(simulation.campaign_entity_presentations(0).unwrap().iter()
                        .any(|view| view.entity_index == handle.source_slot));
                    first_position = Some(root.position);
                }
            }
            if let Some(handle) = child {
                assert!(arena.resolve(handle).is_some(), "actual boss child vanished before follow");
                let root = arena.economy.authored_class5().iter().find(|row|
                    row.source_entity_index == handle.source_slot).unwrap();
                let state = arena.economy.authored_fireflies().iter().find(|row|
                    row.source_entity_index == handle.source_slot).unwrap();
                if state.stage_var2 == 16 && root.action == 159 {
                    assert_ne!(Some(root.position), first_position,
                        "real source flight must move before follow");
                    // Separate configured failure control: mark only the real
                    // boss parent dead, leaving the observed child untouched.
                    let mut loss = campaign.clone();
                    loss.simulation_mut().session.state.live.economy.entities
                        [usize::from(boss)].set_life(-1);
                    loss.step(crate::PlayerCommand::default(), 0);
                    assert!(!loss.simulation().dispatch_events().iter().any(|event|
                        matches!(event, Mc2DispatchEvent::Error { .. })));
                    let lost = loss.simulation().source_arena();
                    assert!(lost.resolve(handle).is_some(), "hidden child survives its source tail");
                    let hidden = lost.economy.authored_class5().iter().find(|row|
                        row.source_entity_index == handle.source_slot).expect("same child row");
                    assert!(!hidden.drawing_enabled, "parent loss hides the actual mode16 child");
                    assert!(!loss.simulation().campaign_entity_presentations(0).unwrap().iter()
                        .any(|view| view.entity_index == handle.source_slot));
                    loss.step(crate::PlayerCommand::default(), 0);
                    assert!(loss.simulation().source_arena().resolve(handle).is_none(),
                        "the following owning boundary reclaims the original child generation");
                    eprintln!("Level25 boss Firefly: parent={parent:?} child={handle:?} followed at turn {turn}");
                    return;
                }
            }
        }
        panic!("unaltered Level25 boss did not produce and launch Firefly: boss={last_boss:?} child={child:?}");
    }

    // These existing checks isolate boss phase/constructor behavior. The
    // source-time callback/order and actual carrier mutations have separate
    // regressions; they are not inferred from this phase-only observer.
    fn ignore_world_action(_: &mut SourceArena, _: EncounterAction) -> Result<(), String> {
        Ok(())
    }

    struct Source24Fixture {
        arena: SourceArena,
        runtime: RuntimeSnapshot,
        terrain: SessionTerrain,
        particles: Vec<ParticleParameter>,
        player_slot: u16,
        player_source_id: u16,
        player_position: Position,
        boss_position: Position,
    }

    fn source24_fixture() -> Source24Fixture {
        let package_root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
            },
            std::path::PathBuf::from,
        );
        let package = crate::campaign::spec::DecodedCampaignPackage::decode(
            package_root.join("level25"),
            crate::campaign::spec::CampaignSelection::ordinary(25)
                .expect("ordinary Level25 selection"),
        )
        .expect("decode owner-data Level25 campaign package");
        let entities = crate::native_world::parse_entities(package.member("entities.bin").unwrap())
            .unwrap()
            .into_boxed_slice();
        let runtime =
            crate::native_world::parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
        assert!(
            runtime.constructor_counters().is_some(),
            "Level25 runtime retains the source constructor counter table"
        );
        let presentation =
            crate::native_world::parse_presentation(package.member("presentation.bin").unwrap())
                .unwrap();
        let particles = presentation.particles.to_vec();
        let mut terrain = SessionTerrain::from_planes(
            crate::native_world::MapType::Night,
            crate::native_world::parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
            0,
        );
        let player = entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
            .expect("Level25 retains its live class3/model0 player row");
        let player_slot = player.index;
        let player_source_id = player.source_id;
        let player_position = player.position;
        let boss_feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
            .unwrap()
            .into_iter()
            .find(|feature| feature.index == 379)
            .expect("Level25 source contains authored record379");
        assert_eq!(
            (
                boss_feature.kind,
                boss_feature.subtype,
                boss_feature.tile_x,
                boss_feature.tile_y,
                boss_feature.activation_group,
                boss_feature.stage_tag,
            ),
            (5, 10, 40, 213, 29, 29),
            "record379 is the authored group29 model10 objective target"
        );
        let mut boss_position = Position {
            x: u16::try_from((u32::from(boss_feature.tile_x) << 8) + 128)
                .expect("Level25 row379 x center fits the source coordinate"),
            y: u16::try_from((u32::from(boss_feature.tile_y) << 8) + 128)
                .expect("Level25 row379 y center fits the source coordinate"),
            z: 0,
        };
        boss_position.z = terrain.ground_height(boss_position);
        let economy =
            crate::native_world::Level2EconomyState::from_parts_with_castle_stage_footprints(
                terrain.clone(),
                &entities,
                player.mana,
                player.model,
                0,
                0,
                runtime.random_state,
                Default::default(),
                runtime.constructor_counters(),
            )
            .unwrap();
        let arena = SourceArena::new(entities, economy);

        Source24Fixture {
            arena,
            runtime,
            terrain,
            particles,
            player_slot,
            player_source_id,
            player_position,
            boss_position,
        }
    }

    fn boss_fixture() -> (Source24Fixture, u16) {
        let mut fixture = source24_fixture();
        let boss_slot = allocate_authored(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            fixture.boss_position,
            SOURCE_FEATURE_MASK,
        )
        .unwrap()
        .expect("available source slot constructs the authored gated boss");
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.set_source_byte_46(9);
        boss.record[71] = 4;
        boss.record[40..42].copy_from_slice(&1_u16.to_le_bytes());
        boss.set_source_dword_10(1);
        boss.set_yaw(317);
        (fixture, boss_slot)
    }

    fn boss_frame(fixture: &Source24Fixture) -> Frame {
        Frame {
            player_slot: fixture.player_slot,
            player_source_id: fixture.player_source_id,
            player_position: fixture.player_position,
            player_alive: true,
            player_class: 3,
            player_model: 0,
            ..Frame::default()
        }
    }

    fn exhaust(arena: &mut SourceArena) {
        for row in &mut arena.entities {
            row.set_control_flags(
                row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
        }
        arena.economy.exhaust_new_event_capacity_for_test();
        arena.rebuild_source_free_arena();
    }

    #[test]
    fn source_bitmap_class_keeps_prefix_order_and_skips_sub10130_return_two() {
        for class in [0_u8, 7, 15] {
            let cumulative = crate::level1_mission::source_search_offsets(class);
            let prefix_len = if class == 0 {
                0
            } else {
                crate::level1_mission::source_search_offsets(class - 1).len()
            };
            assert!(cumulative.len() > prefix_len);
            assert_eq!(
                source_bitmap_class_offsets(class).unwrap(),
                cumulative[prefix_len..cumulative.len() - 1],
                "class {class} is exact class-major SEARCH.DAT suffix minus the final return-2 pair"
            );
        }
        assert!(source_bitmap_class_offsets(16).is_err());
    }

    #[test]
    fn encounter_world_actions_run_in_source_order_before_mana_allocations() {
        for (timer, mode) in [(17, Some(2)), (16, None)] {
            let (mut fixture, boss_slot) = boss_fixture();
            let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
            boss.set_source_byte_46(1);
            boss.set_subspell_index(4);
            boss.set_source_dword_10(timer);
            let before = fixture.arena.economy.model13_spawned_children().len();
            let frame = boss_frame(&fixture);
            let mut observed = Vec::new();
            step_row(
                &mut fixture.arena,
                &mut fixture.runtime,
                &mut fixture.terrain,
                &fixture.particles,
                boss_slot,
                frame,
                &mut |arena, action| {
                    assert_eq!(arena.economy.model13_spawned_children().len(), before);
                    observed.push(action);
                    Ok(())
                },
            )
            .unwrap();
            let mut expected = vec![EncounterAction::KillCreatures {
                player: frame.player_slot,
            }];
            if let Some(mode) = mode {
                expected.push(EncounterAction::Approach(mode));
            }
            assert_eq!(observed, expected);
            assert_eq!(
                fixture.arena.economy.model13_spawned_children().len(),
                before + 4
            );
        }

        let (mut fixture, boss_slot) = boss_fixture();
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.set_source_byte_46(1);
        boss.set_subspell_index(4);
        boss.set_source_dword_10(17);
        let before = fixture.arena.economy.model13_spawned_children().len();
        let frame = boss_frame(&fixture);
        assert!(
            step_row(
                &mut fixture.arena,
                &mut fixture.runtime,
                &mut fixture.terrain,
                &fixture.particles,
                boss_slot,
                frame,
                &mut |_, _| Err("world action failed".to_owned()),
            )
            .is_err()
        );
        assert_eq!(
            fixture.arena.economy.model13_spawned_children().len(),
            before
        );
    }

    #[test]
    fn phase_zero_calls_source_approach_in_same_pass_and_spawns_10_14_effects() {
        let (mut fixture, boss_slot) = boss_fixture();
        let boss_position = fixture.arena.entities[usize::from(boss_slot)].position;
        let (center_x, center_y) = source_tile_center(boss_position);
        let offsets = source_bitmap_class_offsets(0).unwrap();
        let cumulative = crate::level1_mission::source_search_offsets(0);
        let skipped = *cumulative.last().expect("SEARCH.DAT class0 has a row");
        let mut expected_tiles = Vec::new();
        {
            let (planes, _, _, _) = fixture.terrain.disaster_parts();
            for (dx, dy) in offsets {
                let x = center_x.wrapping_add(i16::from(dx)) as u8;
                let y = center_y.wrapping_add(i16::from(dy)) as u8;
                let tile = usize::from(x) + 256 * usize::from(y);
                planes.ground_heights[tile] = 100;
                expected_tiles.push(tile);
            }
            let x = center_x.wrapping_add(i16::from(skipped.0)) as u8;
            let y = center_y.wrapping_add(i16::from(skipped.1)) as u8;
            planes.ground_heights[usize::from(x) + 256 * usize::from(y)] = 100;
        }
        let prior_children = fixture.arena.economy.model13_spawned_children().len();
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.set_source_byte_46(0);
        boss.set_subspell_index(0);
        boss.set_source_dword_10(0);

        let frame = boss_frame(&fixture);
        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        )
        .unwrap();

        let boss = &fixture.arena.entities[usize::from(boss_slot)];
        assert_eq!(
            boss.record[74], 1,
            "phase0 enters phase1 before its callback"
        );
        assert_eq!(
            boss.source_dword_10(),
            14,
            "same-pass callback deforms class0 and decrements timer"
        );
        assert_eq!(boss.subspell_index, 8);
        assert_eq!(effects.sounds.as_slice(), &[10]);
        assert!(
            !effects.request_approach_step,
            "the adapter consumed its internal call request"
        );
        assert!(
            expected_tiles
                .iter()
                .all(|tile| fixture.terrain.planes().ground_heights[*tile] == 99)
        );
        let x = center_x.wrapping_add(i16::from(skipped.0)) as u8;
        let y = center_y.wrapping_add(i16::from(skipped.1)) as u8;
        assert_eq!(
            fixture.terrain.planes().ground_heights[usize::from(x) + 256 * usize::from(y)],
            100,
            "source sub_10130 return2 omits the final SEARCH.DAT pair"
        );
        let children = fixture.arena.economy.model13_spawned_children();
        assert!(
            children.len() > prior_children,
            "successful 10/14 calls allocate real model14 rows"
        );
        assert!(
            children[prior_children..]
                .iter()
                .all(|child| (8..=15).contains(&child.life))
        );
    }

    #[test]
    fn source_approach_clear_scan_enters_mode_four_but_obstruction_retries() {
        // sub_21490, source12930: v28 remains1 only when every visited
        // SEARCH.DAT tile is clear. A blocked tile selects the15-tick retry.
        for blocked in [false, true] {
            let (mut fixture, boss_slot) = boss_fixture();
            let boss_position = fixture.arena.entities[usize::from(boss_slot)].position;
            let (center_x, center_y) = source_tile_center(boss_position);
            let offsets = source_bitmap_class_offsets(7).unwrap();
            {
                let (planes, _, _, _) = fixture.terrain.disaster_parts();
                for &(dx, dy) in &offsets {
                    let x = center_x.wrapping_add(i16::from(dx)) as u8;
                    let y = center_y.wrapping_add(i16::from(dy)) as u8;
                    planes.terrain_types[usize::from(x) + 256 * usize::from(y)] = 0;
                }
                if blocked {
                    let (dx, dy) = offsets[0];
                    let x = center_x.wrapping_add(i16::from(dx)) as u8;
                    let y = center_y.wrapping_add(i16::from(dy)) as u8;
                    planes.terrain_types[usize::from(x) + 256 * usize::from(y)] = 1;
                }
            }
            let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
            boss.set_source_byte_46(1);
            boss.set_subspell_index(8);
            boss.set_source_dword_10(-1);
            let frame = boss_frame(&fixture);
            let effects = step_row(
                &mut fixture.arena,
                &mut fixture.runtime,
                &mut fixture.terrain,
                &fixture.particles,
                boss_slot,
                frame,
                &mut ignore_world_action,
            )
            .unwrap();
            let boss = &fixture.arena.entities[usize::from(boss_slot)];
            assert_eq!(boss.record[74], 1);
            assert_eq!(boss.subspell_index, if blocked { 8 } else { 4 });
            assert_eq!(boss.source_dword_10(), if blocked { 15 } else { 70 });
            assert!(
                !effects.kill_all_creatures,
                "the newly selected mode runs next callback"
            );
            assert!(!effects.request_approach_step);
            assert!(effects.sounds.is_empty());
            if !blocked {
                let retained_rows = fixture.runtime.stage_variables.clone();
                let effects = step_row(
                    &mut fixture.arena,
                    &mut fixture.runtime,
                    &mut fixture.terrain,
                    &fixture.particles,
                    boss_slot,
                    frame,
                    &mut ignore_world_action,
                )
                .unwrap();
                assert!(effects.kill_all_creatures);
                assert!(effects.clear_stage_variable_count);
                assert_eq!(fixture.runtime.active_stage_variable_count(), 0);
                assert_eq!(fixture.runtime.stage_variables, retained_rows);
                assert_eq!(
                    fixture.arena.entities[usize::from(boss_slot)].source_dword_10(),
                    69
                );
            }
        }
    }

    #[test]
    fn source_close_result_enters_phase_four_only_after_approach_callback() {
        let (mut fixture, boss_slot) = boss_fixture();
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.set_source_byte_46(1);
        boss.set_subspell_index(0);
        boss.set_source_dword_10(0);

        let frame = boss_frame(&fixture);
        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        )
        .unwrap();

        let boss = &fixture.arena.entities[usize::from(boss_slot)];
        assert_eq!(boss.record[74], 4);
        assert_eq!(boss.subspell_index, 0x80);
        assert!(!effects.request_approach_step);
    }

    #[test]
    fn authored_constructor_obeys_gate_and_pinned_source_defaults() {
        let fixture = source24_fixture();
        let position = Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        };

        assert_eq!(
            construct_row(
                7,
                0x1234,
                position,
                0,
                12,
                &fixture.terrain,
                &fixture.particles,
            )
            .unwrap(),
            None,
            "source feature gate rejects without creating a boss"
        );
        let row = construct_row(
            7,
            0x1234,
            position,
            SOURCE_FEATURE_MASK,
            12,
            &fixture.terrain,
            &fixture.particles,
        )
        .unwrap()
        .expect("enabled source feature constructs the final boss");

        assert_eq!((row.class, row.model, row.action), (5, 10, 80));
        assert_eq!((row.max_life, row.life, row.mana), (300_000, 300_000, 0));
        assert_eq!(row.source_dword_10(), 0);
        assert_eq!(row.record[60], 1);
        assert_eq!(row.record[61], 64);
        assert_eq!(row.record[69], 3);
        assert_eq!(row.phase, 12);
        assert_eq!(row.position.z, fixture.terrain.ground_height(position));
        assert_eq!(row.particle_parameter_index, Some(SOURCE_PARTICLE));
        assert_eq!(row.parameter_relation, Some(SOURCE_DESCRIPTOR));
    }

    #[test]
    fn boss_model21_stagevar_summon_constructs_real_child_and_only_then_plays_sound() {
        let (mut fixture, boss_slot) = boss_fixture();
        let frame = boss_frame(&fixture);
        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        )
        .unwrap();

        assert!(
            effects.summons.is_empty(),
            "the owned mode-4 request was consumed"
        );
        assert!(
            effects.sounds.contains(&42),
            "child sound follows successful allocation"
        );
        let child = fixture
            .arena
            .entities
            .iter()
            .find(|row| {
                row.active
                    && row.class == 5
                    && row.model == 21
                    && row.action == 175
                    && word(row, 44) == boss_slot
            })
            .expect("boss mode4 allocated the canonical model21 StageVar child");
        assert_eq!(child.record[77], 17);
        assert_eq!(word(child, 50), 250);
        assert_eq!(word(child, 152), 0, "boss source sets target, not playerEntityIndex");
        assert_eq!(word(child, 154), fixture.player_slot);
        assert_eq!(child.actual_speed, 320);
        assert_eq!(child.yaw, 317);
        assert_eq!(child.source_roll(), 317);
        assert_eq!(
            fixture
                .arena
                .creature_parent(fixture.arena.handle(child.index).unwrap()),
            fixture.arena.handle(boss_slot),
        );
        assert_eq!(
            child.source_id,
            fixture.arena.entities[usize::from(boss_slot)].source_id
        );
    }

    #[test]
    fn configured_boss_firefly_and_army_births_keep_distinct_source_mana() {
        use crate::level_definition::FIREFLY_CLASS_PROPERTIES;

        let (mut fixture, boss_slot) = boss_fixture();
        fixture.arena.entities[usize::from(boss_slot)].record[71] = 6;
        let frame = boss_frame(&fixture);
        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        ).unwrap();
        assert!(effects.summons.is_empty());
        assert!(effects.sounds.contains(&44));
        let child = fixture.arena.economy.authored_class5.iter()
            .find(|row| row.active && row.model == 19 && row.action == 159)
            .expect("configured boss mode6 produced its Firefly");
        let child_slot = child.source_entity_index;
        assert_eq!((child.max_life, child.life, child.mana, child.max_mana),
            (FIREFLY_CLASS_PROPERTIES.max_life, FIREFLY_CLASS_PROPERTIES.max_life,
             FIREFLY_CLASS_PROPERTIES.max_life >> 1, 0));
        assert_eq!((child.minimum_speed, child.maximum_speed, child.actual_speed),
            (FIREFLY_CLASS_PROPERTIES.minimum_speed, FIREFLY_CLASS_PROPERTIES.maximum_speed, 320));
        assert_eq!(child.subspell_index, FIREFLY_CLASS_PROPERTIES.contact_damage);
        assert_eq!(child.word_0x96_150, fixture.player_slot);
        let state = fixture.arena.economy.authored_fireflies.iter()
            .find(|state| state.source_entity_index == child_slot).unwrap();
        assert_eq!((state.stage_var2, state.word_0x2e, state.parent_entity_index, state.entity_id),
            (17, 250, boss_slot, fixture.arena.entities[usize::from(boss_slot)].source_id));
        let child_handle = fixture.arena.handle(child_slot).unwrap();
        assert_eq!(fixture.arena.creature_parent(child_handle), fixture.arena.handle(boss_slot));

        let army = crate::native_world::level2_class5::firefly::allocate_summoned(
            &mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles,
            fixture.player_position, fixture.player_slot, fixture.player_source_id,
            fixture.player_slot,
        ).unwrap().expect("Army control has source capacity");
        let army_row = fixture.arena.economy.authored_class5.iter()
            .find(|row| row.source_entity_index == army.source_slot).unwrap();
        assert_eq!((army_row.life, army_row.mana, army_row.max_mana),
            (FIREFLY_CLASS_PROPERTIES.max_life, 0, 0));
        let army_state = fixture.arena.economy.authored_fireflies.iter()
            .find(|state| state.source_entity_index == army.source_slot).unwrap();
        assert_eq!((army_state.stage_var2, army_state.word_0x2e), (13, 250));
    }

    #[test]
    fn boss_model21_stagevar_summon_exhaustion_consumes_attempt_without_sound() {
        let (mut fixture, boss_slot) = boss_fixture();
        exhaust(&mut fixture.arena);
        let frame = boss_frame(&fixture);
        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        )
        .unwrap();

        assert!(
            effects.summons.is_empty(),
            "the failed source attempt is consumed"
        );
        assert!(
            !effects.sounds.contains(&42),
            "failed allocation has no success sound"
        );
        assert!(!fixture.arena.entities.iter().any(|row| {
            row.active
                && row.class == 5
                && row.model == 21
                && row.action == 175
                && word(row, 44) == boss_slot
        }));
    }

    #[test]
    fn boss_attack_constructs_each_real_class9_product_and_full_arena_suppresses_sound() {
        for (mode, model, impact, lifetime, sound) in [
            (1, 0, 0, 0, 15),
            (2, 9, 23, 0, 23),
            (8, 26, 22, 3, 15),
            (9, 3, 17, 10, 15),
        ] {
            for full in [false, true] {
                let (mut fixture, boss_slot) = boss_fixture();
                fixture.arena.entities[usize::from(boss_slot)].record[71] = mode;
                if full {
                    exhaust(&mut fixture.arena);
                }
                let frame = boss_frame(&fixture);
                let effects = step_row(
                    &mut fixture.arena,
                    &mut fixture.runtime,
                    &mut fixture.terrain,
                    &fixture.particles,
                    boss_slot,
                    frame,
                    &mut ignore_world_action,
                )
                .unwrap();
                assert!(effects.summons.is_empty());
                assert_eq!(effects.sounds.contains(&sound), !full);
                let owner = fixture.arena.entities[usize::from(boss_slot)].source_id;
                if model == 0 {
                    let children = fixture
                        .arena
                        .economy
                        .fireball_projectiles
                        .iter()
                        .filter(|row| {
                            row.active
                                && row.origin
                                    == crate::native_world::FireballProjectileOrigin::FinalBoss
                                && row.owner_entity_index == owner
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(children.len(), usize::from(!full));
                    if let Some(child) = children.first() {
                        assert_eq!(
                            (child.source_class, child.source_model, child.action),
                            (9, 0, 0)
                        );
                        assert_eq!((child.target_class, child.target_model), (3, 0));
                        assert_eq!(child.source_subspell_index, 800);
                    }
                } else {
                    let children = fixture
                        .arena
                        .entities
                        .iter()
                        .filter(|row| {
                            row.active
                                && row.class == 9
                                && row.model == model
                                && row.source_id == owner
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(children.len(), usize::from(!full));
                    if let Some(child) = children.first() {
                        assert_eq!(
                            (child.record[71], child.record[72], child.record[74]),
                            (10, impact, lifetime)
                        );
                        assert_eq!((child.record[69], child.record[70]), (3, 0));
                        assert!(
                            fixture
                                .arena
                                .economy
                                .map_chains
                                .linked_entity_indices()
                                .contains(&child.index)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn phase15_materializes_the_owned_volcano_and_sets_success_flag() {
        let (mut fixture, boss_slot) = boss_fixture();
        let boss_owner = fixture.arena.entities[usize::from(boss_slot)].source_id;
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.set_source_byte_46(FINAL_REWARD_PHASE);
        boss.set_source_dword_10(1);
        fixture.runtime.bytes[68] = 0;
        let frame = boss_frame(&fixture);

        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        )
        .unwrap();

        let reward_slot = effects
            .final_death_reward_slot
            .expect("the phase15 source constructor returns its actual 10/9 slot");
        let reward = &fixture.arena.entities[usize::from(reward_slot)];
        assert!(effects.final_death_reward_attempted);
        assert!(!effects.final_death_reward_pending);
        assert_eq!(fixture.runtime.bytes[68], 1);
        assert_eq!((reward.class, reward.model, reward.action), (10, 9, 9));
        assert_eq!((reward.life, reward.max_life), (32, 11));
        assert_eq!(reward.source_id, boss_owner);
        assert!(effects.hide);
        assert_eq!(effects.final_boss_active_counter, Some(0));
        assert!(fixture.arena.entities[usize::from(boss_slot)].drawing_disabled());
    }

    #[test]
    fn phase15_full_arena_consumes_attempt_without_reward_or_success_flag() {
        let (mut fixture, boss_slot) = boss_fixture();
        let boss_owner = fixture.arena.entities[usize::from(boss_slot)].source_id;
        fixture.arena.entities[usize::from(boss_slot)].set_source_byte_46(FINAL_REWARD_PHASE);
        fixture.arena.entities[usize::from(boss_slot)].set_source_dword_10(1);
        fixture.runtime.bytes[68] = 0x5a;
        exhaust(&mut fixture.arena);
        let frame = boss_frame(&fixture);

        let effects = step_row(
            &mut fixture.arena,
            &mut fixture.runtime,
            &mut fixture.terrain,
            &fixture.particles,
            boss_slot,
            frame,
            &mut ignore_world_action,
        )
        .unwrap();

        assert_eq!(effects.final_death_reward_slot, None);
        assert!(effects.final_death_reward_attempted);
        assert!(!effects.final_death_reward_pending);
        assert_eq!(fixture.runtime.bytes[68], 0x5a);
        assert!(!fixture.arena.entities.iter().any(|row| {
            row.active
                && row.source_id == boss_owner
                && (row.class, row.model, row.action) == (10, 9, 9)
        }));
        assert!(effects.hide);
        assert_eq!(effects.final_boss_active_counter, Some(0));
        assert!(fixture.arena.entities[usize::from(boss_slot)].drawing_disabled());
    }
}
