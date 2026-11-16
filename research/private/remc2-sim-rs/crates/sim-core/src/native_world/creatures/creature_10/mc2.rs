//! Source-row constructor and lossless MC2 adapter for class5/model10.

use super::{
    Actor, Effects, Frame, SOURCE_DESCRIPTOR, SOURCE_FEATURE_MASK, SOURCE_INITIAL_ACTION,
    PlayerPressureEffect, SOURCE_INITIAL_LIFE, SOURCE_INITIAL_MANA, SOURCE_MODEL, SOURCE_PARTICLE,
    SummonKind,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain};
use crate::{FlightWorld, Position};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncounterAction {
    Approach(u8),
    Refresh,
    KillCreatures { player: u16 },
    Pressure(PlayerPressureEffect),
    Sound { site: SoundSite, emitter: SoundEmitter, sample: u8 },
    ReadPlayer { slot: u16 },
    ReadPlayerId { slot: u16 },
    ClearFootprint { position: Position, width: u8, height: u8, owner: u16 },
    PlayerFeedback { slot: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SoundSite { Attack, Pressure, Approach, DeathRumble, DeathPeriodic }

/// Resolve the addressed source at the call, including source ID zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SoundEmitter { Slot(u16), CurrentSourceId(u16) }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncounterResponse { Done, PlayerPose { position: Position, yaw: i16 }, PlayerId(u16) }

type ApplyEncounterAction<'a> =
    dyn FnMut(&mut SourceArena, &mut SessionTerrain, &mut RuntimeSnapshot, EncounterAction)
        -> Result<EncounterResponse, String> + 'a;

type ApplyScheduledEncounterAction<'a> =
    dyn FnMut(&mut SourceArena, &mut SessionTerrain, &mut RuntimeSnapshot,
        &mut crate::native_world::Level2SharedSpellResetSession, EncounterAction)
        -> Result<EncounterResponse, String> + 'a;

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
    player_slot: u16,
    apply_world: &mut ApplyEncounterAction<'_>,
) -> Result<(), String> {
    for request in effects.source_effects.drain(..) {
        match (request.source_type, request.source_subtype) {
            (10, 17) => {
                let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
                    continue;
                };
                arena.economy.initialize_authored_model17(slot, u16::MAX, request.position);
                let EncounterResponse::PlayerId(owner) = apply_world(
                    arena, terrain, runtime, EncounterAction::ReadPlayerId { slot: player_slot },
                )? else {
                    return Err("final encounter death Meteor lost its current player ID".to_owned());
                };
                let source = arena.economy.authored_model17.iter_mut()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .ok_or("final encounter death Meteor lost its current carrier")?;
                source.position.z = 0;
                source.max_life = 70;
                source.entity_id = owner;
                source.life = 70;
                let position = source.position;
                arena.economy.source_owner.insert(slot, owner);
                crate::world::effects::sync_position(
                    arena,
                    terrain,
                    slot,
                    position,
                    true,
                    false,
                )?;
            }
            (10, 9) => {
                effects.final_death_reward_slot =
                    super::death_reward::materialize_boss_death_reward(
                        arena,
                        terrain,
                        runtime,
                        &mut effects.final_death_reward_pending,
                        request.position,
                        |arena, terrain, runtime| {
                            match apply_world(arena, terrain, runtime,
                                EncounterAction::ReadPlayerId { slot: player_slot })? {
                                EncounterResponse::PlayerId(owner) => Ok(owner),
                                _ => Err("final encounter death Volcano lost its current player ID".to_owned()),
                            }
                        },
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

    // The boss invokes ordinary sub_4CE00, retaining its3750 mana. Army's
    // zero-mana override belongs only to the spell-ring producer.
    let Some(handle) = crate::native_world::creatures::creature_25::mc2::allocate_source_default(
        arena,
        runtime,
        terrain,
        particles,
        request.position,
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
    boss_slot: u16,
    player_slot: u16,
    effects: &mut Effects,
    apply_world: &mut ApplyEncounterAction<'_>,
) -> Result<(), String> {
    for request in effects.summons.drain(..) {
        let mut projectile_succeeded = false;
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
                let projectile = boss::construct(
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
                )?;
                projectile_succeeded = projectile.is_some();
                projectile.and(request.success_sound)
            }
        };
        if projectile_succeeded {
            apply_world(arena, terrain, runtime, EncounterAction::PlayerFeedback { slot: player_slot })?;
        }
        if let Some(sound) = sound {
            // 21AB0 reads the current caller's ID only after successful
            // allocation and post-constructor writes, before the next birth.
            apply_world(arena, terrain, runtime, EncounterAction::Sound {
                site: SoundSite::Attack,
                emitter: SoundEmitter::CurrentSourceId(boss_slot),
                sample: sound,
            })?;
            effects.sounds.push(sound);
        }
    }
    Ok(())
}

/// Return the exact SEARCH.DAT bitmap class sequence consumed by `sub_10130`.
/// AddE7EE0(0, class) walks every bitmap from class zero through class in
/// order. The source loop accepts return value 1 across intermediate bitmap
/// boundaries, but omits the final pair returned with value 2.
fn source_bitmap_class_offsets(class: u8) -> Result<Vec<(i8, i8)>, String> {
    if class > 15 {
        return Err(format!(
            "final encounter SEARCH.DAT class{class} is invalid"
        ));
    }
    let mut offsets = crate::level1_mission::source_search_offsets(class);
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
            apply_world(arena, terrain, runtime, EncounterAction::Sound {
                site: SoundSite::Approach,
                emitter: SoundEmitter::Slot(actor.slot),
                sample: 10,
            })?;
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
            terrain,
            runtime,
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
            if crate::native_world::level2_wrapping_target_distance(
                frame.player_position,
                actor.position,
            ) >= 0x0a00
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
        apply_world(arena, terrain, runtime, EncounterAction::Approach(creature_mode))?;
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

/// Source 22270/222B0 runs after the reached sound call. It grounds the
/// current fixed-address occupant before reading the current player pose.
fn finish_source_tail(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    runtime: &mut RuntimeSnapshot,
    parameters: &[[u8; 34]],
    slot: u16,
    frame: Frame,
    apply_world: &mut ApplyEncounterAction<'_>,
) -> Result<(), String> {
    if let Some(kind) = arena.economy.live_arena_kind(slot) {
        if kind == crate::native_world::Level2DynamicArenaKind::AuthoredModel17 {
            let current = arena.economy.authored_model17.iter_mut()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or("final encounter tail lost its current model17")?;
            current.position.z = terrain.ground_height(current.position);
            if current.life < 10 { return Ok(()); }
            return Err("final encounter tail current model17 lacks the source facing carrier".to_owned());
        }
        return Err(format!("final encounter tail slot{slot} has unsupported current {kind:?}"));
    }
    let row = arena.entities.get_mut(usize::from(slot))
        .ok_or_else(|| format!("final encounter tail source address {slot} is absent"))?;
    let mut position = row.position;
    position.z = terrain.ground_height(position);
    row.set_position(position);
    if row.life < 10 { return Ok(()); }

    let EncounterResponse::PlayerPose { position: player, yaw: player_yaw } = apply_world(
        arena, terrain, runtime, EncounterAction::ReadPlayer { slot: frame.player_slot },
    )? else {
        return Err("final encounter tail lost its current player pose".to_owned());
    };
    let row = &mut arena.entities[usize::from(slot)];
    let bucket = (((i32::from(row.yaw) - i32::from(player_yaw)) >> 3) & 0xf0) >> 4;
    if bucket <= 2 {
        row.set_yaw(player_yaw.wrapping_add(384) & 0x07ff);
    } else if (13..=15).contains(&bucket) {
        // This is the literal decompiled +6, not an inferred +1536.
        row.set_yaw(player_yaw.wrapping_add(6) & 0x07ff);
    } else {
        let turn = match row.record[72] {
            0 => {
                row.set_source_roll(crate::heading_for_positions(row.position, player) as i16);
                true
            }
            1 => true,
            2 => {
                let side = if frame.source_frame_timing & 1 != 0 { 512_i16 } else { -512_i16 };
                row.set_source_roll(player_yaw.wrapping_add(1024).wrapping_add(side) & 0x07ff);
                row.record[72] = 1;
                true
            }
            3 => { row.set_source_roll(row.yaw); false }
            _ => false,
        };
        if turn {
            let index = row.parameter_relation
                .ok_or("final encounter facing descriptor is absent")?;
            let parameter = parameters.get(usize::from(index))
                .ok_or("final encounter facing descriptor is out of bounds")?;
            let _nominal = u16::from_le_bytes([parameter[4], parameter[5]]);
            // 58350 ignores that nominal word and clamps by unsigned word2C.
            let maximum = i16::try_from(word(row, 48).min(1024)).expect("source angle cap fits i16");
            row.set_yaw(row.yaw.wrapping_add(crate::turn_toward(row.yaw, row.source_roll(), maximum)) & 0x07ff);
        } else {
            row.set_yaw(row.yaw & 0x07ff);
        }
    }
    Ok(())
}

/// Integrator seam for the actual UpdateEntities source row. Typed state is
/// decoded for one callback and committed before returned producers enter the
/// shared same-pass tail.
pub(crate) fn step_row(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    entity_parameters: &[[u8; 34]],
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
    let source_callback = actor.action == SOURCE_INITIAL_ACTION;
    let phase_zero = source_callback && actor.phase == 0;
    let death_tail = source_callback && (12..=15).contains(&actor.phase);
    if actor.action == SOURCE_INITIAL_ACTION && actor.phase == 14 {
        // 21030 plays this before the phase-14 decrement and world effects.
        apply_world(arena, terrain, runtime, EncounterAction::Sound {
            site: SoundSite::DeathRumble,
            emitter: SoundEmitter::Slot(slot),
            sample: 10,
        })?;
    }
    #[cfg(test)]
    let frame = Frame {
        damage_trace: arena.handle(slot).map(|handle| (handle, arena.economy.events().len())),
        ..frame
    };
    let mut effects = super::step(&mut actor, frame, particles)?;
    if phase_zero {
        if !runtime.set_final_boss_active_counter(1) {
            return Err("final encounter source active-counter word is absent".to_owned());
        }
        runtime.bytes.get_mut(0x87a..0x87c)
            .ok_or("final encounter progress word0x87a is absent")?
            .copy_from_slice(&60_i16.to_le_bytes());
        // Source phase0 publishes its own fields before the burial walk.
        store(&mut row, actor);
        arena.entities[usize::from(slot)] = row.clone();
        apply_world(arena, terrain, runtime, EncounterAction::ClearFootprint {
            position: actor.position, width: 38, height: 38, owner: actor.source_id,
        })?;
    }
    if let Some(pressure) = effects.player_pressure {
        // 21AB0 clears the start bit before the world move; its phase timer
        // is written only after the movement and sound have returned.
        arena.entities[usize::from(slot)].set_subspell_index(actor.subspell_index);
        apply_world(arena, terrain, runtime, EncounterAction::Pressure(pressure))?;
        if pressure.palette_submod.is_some() {
            apply_world(arena, terrain, runtime, EncounterAction::Sound {
                site: SoundSite::Pressure,
                emitter: SoundEmitter::CurrentSourceId(slot),
                sample: 19,
            })?;
        }
    }
    if effects.kill_all_creatures {
        apply_world(
            arena,
            terrain,
            runtime,
            EncounterAction::KillCreatures {
                player: frame.player_slot,
            },
        )?;
    }
    if effects.refresh_creatures {
        apply_world(arena, terrain, runtime, EncounterAction::Refresh)?;
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
    #[cfg(test)]
    if let Some(receipt) = effects.damage_receipt.take() {
        let current = &arena.entities[usize::from(slot)];
        assert_eq!(arena.handle(slot), Some(receipt.target), "boss damage commits to the same current generation");
        assert_eq!((current.life, current.word_26(), current.source_upkeep_word_62()),
            (receipt.life_after, receipt.mailbox.1, 0),
            "actual sub22190 actor store publishes life/attacker and clears the mailbox owner");
        crate::native_world::area_damage::trace_player_damage_consumption(receipt);
    }
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    materialize_summons(arena, runtime, terrain, particles, slot, frame.player_slot, &mut effects, apply_world)?;
    materialize_source_effects(arena, runtime, terrain, &mut effects, frame.player_slot, apply_world)?;
    if effects.clear_stage_variable_count && !runtime.clear_active_stage_variable_count() {
        return Err("final encounter could not clear the active StageVar count".to_owned());
    }
    if !phase_zero && let Some(counter) = effects.final_boss_active_counter
        && !runtime.set_final_boss_active_counter(counter)
    {
        return Err("final encounter source active-counter word is absent".to_owned());
    }
    // Phase12 can allocate a typed model17 at the executing fixed address;
    // phase15 can allocate raw Volcano9 there. Read the current occupant's
    // byte3E after NewEvent, never the predecessor's captured counter.
    let current_counter = if death_tail {
        match arena.economy.live_arena_kind(slot) {
            Some(crate::native_world::Level2DynamicArenaKind::AuthoredModel17) => Some(
                arena.economy.authored_model17().iter()
                    .find(|row| row.active && row.source_entity_index == slot)
                    .ok_or("final encounter death-tail model17 lost its current carrier")?
                    .phase,
            ),
            None => Some(arena.entities[usize::from(slot)].phase),
            Some(kind) => return Err(format!(
                "final encounter death-tail slot{slot} has no current counter for {kind:?}"
            )),
        }
    } else {
        None
    };
    if current_counter.is_some_and(|counter| counter & 3 == 0) {
        // LABEL48 is after death/reward allocation and its current-address
        // writes, immediately before the source animation tail.
        apply_world(arena, terrain, runtime, EncounterAction::Sound {
            site: SoundSite::DeathPeriodic,
            emitter: SoundEmitter::Slot(slot),
            sample: 63,
        })?;
        effects.sounds.push(63);
    }
    if source_callback {
        finish_source_tail(arena, terrain, runtime, entity_parameters, slot, frame, apply_world)?;
    }
    Ok(effects)
}

/// Source sub_223E0 and sub_21F60 run immediately before action80. The
/// population lists are frozen at pass start; callbacks still use the live
/// numeric arena scan, so a higher newly allocated slot can run this pass.
fn source_player_attack_eligible(player: &EntitySnapshot) -> bool {
    // 21AB0 rejects the source null address and byte1 bit4 (0x400).
    player.index != 0 && player.life >= 0 && !player.drawing_disabled()
}

pub(crate) fn scheduled_step(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    shared_spells: &mut crate::native_world::Level2SharedSpellResetSession,
    particles: &[ParticleParameter],
    entity_parameters: &[[u8; 34]],
    animations: &crate::source_animation::SourceAnimationResidency,
    slot: u16,
    player_slot: u16,
    families: &[Vec<u16>; 29],
    class9: &[u16],
    setting_30: u8,
    source_frame_timing: u8,
    apply_world: &mut ApplyScheduledEncounterAction<'_>,
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
        player_alive: source_player_attack_eligible(player),
        player_class: player.class,
        player_model: player.model,
        source_setting: u16::from(setting_30),
        source_frame_timing,
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
    let mut scoped = |arena: &mut SourceArena, terrain: &mut SessionTerrain,
                      runtime: &mut RuntimeSnapshot, action| {
        apply_world(arena, terrain, runtime, shared_spells, action)
    };
    step_row(arena, runtime, terrain, particles, entity_parameters, slot, frame, &mut scoped)
}

#[cfg(test)]
mod tests {
    use super::super::FINAL_REWARD_PHASE;
    use super::*;
    use crate::campaign::source_arena::SourceArena;

    #[test]
    fn configured_level25_boss_model25_reaches_stage_launch_parent_expiry_and_reclaim() {
        use crate::campaign::source_arena::CreatureRootOrigin;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::level_definition::CreatureKind;
        use crate::native_world::Mc2DispatchEvent;
        use std::path::PathBuf;

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map_or_else(
            || generated.join("mc-bhq10-owner-v3-level25-20261029"),
            PathBuf::from,
        );
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map_or_else(
            || generated.join("dragon-bee-20261105-integration/current-level25-environment"),
            PathBuf::from,
        );
        let mut campaign = CampaignSession::open_with_owner_cache(
            &package,
            &cache,
            CampaignSelection::ordinary(25).unwrap(),
        )
        .expect("whole Level25 and its qualified owner cache");
        campaign.step(crate::PlayerCommand::default(), 0);
        let simulation = campaign.simulation_mut();
        // Declared upstream progression, then one configured source attack
        // branch. This does not claim natural boss RNG selection or victory.
        simulation
            .activate_authored_group_prerequisite_for_test(29)
            .unwrap();
        let boss = simulation
            .entities()
            .iter()
            .find(|row| row.active && row.class == 5 && row.model == 10 && !row.drawing_disabled())
            .expect("actual group29 boss constructor")
            .index;
        let parent = simulation.source_arena().handle(boss).unwrap();
        let player = simulation.state.entity_index;
        let owner = simulation.entities()[usize::from(boss)].source_id;
        assert!(
            simulation.source_arena().rebuilt_class5_model_slots()[25].len() < 6,
            "configured mode5 is below the source selector population cap"
        );
        let mut approach = simulation.entities()[usize::from(boss)].position;
        crate::move_entity(&mut approach, 1024, 0, 4096);
        approach.z = simulation
            .canonical_terrain()
            .ground_height(approach)
            .wrapping_add(768);
        simulation
            .set_level2_entity_position(player, approach)
            .unwrap();
        simulation.state.actual_speed = 0;
        simulation.state.target_speed = 0;
        {
            // Fixed player survival budget; no later replenishment.
            let live = &mut simulation.session.state.live;
            let player_row = &mut live.economy.entities[usize::from(player)];
            player_row.set_max_life(200_000);
            player_row.set_life(200_000);
            live.player_life = player_row.life;
            live.shared_spells
                .sync_player_entity_from_authority(player_row);
            // Exact sub21AB0 mode5 entry: one pending source attempt. No
            // child, constructor result, RNG or child lifetime is supplied.
            let row = &mut live.economy.entities[usize::from(boss)];
            row.set_source_byte_46(9);
            row.record[71] = 5;
            put_word(row, 40, 1);
            row.set_source_dword_10(1);
        }
        let counter = simulation.runtime().constructor_counters().unwrap()[25];
        // Isolated call-input controls prove source ID is dereferenced as an
        // address, including silence and source0. They do not claim these
        // identity variants occur in the unmodified group29 producer.
        for (emitter, muted) in [(player, false), (player, true), (0, false)] {
            let mut control = simulation.clone();
            let journal_start = control.effect_journal().records().len();
            let live = &mut control.session.state.live;
            let caller = &mut live.economy.entities[usize::from(boss)];
            caller.source_id = emitter;
            caller.record[30..32].copy_from_slice(&emitter.to_le_bytes());
            let listener = &mut live.economy.entities[usize::from(player)];
            listener.set_control_flags(if muted {
                listener.control_flags | 0x80
            } else {
                listener.control_flags & !0x80
            });
            control.step_level2_source_pass(0);
            let sound = control.effect_journal().records()[journal_start..]
                .iter()
                .find(|record| {
                    matches!(record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                        audio_site: crate::MissionAudioSite::BossCymmerianSummon,
                        entity_index, player_index: -1, sound_id: 37,
                    }) if entity_index == emitter)
                })
                .expect("the successful real mode5 constructor publishes its addressed sound call");
            if muted {
                assert!(
                    sound.audio.is_none(),
                    "current addressed emitter's silence flag suppresses playback"
                );
            } else {
                let audio = sound
                    .audio
                    .expect("null/player emitter reaches native prepared audio");
                assert_eq!(audio.play_type, 3);
                assert!(audio.command.audible);
                assert_eq!(
                    (
                        audio.command.relative_x,
                        audio.command.relative_y,
                        audio.command.relative_z
                    ),
                    (0, 0, 0)
                );
                assert_eq!(
                    (
                        audio.command.volume_q15,
                        audio.command.pan_q16,
                        audio.command.rate_delta_percent
                    ),
                    (0x7fff, 0x7fff, 0)
                );
                let flags = if emitter == 0 {
                    0
                } else {
                    control.entities()[usize::from(player)]
                        .source_id
                        .cast_signed()
                };
                assert_eq!(audio.command.sound_flags, flags);
            }
        }
        let journal_start = simulation.effect_journal().records().len();
        simulation.step_level2_source_pass(0);
        assert!(
            simulation
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let child = simulation
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Cymmerian)
            .find(|entry| {
                entry.origin
                    == CreatureRootOrigin::CreatureConversion {
                        producer_slot: boss,
                    }
                    && simulation
                        .source_arena()
                        .creature_parent_binding(entry.handle)
                        == Some(parent)
            })
            .expect("real boss mode5 constructs and binds its model25")
            .handle;
        let birth = simulation.entities()[usize::from(child.source_slot)].clone();
        assert_eq!(
            (birth.class, birth.model, birth.action, birth.record[77]),
            (5, 25, 207, 17)
        );
        assert_eq!(
            (birth.max_life, birth.life, birth.mana, birth.max_mana),
            (7500, 7500, 3750, 0),
            "boss preserves ordinary4CE00 mana; Army alone overrides it to zero"
        );
        assert_eq!(
            (word(&birth, 44), word(&birth, 50), word(&birth, 154)),
            (boss, 250, player)
        );
        assert_eq!(birth.source_id, owner);
        assert_eq!(
            simulation.runtime().constructor_counters().unwrap()[25],
            counter.wrapping_add(1)
        );
        let birth_visit = u8::from(child.source_slot > boss);
        assert_eq!(
            (birth.phase, birth.actual_speed),
            (
                counter.wrapping_add(birth_visit),
                320 - 8 * i16::from(birth_visit)
            ),
            "a higher source slot executes in its birth pass; a lower slot waits"
        );
        assert!(
            simulation
                .campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .any(|view| view.entity_index == child.source_slot)
        );
        let summon_sound = simulation.effect_journal().records()[journal_start..]
            .iter()
            .find(|record| {
                matches!(record.effect,
                crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::BossCymmerianSummon,
                    entity_index, player_index: -1, sound_id: 37,
                }) if entity_index == owner)
            })
            .expect(
                "successful source allocation reaches its current source-ID prepared sound consumer",
            );
        let prepared = summon_sound
            .audio
            .expect("actual group29 emitter is not silent");
        assert_eq!(prepared.play_type, 3);
        assert!(prepared.command.audible);
        let emitter = crate::native_world::source_target::read(
            simulation.source_arena(),
            crate::native_world::source_target::Context::default(),
            owner,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            prepared.command.sound_flags,
            emitter.source_id.cast_signed()
        );
        assert_eq!(prepared.command.rate_delta_percent, 0);

        // Source1E320 subtracts8 from320 until <=16, after ordinary movement.
        // Step the actual shared scheduler, preserving all source tails.
        let launch_bound = (320_usize - 16) / 8 + 1;
        let mut transitioned = false;
        let mut moved = false;
        for visit in 0..launch_bound {
            let before = simulation.entities()[usize::from(child.source_slot)].clone();
            assert_eq!(before.record[77], 17);
            simulation.step_level2_source_pass(0);
            assert_eq!(
                simulation.source_arena().handle(child.source_slot),
                Some(child)
            );
            let after = &simulation.entities()[usize::from(child.source_slot)];
            assert!(
                after.life >= 0 && !after.drawing_disabled(),
                "source launch must survive its real terrain/target: visit{visit} before={before:?} after={after:?}"
            );
            assert_eq!(after.phase, before.phase.wrapping_add(1));
            assert_eq!(word(after, 50), 250);
            assert_eq!(after.actual_speed, before.actual_speed.wrapping_sub(8));
            moved |= before.position != after.position;
            if after.record[77] == 16 {
                assert_eq!(after.actual_speed, 16);
                assert_eq!(after.action, 207);
                transitioned = true;
                break;
            }
            assert_eq!(after.record[77], 17);
        }
        assert!(
            transitioned && moved,
            "actual launch motion reaches source mode16"
        );
        let before = simulation.entities()[usize::from(child.source_slot)].clone();
        let mut alive = simulation.clone();
        alive.step_level2_source_pass(0);
        assert_eq!(alive.source_arena().handle(child.source_slot), Some(child));
        assert!(
            !alive.entities()[usize::from(child.source_slot)].drawing_disabled(),
            "the same scheduled child remains visible with its current live parent"
        );

        // Isolated configured current-parent-death branch, not earned boss
        // victory. The original child's life/timer/action/pose remain intact.
        assert!(simulation.economy().free_slot_count() > 1);
        let generations = simulation
            .entities()
            .iter()
            .map(|row| simulation.source_arena().slot_generation(row.index))
            .collect::<Vec<_>>();
        simulation.session.state.live.economy.entities[usize::from(boss)].set_life(-1);
        simulation.step_level2_source_pass(0);
        assert_eq!(
            simulation.source_arena().handle(child.source_slot),
            Some(child)
        );
        let expired = &simulation.entities()[usize::from(child.source_slot)];
        assert!(expired.drawing_disabled());
        assert_eq!(
            (expired.life, expired.mana, expired.position),
            (before.life, before.mana, before.position)
        );
        assert_eq!(
            (expired.action, expired.record[77], word(expired, 50)),
            (207, 16, 0)
        );
        assert!(
            simulation.entities().iter().any(|row| simulation
                .source_arena()
                .slot_generation(row.index)
                != generations[usize::from(row.index)]
                && row.active
                && (row.class, row.model) == (10, 0)
                && row.position == before.position),
            "mode16 parent expiry allocates the actual10/0 product at the original current pose"
        );
        assert!(
            !simulation
                .campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .any(|view| view.entity_index == child.source_slot)
        );
        simulation.step_level2_source_pass(0);
        assert!(
            simulation.source_arena().resolve(child).is_none(),
            "following source boundary reclaims the retained original generation"
        );
        assert!(
            !simulation.source_arena().rebuilt_class5_model_slots()[25].contains(&child.source_slot)
        );
    }

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
    fn level25_reset_absence_chain_produces_original_boss_and_real_exit() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::campaign::source_arena::SourceEntityHandle;
        use crate::native_world::{Mc2DispatchEvent, area_damage, directional_spells};
        use std::collections::{BTreeMap, BTreeSet};
        use std::path::PathBuf;

        fn tick(campaign: &mut CampaignSession, groups: &mut BTreeMap<u16, usize>, field: &mut Option<(SourceEntityHandle, bool)>) {
            campaign.step(crate::PlayerCommand::default(), 0);
            let sim = campaign.simulation();
            let player = sim.state.entity_index;
            let actor = &sim.entities()[usize::from(player)];
            let boss = sim.entities().iter().find(|row| row.active && (row.class, row.model) == (5, 10))
                .map(|row| (row.index, row.record[74], row.subspell_index, row.source_dword_10(), row.life));
            assert!(actor.life >= 0,
                "Uluth player must remain alive: position={:?} life={} killer={} attacker={} pending=({}, {}) boss={boss:?} groups={groups:?}",
                sim.state.position, actor.life, actor.word_24(), actor.word_26(),
                actor.source_upkeep_payload(), actor.source_upkeep_word_62());
            for event in campaign.simulation().dispatch_events() {
                match event {
                    Mc2DispatchEvent::Error { .. } => panic!("Uluth dispatch: {event:?}"),
                    Mc2DispatchEvent::AuthoredGroupActivated { group: 7, authored_indices, .. }
                        if authored_indices.contains(&217) => {
                            let sim = campaign.simulation();
                            let row = sim.entities().iter().find(|row| row.active
                                && (row.class, row.model, row.action) == (10, 25, 25)
                                && (row.position.x, row.position.y) == (125 * 256 + 128, 114 * 256 + 128))
                                .expect("actual group7 row217 field");
                            assert_eq!((row.source_id, row.record[74]), (row.index, 0));
                            assert!(field.replace((sim.source_arena().handle(row.index).unwrap(), false)).is_none(),
                                "the consumed row217 publishes exactly once");
                        }
                    Mc2DispatchEvent::AuthoredGroupActivated { group, authored_indices, .. }
                        if matches!(*group, 2 | 3 | 12 | 25 | 28 | 29 | 31) => {
                            let required = match *group {
                                2 => 43, 3 => 120, 12 => 266, 25 => 368,
                                28 => 373, 29 => 379, 31 => 394, _ => unreachable!(),
                            };
                            assert!(authored_indices.contains(&required), "group{group}: {authored_indices:?}");
                            *groups.entry(*group).or_default() += 1;
                        }
                    _ => {}
                }
            }
            if let Some((handle, hidden)) = field
                && campaign.simulation().source_arena().handle(handle.source_slot) == Some(*handle)
            {
                *hidden |= campaign.simulation().entities()[usize::from(handle.source_slot)].drawing_disabled();
            }
        }
        fn contact(campaign: &mut CampaignSession, player: u16, slot: u16) {
            let live = &mut campaign.simulation_mut().session.state.live;
            area_damage::contact(&mut live.economy,
                Some(area_damage::ActorDomains::new(&mut live.goats, &mut live.ordinary)),
                slot, u16::MAX, player).unwrap();
        }
        fn count(groups: &BTreeMap<u16, usize>, group: u16) -> usize {
            groups.get(&group).copied().unwrap_or(0)
        }

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map_or_else(
            || generated.join("mc-bhq10-owner-v3-level25-20261029"), PathBuf::from);
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map_or_else(
            || generated.join("dragon-bee-20261105-integration/current-level25-environment"), PathBuf::from);
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache).unwrap();
        let mut campaign = CampaignSession::open_with_owner_cache(
            &package, &cache, CampaignSelection::ordinary(25).unwrap()).unwrap();
        let player = campaign.simulation().state.entity_index;
        let mut groups = BTreeMap::new();
        let mut field = None;
        directional_spells::start_steal_mana_lane_trace();
        tick(&mut campaign, &mut groups, &mut field);
        // Declare a terrain-admitted player retreat near the authored exit,
        // away from the source encounter while its live populations advance.
        let terrain = campaign.simulation().canonical_terrain();
        let mut retreat = (216_u16..240).flat_map(|y| (216_u16..240).map(move |x|
            Position { x: x * 256, y: y * 256, z: 0 }))
            .find(|position| terrain.material_mask(*position) != crate::MOVEMENT_REDIRECT_MASK
                && terrain.ceiling_height(*position).is_none_or(|ceiling|
                    i32::from(ceiling) > i32::from(terrain.ground_height(*position)) + 768))
            .expect("Uluth source terrain admits a retreat near the authored exit");
        retreat.z = terrain.ground_height(retreat).wrapping_add(768);
        campaign.simulation_mut().set_level2_entity_position(player, retreat).unwrap();
        // After group3 exists, visit this source position inside both row62's
        // area and row217's field, then retreat when the real field reclaims.
        let mut field_position = Position { x: 125 * 256 + 128, y: 114 * 256 + 128, z: 0 };
        field_position.z = campaign.simulation().canonical_terrain().ground_height(field_position);
        let fireflies = campaign.simulation().creature_runtime_census()
            .roots_of_kind(crate::level_definition::CreatureKind::Firefly)
            .filter(|row| row.living).map(|row| row.handle)
            .collect::<Vec<_>>();
        assert_eq!(fireflies.len(), 7, "actual reset Fireflies gate group2");
        for handle in &fireflies[1..] { contact(&mut campaign, player, handle.source_slot); }
        for _ in 0..64 { tick(&mut campaign, &mut groups, &mut field); }
        assert_eq!(count(&groups, 2), 0, "one actual Firefly blocks row15");
        assert!(campaign.simulation().creature_identity_for_source_slot(fireflies[0].source_slot)
            .is_some_and(|row| row.handle == fireflies[0] && row.living),
            "the original held Firefly is still living in the source census");
        contact(&mut campaign, player, fireflies[0].source_slot);

        // Only source-managed population models0..11 and16 block selector30.
        // Each observed generation receives one declared arrived contact;
        // unrelated living models, their AI, and the source countdown remain live.
        let mut struck = BTreeSet::new();
        let mut field_arrival = false;
        let mut field_complete = false;
        for _ in 0..4096 {
            if count(&groups, 28) == 1 && field_complete { break; }
            let sim = campaign.simulation();
            let buckets = sim.source_arena().rebuilt_class5_model_slots();
            let targets = (0..=11).chain(std::iter::once(16))
                .flat_map(|model| buckets[model].iter().copied())
                .filter_map(|slot| sim.source_arena().handle(slot)).collect::<Vec<_>>();
            for handle in targets {
                if struck.insert((handle.source_slot, handle.generation)) {
                    contact(&mut campaign, player, handle.source_slot);
                }
            }
            tick(&mut campaign, &mut groups, &mut field);
            if count(&groups, 3) == 1 && !field_arrival {
                campaign.simulation_mut().set_level2_entity_position(player, field_position).unwrap();
                field_arrival = true;
            }
            if !field_complete && let Some((handle, true)) = field
                && campaign.simulation().source_arena().resolve(handle).is_none()
            {
                let lanes = directional_spells::take_steal_mana_lane_trace();
                assert!(lanes.contains(&(false, player, handle.source_slot, 0))
                    && lanes.contains(&(true, player, handle.source_slot, 0)),
                    "row217's real source field must write and the current player must consume its Steal Mana lane: {lanes:?}");
                campaign.simulation_mut().set_level2_entity_position(player, retreat).unwrap();
                field_complete = true;
            }
        }
        assert!([2, 3, 12, 25, 28].into_iter().all(|group| count(&groups, group) == 1),
            "actual absence chain must publish in source order: {groups:?}");
        let (field, field_hidden) = field.expect("the real area switch publishes row217");
        assert!(field_complete && field_hidden && campaign.simulation().source_arena().resolve(field).is_none(),
            "the original row217 field must hide and reclaim naturally");
        eprintln!("Uluth reset absence chain: {groups:?}");

        // Keep the now-reclaimed handle in the tick observer without allowing
        // an unrelated future field to replace the original receipt.
        let mut field = Some((field, field_hidden));

        // The actual group28 BUILD68 advances through the source successor
        // chain. Contacts are only admitted to live constructed Houses.
        for selector in [68, 69, 72, 73] {
            let mut finished = false;
            for _ in 0..512 {
                let house = campaign.simulation().ordinary().houses().iter()
                    .find(|house| house.active && house.building_index == selector).copied();
                if let Some(house) = house {
                    if house.action == crate::house::HOUSE_LIVE_ACTION {
                        assert!(campaign.simulation_mut().session.state.live.ordinary.houses
                            .submit_source_damage(house.entity_index, u16::MAX, player));
                    }
                } else if selector == 73 && count(&groups, 29) == 1 {
                    finished = true; break;
                } else if campaign.simulation().ordinary().houses().iter().any(|house|
                    house.active && house.building_index == match selector { 68 => 69, 69 => 72, 72 => 73, _ => 0 }) {
                    finished = true; break;
                }
                tick(&mut campaign, &mut groups, &mut field);
            }
            assert!(finished, "actual BUILD{selector} must construct and remove: {groups:?}");
        }
        assert_eq!(count(&groups, 29), 1);
        let sim = campaign.simulation();
        let boss_slot = sim.entities().iter().find(|row| row.active && (row.class, row.model) == (5, 10)
            && !row.drawing_disabled()).expect("actual group29 boss").index;
        let boss = sim.source_arena().handle(boss_slot).unwrap();
        assert_eq!((sim.entities()[usize::from(boss_slot)].life, sim.entities()[usize::from(boss_slot)].max_life),
            (300_000, 300_000));
        let mut approach = sim.entities()[usize::from(boss_slot)].position;
        crate::move_entity(&mut approach, sim.state.yaw.wrapping_add(1024) & 2047, 0, 2048);
        approach.z = sim.canonical_terrain().ground_height(approach).wrapping_add(768);
        campaign.simulation_mut().set_level2_entity_position(player, approach).unwrap();
        let mut ready = false;
        let mut approach_samples = Vec::new();
        let mut terrain_samples = Vec::new();
        let scan_offsets = source_bitmap_class_offsets(7).unwrap();
        let (center_x, center_y) = source_tile_center(campaign.simulation().entities()[usize::from(boss_slot)].position);
        let lowered = (0..=15).flat_map(|class| source_bitmap_class_offsets(class).unwrap())
            .map(|(dx, dy)| (center_x.wrapping_add(i16::from(dx)) as u8,
                center_y.wrapping_add(i16::from(dy)) as u8)).collect::<BTreeSet<_>>();
        let watched = scan_offsets.iter().flat_map(|&(dx, dy)| {
            (-1_i16..=1).flat_map(move |ny| (-1_i16..=1).map(move |nx|
                (center_x.wrapping_add(i16::from(dx)).wrapping_add(nx) as u8,
                 center_y.wrapping_add(i16::from(dy)).wrapping_add(ny) as u8)))
        }).filter(|tile| lowered.contains(tile)).collect::<BTreeSet<_>>();
        let planes = campaign.simulation().canonical_terrain().planes();
        let max_height = watched.iter().map(|&(x, y)| planes.ground_heights[usize::from(x) + 256 * usize::from(y)])
            .max().unwrap_or(0);
        // A complete source retry visits each admitted SEARCH class once in
        // 17 turns. Include one partial entry cycle, topology settling and
        // the real post-clear approach, with no terrain or timer edits.
        let approach_bound = (usize::from(max_height) + 3) * 17 + 192;
        let mut previous_retry_terrain = None;
        let mut stalled_retries = 0;
        let mut previous_mode = None;
        for turn in 0..approach_bound {
            tick(&mut campaign, &mut groups, &mut field);
            // sub_21490 modes8/4 do not read primary visibility. Mode10
            // clears the prior bit on entry and then consumes fresh frames.
            let needs_visibility = {
                let row = &campaign.simulation().entities()[usize::from(boss_slot)];
                row.record[74] != 1 || row.subspell_index & 0x10 != 0
            };
            let visibility = if needs_visibility {
                let generation = campaign.generation();
                let frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
                    campaign.inspection(), campaign.simulation(), &assets, generation).unwrap();
                frame.apply_primary_visibility(campaign.simulation_mut(), generation).unwrap()
            } else { Vec::new() };
            let sim = campaign.simulation();
            let row = &sim.entities()[usize::from(boss_slot)];
            assert_eq!(sim.source_arena().handle(boss_slot), Some(boss));
            if row.subspell_index & 8 != 0 && row.source_dword_10() == -1 {
                let planes = sim.canonical_terrain().planes();
                let blocked = scan_offsets.iter().any(|&(dx, dy)| {
                    let x = center_x.wrapping_add(i16::from(dx)) as u8;
                    let y = center_y.wrapping_add(i16::from(dy)) as u8;
                    planes.terrain_types[usize::from(x) + 256 * usize::from(y)] != 0
                });
                let terrain_state = watched.iter().map(|&(x, y)| {
                    let index = usize::from(x) + 256 * usize::from(y);
                    (planes.ground_heights[index], planes.angles[index] & 7, planes.terrain_types[index])
                }).collect::<Vec<_>>();
                let height_sum = terrain_state.iter().map(|row| u32::from(row.0)).sum::<u32>();
                if blocked {
                    if previous_retry_terrain.as_ref() == Some(&terrain_state) {
                        stalled_retries += 1;
                    } else { stalled_retries = 0; }
                    assert!(stalled_retries < 2,
                        "source class7 remains blocked with unchanged height/topology across two complete retries: height_sum={height_sum}, samples={terrain_samples:?}");
                    previous_retry_terrain = Some(terrain_state);
                }
            }
            if turn % 64 == 0 {
                let (center_x, center_y) = source_tile_center(row.position);
                let planes = sim.canonical_terrain().planes();
                let blocked = scan_offsets.iter().filter_map(|&(dx, dy)| {
                    let x = center_x.wrapping_add(i16::from(dx)) as u8;
                    let y = center_y.wrapping_add(i16::from(dy)) as u8;
                    let index = usize::from(x) + 256 * usize::from(y);
                    (planes.terrain_types[index] != 0).then_some((dx, dy,
                        planes.terrain_types[index], planes.ground_heights[index], planes.angles[index]))
                }).collect::<Vec<_>>();
                terrain_samples.push((turn, blocked.len(), blocked.iter().map(|row| row.3).max(),
                    blocked.into_iter().take(8).collect::<Vec<_>>()));
            }
            let mode = (row.action, row.record[74], row.subspell_index);
            if turn % 32 == 0 || previous_mode != Some(mode) {
                approach_samples.push((turn, mode, row.source_dword_10(), row.record[61], row.position,
                    sim.entities()[usize::from(player)].life, sim.state.position,
                    (sim.state.actual_speed, sim.state.target_speed, sim.state.recoil_speed),
                    visibility.iter().any(|(slot, _)| *slot == boss_slot)));
                previous_mode = Some(mode);
            }
            if row.action == 80 && (2..12).contains(&row.record[74]) && row.record[61] != 0 {
                ready = true; break;
            }
        }
        assert!(ready, "the same naturally produced boss reaches its real damage window: bound={approach_bound}, initial_max_height={max_height}, {approach_samples:?}; class7 blocked terrain {terrain_samples:?}");
        // The real initial approach is now complete. As in the independently
        // paid encounter, use one disclosed westward player retreat outside
        // the source phase3 8192-unit attack radius; no NPC state is changed.
        let sim = campaign.simulation();
        let target = &sim.entities()[usize::from(boss_slot)];
        let parent = &sim.entities()[usize::from(player)];
        assert!(parent.life >= 0, "the original player survived the natural approach");
        let mut combat_retreat = Position { x: target.position.x.wrapping_sub(8256),
            y: target.position.y, z: 0 };
        let terrain = sim.canonical_terrain();
        assert_ne!(terrain.material_mask(combat_retreat), crate::MOVEMENT_REDIRECT_MASK,
            "the fixed westward source position admits player movement");
        let descriptor_clearance = parent.parameter_relation
            .and_then(|index| sim.session.definition.presentation.entity_parameters.get(usize::from(index)))
            .map_or(0, |row| i16::from_le_bytes([row[12], row[13]]));
        let clearance = sim.profile().ground_clearance.max(descriptor_clearance);
        combat_retreat.z = terrain.ground_height(combat_retreat).checked_add(clearance)
            .expect("source-valid player altitude");
        assert!(terrain.ceiling_height(combat_retreat).is_none_or(|ceiling| ceiling > combat_retreat.z));
        assert!(crate::native_world::level2_wrapping_target_distance(combat_retreat, target.position) >= 8192);
        campaign.simulation_mut().set_level2_entity_position(player, combat_retreat).unwrap();
        assert!(campaign.simulation().entities()[usize::from(player)].life >= 0,
            "the declared retreat preserves the live source player");
        area_damage::start_actor_damage_trace(&[boss]);
        let mut contact_writes = BTreeSet::new();
        let mut admitted_contact = false;
        let mut terminal_consumption = false;
        let mut phases = BTreeSet::new();
        for _ in 0..8192 {
            let sim = campaign.simulation();
            if sim.source_arena().resolve(boss).is_none() { break; }
            let row = &sim.entities()[usize::from(boss_slot)];
            phases.insert(row.record[74]);
            if row.action == 80 && (2..12).contains(&row.record[74]) && row.record[61] != 0
                && row.life >= 10 && row.source_upkeep_word_62() == 0 {
                let expected_payload = row.source_upkeep_payload().wrapping_add(i32::from(u16::MAX));
                contact(&mut campaign, player, boss_slot);
                let write = area_damage::latest_player_mailbox_writer()
                    .expect("the selected contact writes the watched boss mailbox");
                assert_eq!((write.target, write.mailbox), (boss, (expected_payload, player)));
                contact_writes.insert(write.sequence);
            }
            tick(&mut campaign, &mut groups, &mut field);
            for receipt in area_damage::take_player_damage_consumptions() {
                assert_eq!(receipt.target, boss);
                let after = receipt.life_before.wrapping_sub(receipt.mailbox.0.clamp(1, 300));
                assert_eq!(receipt.life_after, if after < 10 { 8 } else { after },
                    "the real boss callback applies the source damage cap and terminal clamp");
                if let Some(writer) = receipt.writer && contact_writes.contains(&writer.sequence) {
                    assert_eq!((writer.target, writer.mailbox, receipt.mailbox.1),
                        (boss, receipt.mailbox, player));
                    admitted_contact = true;
                }
                terminal_consumption |= receipt.life_after == 8;
            }
        }
        area_damage::start_actor_damage_trace(&[]);
        assert!(campaign.simulation().source_arena().resolve(boss).is_none(),
            "original boss must die and reclaim: contacts={} phases={phases:?}", contact_writes.len());
        assert!(admitted_contact && terminal_consumption
            && [13, 14, 15].into_iter().all(|phase| phases.contains(&phase)),
            "the original boss must consume a selected contact, reach terminal life8, and visit its real death phases");
        assert_eq!(campaign.simulation().runtime().objectives[0].stage_selectors[1], 2);
        if campaign.simulation().runtime().objectives[0].stage_selectors[0] == 1 {
            let mut point = Position { x: 40 * 256, y: 212 * 256, z: 0 };
            point.z = campaign.simulation().canonical_terrain().ground_height(point).wrapping_add(768);
            campaign.simulation_mut().set_level2_entity_position(player, point).unwrap();
        }
        for _ in 0..128 {
            if count(&groups, 31) == 1 { break; }
            tick(&mut campaign, &mut groups, &mut field);
        }
        assert_eq!(&campaign.simulation().runtime().objectives[0].stage_selectors[..2], &[2, 2]);
        assert_eq!(count(&groups, 31), 1);
        eprintln!("Uluth same boss {boss:?} natural death: contacts={} phases={phases:?}", contact_writes.len());
        assert_final_exit_consumer(campaign, &package, &cache, player, false);
    }

    #[test]
    fn level25_early_slot_exit_reaches_results_after_natural_boss_approach() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::Mc2DispatchEvent;
        use std::path::PathBuf;

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map_or_else(
            || generated.join("mc-bhq10-owner-v3-level25-20261029"), PathBuf::from);
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map_or_else(
            || generated.join("dragon-bee-20261105-integration/current-level25-environment"), PathBuf::from);
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache).unwrap();
        let mut campaign = CampaignSession::open_with_owner_cache(
            &package, &cache, CampaignSelection::ordinary(25).unwrap(),
        ).expect("actual Level25 package and owner cache");
        campaign.step(crate::PlayerCommand::default(), 0);
        // Configured prior group29 and group31 boundaries isolate the real
        // exit consumer. This is not objective completion or a boss defeat.
        campaign.simulation_mut().activate_authored_group_prerequisite_for_test(29)
            .expect("actual boss producer");
        let sim = campaign.simulation();
        let boss = sim.entities().iter().find(|row|
            row.active && (row.class, row.model) == (5, 10) && !row.drawing_disabled())
            .expect("authored boss").index;
        let player = sim.state.entity_index;
        let original_creatures = sim.entities().iter().filter(|row|
            row.active && row.class == 5 && row.index < player && row.index != boss)
            .filter_map(|row| sim.source_arena().handle(row.index)).collect::<Vec<_>>();
        assert!(!original_creatures.is_empty());
        let mut approach = sim.entities()[usize::from(boss)].position;
        crate::move_entity(&mut approach, sim.state.yaw.wrapping_add(1024) & 2047, 0, 2048);
        approach.z = sim.canonical_terrain().ground_height(approach).wrapping_add(768);
        let sim = campaign.simulation_mut();
        sim.set_level2_entity_position(player, approach).unwrap();
        sim.state.actual_speed = 0;
        sim.state.target_speed = 0;
        {
            // Match the paid encounter's disclosed player survival budget.
            let live = &mut sim.session.state.live;
            let row = &mut live.economy.entities[usize::from(player)];
            row.set_max_life(200_000);
            row.set_life(200_000);
            let row = row.clone();
            live.player_life = row.life;
            live.shared_spells.sync_player_entity_from_authority(&row);
        }
        let mut ready = false;
        let mut saw_clear_window = false;
        for turn in 0..320 {
            campaign.step(crate::PlayerCommand::default(), 0);
            let generation = campaign.generation();
            let frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
                campaign.inspection(), campaign.simulation(), &assets, generation,
            ).expect("actual boss approach frame");
            frame.apply_primary_visibility(campaign.simulation_mut(), generation).unwrap();
            let sim = campaign.simulation();
            assert!(!sim.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                "approach turn{turn}: {:?}", sim.dispatch_events());
            let row = &sim.entities()[usize::from(boss)];
            saw_clear_window |= row.subspell_index & 4 != 0;
            if row.action == 80 && (2..12).contains(&row.record[74]) && row.record[61] != 0 {
                ready = true;
                break;
            }
        }
        assert!(ready, "ordinary boss approach must complete before exit setup");
        assert!(saw_clear_window && original_creatures.iter().any(|handle|
            campaign.simulation().source_arena().resolve(*handle).is_none()),
            "unmodified source approach must clear and reclaim an original lower-slot creature");
        campaign.simulation_mut().activate_authored_group_prerequisite_for_test(31)
            .expect("actual ordered group31 constructors");
        assert_final_exit_consumer(campaign, &package, &cache, player, false);
    }

    #[test]
    fn level25_paid_meteor_reaches_original_boss_damage_and_cleanup() {
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{area_damage, Mc2DispatchEvent};
        use std::path::PathBuf;

        let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let package = std::env::var_os("MC_LEVEL25_PACKAGE").map_or_else(
            || generated.join("mc-bhq10-owner-v3-level25-20261029"), PathBuf::from);
        let cache = std::env::var_os("MC_LEVEL25_ENVIRONMENT").map_or_else(
            || generated.join("dragon-bee-20261105-integration/current-level25-environment"), PathBuf::from);
        let assets = crate::day_indexed_frame::DayIndexedFrameAssets::load(&cache)
            .expect("qualified Level25 environment assets");
        let mut campaign = CampaignSession::open_with_owner_cache(
            &package, &cache, CampaignSelection::ordinary(25).unwrap(),
        ).expect("actual Level25 owner package and environment");
        campaign.step(crate::PlayerCommand::default(), 0);
        let player = campaign.simulation().state.entity_index;
        let exit_root = campaign.simulation().entities().iter().find(|row|
            row.active && (row.class, row.model, row.action, row.source_id) == (11, 4, 4, 31))
            .expect("reset-born final mission group31 controller").index;
        let exit_root_handle = campaign.simulation().source_arena().handle(exit_root).unwrap();
        let objective_stages = &campaign.simulation().runtime().stages;
        assert_eq!((objective_stages[0].kind, objective_stages[1].kind), (5, 1));
        let group31_rows = campaign.simulation().mission().authored_features().iter()
            .filter(|row| row.kind != 0 && row.activation_group == 31)
            .map(|row| row.index).collect::<Vec<_>>();
        assert_eq!(group31_rows, [393, 394]);
        {
            // Declared prior carry and resources. The player pays the real
            // holder; no boss state, phase, life, RNG or timer is changed.
            let sim = campaign.simulation_mut();
            sim.apply_carried_spell_families(&[9], Some(9)).unwrap();
            sim.apply_carried_spell_progression(&[crate::native_world::CarriedSpellProgress {
                family: 9,
                persistent_experience: crate::native_world::LEVEL2_SPELL_TIER_THRESHOLDS[9][1],
                runtime_experience: 0, unlocked_tier: 1, selected_tier: 1,
            }]).unwrap();
            for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
                sim.economy_mut().set_castle_stage_footprint(stage, footprint);
            }
            let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x|
                Position { x: x * 2048, y: y * 2048, z: 0 }))
                .find(|position| !sim.economy().castle_projectile_site_blocked(
                    *position, sim.entities(), sim.canonical_terrain()))
                .expect("source-valid declared Castle site");
            let castle = sim.economy_mut().build_castle(site, player).unwrap();
            assert!(sim.economy_mut().set_castle_mana_prerequisite_for_test(castle, 200_000));
            let live = &mut sim.session.state.live;
            let row = &mut live.economy.entities[usize::from(player)];
            row.set_max_life(200_000);
            row.set_life(200_000);
            row.max_mana = 200_000;
            row.record[144..148].copy_from_slice(&200_000_i32.to_le_bytes());
            row.set_mana(200_000);
            row.set_mana_regen(0);
            let row = row.clone();
            live.player_life = row.life;
            live.shared_spells.sync_player_entity_from_authority(&row);
            live.shared_spells.player_mana = row.mana;
            live.shared_spells.caster.mana = row.mana;
            live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
            if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = row.mana; }
        }
        let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 9).unwrap() as i8;
        campaign.step(crate::PlayerCommand { opcode: 31, argument_1: ui, argument_2: 1,
            ..Default::default() }, 0);
        assert!(campaign.simulation().dispatch_events().iter().all(|event|
            !matches!(event, Mc2DispatchEvent::Error { .. })));
        let sim = campaign.simulation();
        let holder = sim.shared_spells().local_player().enabled_spell_holders[9];
        assert_ne!(holder, 0);
        assert_eq!(sim.shared_spells().pickup(holder).unwrap().level_gate, 1);
        assert!(!sim.shared_spells().free_spell_usage);
        let holder_handle = sim.source_arena().handle(holder).unwrap();
        let caster_handle = sim.source_arena().handle(player).unwrap();
        // As in the existing natural Firefly encounter, this declared prior
        // boundary activates the original group29, not a claimed BUILD kill.
        campaign.simulation_mut().activate_authored_group_prerequisite_for_test(29)
            .expect("source group29 admits the actual authored boss");
        let sim = campaign.simulation();
        let boss_slot = sim.entities().iter().find(|row| row.active && row.class == 5 && row.model == 10
            && !row.drawing_disabled()).expect("original source24 boss").index;
        let boss = sim.source_arena().handle(boss_slot).unwrap();
        assert_eq!((sim.entities()[usize::from(boss_slot)].life,
            sim.entities()[usize::from(boss_slot)].max_life), (300_000, 300_000));
        // Real sub21490 approach and primary-visibility feedback are needed
        // to reach the ordinary damageable phase. This is the same single
        // declared player-only lure as the natural Firefly encounter.
        let mut approach = sim.entities()[usize::from(boss_slot)].position;
        crate::move_entity(&mut approach, sim.state.yaw.wrapping_add(1024) & 2047, 0, 2048);
        approach.z = sim.canonical_terrain().ground_height(approach).wrapping_add(768);
        let sim = campaign.simulation_mut();
        sim.set_level2_entity_position(player, approach).unwrap();
        sim.state.actual_speed = 0;
        sim.state.target_speed = 0;
        let mut ready = false;
        for turn in 0..320 {
            campaign.step(crate::PlayerCommand::default(), 0);
            let generation = campaign.generation();
            let frame = crate::day_indexed_frame::render_level2_day_indexed_frame(
                campaign.inspection(), campaign.simulation(), &assets, generation,
            ).expect("actual Level25 approach frame");
            frame.apply_primary_visibility(campaign.simulation_mut(), generation)
                .expect("actual primary visibility feeds the source boss");
            let sim = campaign.simulation();
            assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "natural boss entry turn{turn}: {:?}", sim.dispatch_events());
            assert_eq!(sim.source_arena().handle(boss_slot), Some(boss));
            let row = &sim.entities()[usize::from(boss_slot)];
            assert_eq!(row.life, 300_000, "preparation does not damage the boss");
            if row.action == 80 && (2..12).contains(&row.record[74]) && row.record[61] != 0 {
                ready = true; break;
            }
        }
        assert!(ready, "the original unmodified boss must naturally reach a damage-consuming phase");
        let target = campaign.simulation().entities()[usize::from(boss_slot)].clone();
        // One disclosed player placement, immediately above the real current
        // boss; no later tracking or command depends on its pose or health.
        let origin = Position { z: target.position.z.wrapping_add(1024), ..target.position };
        let sim = campaign.simulation_mut();
        sim.set_level2_entity_position(player, origin).unwrap();
        sim.state.actual_speed = 0;
        sim.state.target_speed = 0;
        let caster = &sim.entities()[usize::from(player)];
        let aim = crate::level1_mission::projectile_guidance_angles(
            Position { z: caster.position.z.wrapping_add(caster.array_fov()), ..caster.position },
            Position { z: target.position.z.wrapping_add(target.array_yaw()), ..target.position },
        );
        assert_eq!(aim.1, 512, "the fixed shot points down at the actual boss");
        let mut no_cast = campaign.clone();
        area_damage::start_actor_damage_trace(&[boss]);
        area_damage::start_effect_damage_trace(boss_slot);
        let mut launches = Vec::new();
        let mut payments = Vec::new();
        let mut impacts = Vec::new();
        let mut consumed = Vec::new();
        let mut published_hit = None;
        let mut payment_visits = Vec::new();
        let mut projectile_hidden = false;
        let mut turns = 0;
        for turn in 0..160 {
            if turn == 0 { campaign.simulation_mut().set_level2_player_aim_for_test(aim.0, aim.1); }
            campaign.simulation().start_creature17_trace();
            campaign.step(crate::PlayerCommand { flags: if turn == 0 { 0x10 } else { 0 },
                ..Default::default() }, 0);
            turns += 1;
            let trace = campaign.simulation().take_creature17_trace();
            launches.extend(trace.meteor_launches.into_iter().filter(|row|
                row.holder == holder_handle && row.caster == caster_handle));
            payments.extend(trace.spell_launches.into_iter().filter(|row|
                row.holder == holder_handle && row.caster == caster_handle));
            impacts.extend(trace.meteor_impacts);
            let fresh = area_damage::take_player_damage_consumptions();
            let sim = campaign.simulation();
            assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "paid Meteor turn{turn}: {:?}", sim.dispatch_events());
            assert_eq!(sim.source_arena().handle(boss_slot), Some(boss));
            if published_hit.is_none() {
                if let Some(visit) = fresh.iter().find(|visit| visit.target == boss
                    && visit.mailbox == (1600, player)
                    && visit.writer.is_some_and(|writer| impacts.iter().any(|impact|
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                            && launches.iter().any(|launch| launch.projectile == impact.projectile))))
                {
                    let row = &sim.entities()[usize::from(boss_slot)];
                    // The receipt is published only after the real actor
                    // store verified life/attacker/cleared owner. A later
                    // numeric callback may write a new mailbox this turn.
                    let pending_owner = row.source_upkeep_word_62();
                    let refill = if pending_owner == 0 { None } else {
                        let mailbox = (row.source_upkeep_payload(), pending_owner);
                        let writer = area_damage::current_actor_mailbox_writer(boss, mailbox)
                            .expect("a later nonempty boss mailbox must have its actual watched writer");
                        assert!(writer.sequence > visit.writer.unwrap().sequence,
                            "a later writer, not the consumed mailbox, explains the current owner");
                        Some(writer)
                    };
                    published_hit = Some((*visit, refill));
                }
            }
            consumed.extend(fresh);
            payment_visits.extend(sim.cf10_production_schedule_receipts().into_iter().filter_map(|receipt|
                match receipt {
                    crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                        slot, original,
                        current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
                    } if slot == player => Some((original.observed_player_resources()?,
                        selector.observed_player_resources()?)),
                    _ => None,
                }));
            for launch in &launches {
                if sim.source_arena().handle(launch.projectile.source_slot) == Some(launch.projectile) {
                    projectile_hidden |= sim.entities()[usize::from(launch.projectile.source_slot)].drawing_disabled();
                }
            }
            let cleanup = matches!(launches.as_slice(), [launch] if projectile_hidden
                && sim.source_arena().resolve(launch.projectile).is_none())
                && impacts.iter().any(|impact| launches.iter().any(|launch| launch.projectile == impact.projectile)
                    && sim.source_arena().resolve(impact.impact).is_none());
            if cleanup && !consumed.is_empty()
                && sim.entities()[usize::from(boss_slot)].source_upkeep_word_62() == 0 { break; }
        }
        let receipts = area_damage::take_effect_damage_trace();
        let mut first_segment_life = 300_000_i32;
        for visit in &consumed {
            assert_eq!((visit.target, visit.life_before, visit.mailbox.1), (boss, first_segment_life, player));
            let writer = visit.writer.expect("every first-segment boss hit has its actual mailbox writer");
            assert_eq!((writer.target, writer.mailbox), (boss, visit.mailbox));
            assert!(impacts.iter().any(|impact|
                writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                    && launches.iter().any(|launch| launch.projectile == impact.projectile)
                    && receipts.iter().any(|receipt| receipt.source_scoped
                        && receipt.source_slot == Some(impact.impact.source_slot)
                        && receipt.target_slot == boss_slot && receipt.target_generation == boss.generation
                        && receipt.pulse.owner == player && receipt.pulse.payload == 1600
                        && receipt.writer_sequence == Some(writer.sequence))),
                "every first-segment subtraction must join the paid Meteor impact");
            let raw = first_segment_life.wrapping_sub(visit.mailbox.0.clamp(1, 300));
            first_segment_life = if raw < 10 { 8 } else { raw };
            assert_eq!(visit.life_after, first_segment_life);
        }
        assert_eq!((campaign.simulation().entities()[usize::from(boss_slot)].life,
            campaign.simulation().entities()[usize::from(boss_slot)].source_upkeep_word_62()),
            (first_segment_life, 0), "the completed field leaves no unaccounted boss hit or pending owner");
        // The no-cast clone shares this thread, not this arena. Stop watching
        // the original handle until that separate replay has finished.
        area_damage::start_actor_damage_trace(&[]);
        for turn in 0..turns {
            if turn == 0 { no_cast.simulation_mut().set_level2_player_aim_for_test(aim.0, aim.1); }
            no_cast.step(crate::PlayerCommand::default(), 0);
            assert!(no_cast.simulation().dispatch_events().iter().all(|event|
                !matches!(event, Mc2DispatchEvent::Error { .. })), "no-cast turn{turn}");
        }
        let hit = consumed.iter().find(|visit| visit.target == boss && visit.mailbox == (1600, player)
            && visit.life_before >= 10 && visit.life_after == visit.life_before - 300
            && visit.writer.is_some_and(|writer| writer.target == boss && writer.mailbox == visit.mailbox
                && impacts.iter().any(|impact| writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                    && impact.state.subspell_index == 8000 && launches.iter().any(|launch|
                        launch.projectile == impact.projectile && launch.packet.class == 9
                            && launch.packet.model == 3 && launch.packet.subspell_index == 8000)
                    && receipts.iter().any(|receipt| receipt.source_scoped
                        && receipt.source_slot == Some(impact.impact.source_slot)
                        && receipt.target_slot == boss_slot && receipt.target_generation == boss.generation
                        && receipt.pulse.owner == player && receipt.pulse.payload == 1600
                        && receipt.writer_sequence == Some(writer.sequence)))))
            .unwrap_or_else(|| panic!("one paid MeteorII must reach the original boss mailbox: origin={origin:?} aim={aim:?} launches={launches:?} impacts={impacts:?} consumed={consumed:?} receipts={receipts:?}"));
        assert_eq!(hit.life_before, 300_000, "first attributed hit reaches the unchanged boss");
        let (published, refill) = published_hit.expect("actual boss store publishes the attributed consumption");
        assert_eq!(published, *hit, "the attributed consumption is the observed published boss state");
        if let Some(refill) = refill {
            assert_eq!(refill.source, hit.writer.unwrap().source,
                "the refill must come from the same actual impact generation");
            assert!(receipts.iter().any(|receipt| receipt.writer_sequence == Some(refill.sequence)
                && receipt.target_slot == boss_slot && receipt.target_generation == boss.generation
                && receipt.source_scoped && impacts.iter().any(|impact|
                    refill.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                        && receipt.source_slot == Some(impact.impact.source_slot)
                        && launches.iter().any(|launch| launch.projectile == impact.projectile))),
                "the later accepted write belongs to the same actual Meteor impact generation");
        }
        assert_eq!(launches.len(), 1);
        let payment = payments.iter().find(|row| row.projectile == launches[0].projectile)
            .expect("actual holder and parent around the successful launch");
        let cost = crate::level_definition::METEOR_PROPERTIES.tiers[1].mana_cost;
        let before = payment.caster_before.mana_regen;
        let expected = if before >= 0 { -cost } else { before.wrapping_sub(cost) };
        assert_eq!((payment.holder_before.level_gate, payment.holder_before.max_mana), (1, cost));
        assert_eq!(payment.caster_after.mana_regen, expected, "real current parent queues the exact paid cost");
        let (before_debit, after_debit) = payment_visits.iter().find(|(before, after)|
            before.0 == caster_handle && after.0 == caster_handle && before.2 <= -cost
                && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3))
            .unwrap_or_else(|| panic!("actual player upkeep must consume the queued Meteor cost: {payment_visits:?}"));
        assert!(before_debit.1 - after_debit.1 >= cost, "actual source upkeep debits the paid cost");
        assert!(projectile_hidden && campaign.simulation().source_arena().resolve(launches[0].projectile).is_none());
        assert!(impacts.iter().any(|impact| impact.projectile == launches[0].projectile
            && campaign.simulation().source_arena().resolve(impact.impact).is_none()));
        assert_eq!(no_cast.simulation().source_arena().handle(boss_slot), Some(boss));
        assert_eq!(no_cast.simulation().entities()[usize::from(boss_slot)].life, 300_000,
            "equal-elapsed no-cast original boss stays undamaged");
        eprintln!("Level25 paid Meteor first hit: boss={boss:?} mailbox={:?} life={}→{} impact(s)={} turns={turns}",
            hit.mailbox, hit.life_before, hit.life_after, impacts.len());

        // Continue this same real encounter. Send the public full-stop once,
        // then let recoil and impulses settle naturally while waiting for the
        // actual phase3 with no pending mailbox or close-projectile latch.
        let mut stable_entry = false;
        for turn in 0..1024 {
            let sim = campaign.simulation();
            assert_eq!(sim.source_arena().handle(boss_slot), Some(boss));
            let row = &sim.entities()[usize::from(boss_slot)];
            assert_eq!(row.life, first_segment_life,
                "the natural corridor-entry wait cannot hide an unobserved boss subtraction");
            let flight = &sim.state;
            let quiet = flight.actual_speed == 0 && flight.target_speed == 0
                && flight.strafe_speed == 0 && flight.recoil_speed == 0
                && flight.impulse_x == 0 && flight.impulse_y == 0 && flight.impulse_z == 0;
            if turn != 0 && quiet && row.action == 80 && row.record[74] == 3 && row.subspell_index & 1 == 0
                && row.source_upkeep_word_62() == 0
            {
                stable_entry = true;
                break;
            }
            let command = if turn == 0 {
                crate::PlayerCommand { opcode: 0x27, argument_1: 1, ..Default::default() }
            } else { crate::PlayerCommand::default() };
            campaign.step(command, 0);
            assert!(campaign.simulation().dispatch_events().iter().all(|event|
                !matches!(event, Mc2DispatchEvent::Error { .. })), "natural corridor entry turn{turn}");
        }
        assert!(stable_entry, "the same original boss must naturally reach phase3 with its source latch and mailbox clear while the real player comes to rest: flight={:?}", campaign.simulation().state);

        // One source-derived player-only retreat, then one unchanged held-fire
        // command. The -8256 offset is outside phase3's strict8192 radius and
        // still within the 22*384 expiry field's reach. Test the CURRENT
        // post-feature terrain and source axes, not the reset terrain.
        let sim = campaign.simulation();
        let target = sim.entities()[usize::from(boss_slot)].clone();
        let parent = sim.entities()[usize::from(player)].clone();
        let mut retreat = Position { x: target.position.x.wrapping_sub(8256),
            y: target.position.y, z: 0 };
        let terrain = sim.canonical_terrain();
        let old_east = Position { x: target.position.x.wrapping_add(8256),
            y: target.position.y, z: 0 };
        let east_material = (terrain.terrain_type(old_east), terrain.material_mask(old_east));
        let west_material = (terrain.terrain_type(retreat), terrain.material_mask(retreat));
        assert_ne!(west_material.1, crate::MOVEMENT_REDIRECT_MASK,
            "the fixed player position must admit ordinary stationary movement: west={retreat:?} material={west_material:?}; old east={old_east:?} material={east_material:?}");
        let player_floor = terrain.ground_height(retreat);
        let descriptor_clearance = parent.parameter_relation
            .and_then(|index| sim.session.definition.presentation.entity_parameters.get(usize::from(index)))
            .map_or(0, |row| i16::from_le_bytes([row[12], row[13]]));
        let clearance = sim.profile().ground_clearance.max(descriptor_clearance);
        retreat.z = player_floor.checked_add(clearance).expect("legal source player altitude");
        assert!(crate::native_world::level2_wrapping_target_distance(retreat, target.position) >= 8192,
            "fixed retreat must stay outside the source phase3 trigger");
        let mut muzzle = retreat;
        crate::move_entity(&mut muzzle, 0, 0, 256); // source left hand at yaw512
        assert!(terrain.ground_height(muzzle) <= retreat.z,
            "the source left-hand muzzle must not fall back to the player's position");
        muzzle.z = muzzle.z.checked_add(parent.array_fov()).expect("source muzzle height");
        let field_center = i32::from(target.position.z) + i32::from(target.array_yaw());
        assert!((i32::from(muzzle.z) - field_center).abs() < 512 + i32::from(target.array_fov()),
            "the eventual source Meteor field must overlap the real boss vertical extent");
        let mut ray = muzzle;
        let mut sampled_ground = Vec::new();
        for _ in 0..22 {
            crate::move_entity(&mut ray, 512, 0, crate::level_definition::METEOR_PROPERTIES.projectile_speed);
            let floor = terrain.ground_height(ray);
            sampled_ground.push(floor);
            assert!(floor <= ray.z, "the fixed source ray hits current terrain before its terminal endpoint: ray={ray:?} floor={floor} retreat={retreat:?}");
        }
        assert!(i32::from(target.position.x.wrapping_sub(ray.x) as i16).abs() < i32::from(target.array_pitch())
            && i32::from(target.position.y.wrapping_sub(ray.y) as i16).abs() < i32::from(target.array_roll()),
            "the fixed expiry field endpoint overlaps the current boss XY axes");
        let stage_life = target.life;
        assert!(!sim.session.state.live.core.runtime.alternate_volcano(),
            "the original alternate Volcano flag is clear before the actual boss death");
        let sim = campaign.simulation_mut();
        sim.set_level2_entity_position(player, retreat).unwrap();
        sim.set_level2_player_aim_for_test(512, 0);
        area_damage::start_actor_damage_trace(&[boss]);

        // Budget a full source recharge/active/flight/field interval for EACH
        // remaining 300-cap callback. Even one accepted hit per field has
        // enough time; success is actual death, not any cast or hit count.
        let remaining_hits = (i64::from(stage_life) - 9 + 299) / 300;
        let animation = campaign.simulation().session.definition.presentation.particles[345];
        let death_animation = campaign.simulation().session.state.live.source_animations
            .boss_timer_read(animation.base_tmap).expect("real boss death animation")
            .unwrap_or(32);
        let interval_bound = usize::try_from(remaining_hits).unwrap()
            .saturating_mul(100 + 7 + 22 + 5 + 1)
            .saturating_add(usize::from(death_animation) + 93 + 1024);
        let mut fight_launches = Vec::new();
        let mut fight_payments = Vec::new();
        let mut fight_impacts = Vec::new();
        let mut fight_effects = Vec::new();
        let mut fight_consumed = Vec::new();
        let mut life_chain = stage_life;
        let mut phase13 = false;
        let mut phase14 = false;
        let mut phase15 = false;
        let mut visible13 = false;
        let mut visible14 = false;
        let mut negative_retained = false;
        let mut death_sound10 = false;
        let mut death_sound63 = false;
        let mut death_meteor = None;
        let mut reward: Option<(crate::campaign::source_arena::SourceEntityHandle, i32, usize)> = None;
        let mut reward_consumed = false;
        let mut reward_product = None;
        let mut hidden = false;
        let mut reclaimed = false;
        let mut first_drift = None;
        let mut first_acquisition = None;
        let mut first_impact_reclaim = None;
        let mut initial_flight = Vec::new();
        // (phase, remaining timer at first observation, real later callbacks).
        let mut death_timer_watch: Option<(u8, i32, usize)> = None;
        let mut completed_timers = Vec::new();
        let mut group31_publications = 0;
        let mut exit_root_hidden = false;
        for turn in 0..interval_bound {
            let prior_boss = campaign.simulation().source_arena().handle(boss_slot) == Some(boss);
            let boss_before = prior_boss.then(|| &campaign.simulation().entities()[usize::from(boss_slot)]);
            let constructing_death_meteor = boss_before.is_some_and(|row| row.record[74] == 12);
            let constructing_reward = boss_before.is_some_and(|row|
                row.record[74] == 15 && row.source_dword_10() <= 1);
            let pre_meteors = if constructing_death_meteor {
                campaign.simulation().economy().authored_model17().iter().filter(|row| row.active)
                    .filter_map(|row| campaign.simulation().source_arena().handle(row.source_entity_index))
                    .collect::<Vec<_>>()
            } else { Vec::new() };
            let pre_volcanoes = if constructing_reward {
                campaign.simulation().entities().iter().filter(|row|
                    row.active && row.class == 10 && row.model == 9)
                    .filter_map(|row| campaign.simulation().source_arena().handle(row.index))
                    .collect::<Vec<_>>()
            } else { Vec::new() };
            let reward_before = reward.and_then(|(handle, _, _)| {
                let sim = campaign.simulation();
                (sim.source_arena().handle(handle.source_slot) == Some(handle))
                    .then(|| sim.entities()[usize::from(handle.source_slot)].clone())
            });
            let pre_reward_products = if reward_before.as_ref().is_some_and(|row| row.life == 4) {
                campaign.simulation().entities().iter().filter(|row|
                    row.active && row.class == 10 && row.model == 91)
                    .filter_map(|row| campaign.simulation().source_arena().handle(row.index))
                    .collect::<Vec<_>>()
            } else { Vec::new() };
            area_damage::start_effect_damage_trace(boss_slot);
            campaign.simulation().start_creature17_trace();
            campaign.step(crate::PlayerCommand { flags: 0x10, ..Default::default() }, 0);
            let trace = campaign.simulation().take_creature17_trace();
            if turn < 2 {
                initial_flight.extend(trace.flight.iter().filter(|visit| visit.target == caster_handle)
                    .cloned().map(|visit| (turn, visit,
                        campaign.simulation().state.position)));
                if turn == 1 {
                    eprintln!("Level25 fixed west setup: old_east={old_east:?} material={east_material:?} west={retreat:?} material={west_material:?} first_flight={initial_flight:?}");
                }
            }
            let sound_calls = trace.boss_sound_calls;
            let new_launches = trace.meteor_launches.into_iter().filter(|row|
                row.holder == holder_handle && row.caster == caster_handle).collect::<Vec<_>>();
            fight_launches.extend(new_launches);
            if first_acquisition.is_none() {
                first_acquisition = trace.meteor_acquisitions.into_iter().find(|observation|
                    fight_launches.first().is_some_and(|launch| launch.projectile == observation.projectile));
            }
            fight_payments.extend(trace.spell_launches.into_iter().filter(|row|
                row.holder == holder_handle && row.caster == caster_handle));
            fight_impacts.extend(trace.meteor_impacts);
            fight_effects.extend(area_damage::take_effect_damage_trace());
            let visits = area_damage::take_player_damage_consumptions();
            for visit in &visits {
                assert_eq!(visit.target, boss);
                assert_eq!(visit.life_before, life_chain,
                    "every source damage callback must follow the original boss life chain");
                assert_ne!(visit.mailbox.1, 0);
                let raw_after = life_chain.wrapping_sub(visit.mailbox.0.clamp(1, 300));
                let expected = if raw_after < 10 { 8 } else { raw_after };
                assert_eq!(visit.life_after, expected, "source22190 clamps damage and terminal life");
                let writer = visit.writer.expect("actual current owner must have a generation-bound writer");
                assert_eq!((writer.target, writer.mailbox), (boss, visit.mailbox));
                assert!(fight_impacts.iter().any(|impact|
                    writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                        && fight_launches.iter().any(|launch| launch.projectile == impact.projectile)
                        && fight_payments.iter().any(|payment| payment.projectile == impact.projectile
                            && payment.holder_before.max_mana == cost
                            && payment.caster_after.mana_regen == if payment.caster_before.mana_regen >= 0 {
                                -cost
                            } else { payment.caster_before.mana_regen.wrapping_sub(cost) })
                        && fight_effects.iter().any(|receipt| receipt.source_scoped
                            && receipt.source_slot == Some(impact.impact.source_slot)
                            && receipt.target_slot == boss_slot && receipt.target_generation == boss.generation
                            && receipt.pulse.owner == player && receipt.pulse.payload == 1600
                            && receipt.writer_sequence == Some(writer.sequence))),
                    "every actual boss subtraction must join the current player's paid Meteor generation");
                life_chain = expected;
            }
            fight_consumed.extend(visits);
            let sim = campaign.simulation();
            assert!(sim.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "held Meteor turn{turn}: {:?}", sim.dispatch_events());
            group31_publications += sim.dispatch_events().iter().filter(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 31, authored_indices, .. }
                    if *source_slot == exit_root && authored_indices.as_ref() == group31_rows.as_slice())).count();
            if sim.source_arena().handle(exit_root) == Some(exit_root_handle) {
                exit_root_hidden |= sim.entities()[usize::from(exit_root)].drawing_disabled();
            }
            let actual_player = &sim.entities()[usize::from(player)];
            if actual_player.position != retreat && first_drift.is_none() {
                first_drift = Some((turn, actual_player.position, actual_player.life));
            }
            for call in sound_calls.iter().filter(|call| call.caller == Some(boss)
                && call.emitter == boss_slot && matches!(call.site, SoundSite::DeathRumble | SoundSite::DeathPeriodic))
            {
                let (site, sample) = match call.site {
                    SoundSite::DeathRumble => (crate::MissionAudioSite::BossDeathRumble, 10),
                    SoundSite::DeathPeriodic => (crate::MissionAudioSite::BossDeathPeriodic, 63),
                    _ => unreachable!(),
                };
                assert_eq!(call.sample, sample);
                let record = sim.effect_journal().records().iter().find(|record|
                    record.sequence == call.journal_sequence).expect("actual death sound journal sequence");
                assert!(matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(
                    crate::FlightEvent::PrepareSound { audio_site, entity_index, player_index: -1, sound_id }
                ) if audio_site == site && entity_index == boss_slot && sound_id == sample));
                assert_eq!(record.audio.expect("unmuted source death sound is prepared").play_type, 1);
                death_sound10 |= call.site == SoundSite::DeathRumble;
                death_sound63 |= call.site == SoundSite::DeathPeriodic;
            }
            if sim.source_arena().handle(boss_slot) == Some(boss) {
                let row = &sim.entities()[usize::from(boss_slot)];
                assert_eq!(row.life, if row.life < 0 { -1 } else { life_chain },
                    "no unobserved source may alter the original boss life chain");
                phase13 |= row.record[74] == 13;
                if row.record[74] == 14 && !phase14 {
                    assert_eq!(row.source_dword_10(), i32::from(death_animation),
                        "the real phase13→14 callback installs the resident particle345 timer");
                }
                phase14 |= row.record[74] == 14;
                phase15 |= row.record[74] == 15;
                hidden |= row.drawing_disabled();
                if life_chain == 8 {
                    let callbacks = sim.cf10_production_schedule_receipts().iter().filter(|receipt|
                        matches!(receipt, crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                            if *slot == boss_slot)).count();
                    if let Some((phase, remaining, ref mut seen)) = death_timer_watch {
                        *seen += callbacks;
                        if row.record[74] != phase || row.drawing_disabled() {
                            assert!(*seen >= usize::try_from(remaining.max(0)).unwrap(),
                                "natural death phase{phase} must consume its observed remaining timer{remaining}, saw{seen} source callbacks");
                            completed_timers.push(phase);
                            death_timer_watch = None;
                        }
                    }
                    if death_timer_watch.is_none() && matches!(row.record[74], 13..=15)
                        && !row.drawing_disabled()
                    {
                        death_timer_watch = Some((row.record[74], row.source_dword_10(), 0));
                    }
                }
                if matches!(row.record[74], 13 | 14) && row.life == 8 && !row.drawing_disabled() {
                    let views = sim.campaign_entity_presentations(0).expect("actual boss death presentation");
                    assert!(views.iter().any(|view| view.entity_index == boss_slot
                        && Some(view.render_parameter_index) == row.particle_parameter_index),
                        "the living life8 death phase keeps its actual descriptor-backed sprite");
                    visible13 |= row.record[74] == 13;
                    visible14 |= row.record[74] == 14;
                }
                if row.record[74] == 15 && row.life == -1 && !row.drawing_disabled() {
                    assert_ne!(row.control_flags & 1, 0, "source phase14 sets the separate sprite-exclusion bit");
                    assert!(!sim.campaign_entity_presentations(0).unwrap().iter()
                        .any(|view| view.entity_index == boss_slot));
                    let identity = sim.creature_runtime_census().entries().iter()
                        .find(|entry| entry.handle == boss).copied()
                        .expect("the negative-life original remains a retained creature identity");
                    assert!(!identity.living);
                    assert!(!sim.source_arena().rebuilt_class5_model_slots()[10].contains(&boss_slot),
                        "source absence/target lists exclude the negative-life original before reclaim");
                    negative_retained = true;
                }
                if constructing_death_meteor && row.record[74] == 13 && death_meteor.is_none() {
                    death_meteor = sim.economy().authored_model17().iter()
                        .find(|source| source.active && source.max_life == 70
                            && source.entity_id == actual_player.source_id
                            && sim.source_arena().handle(source.source_entity_index)
                                .is_some_and(|handle| !pre_meteors.contains(&handle)))
                        .and_then(|source| sim.source_arena().handle(source.source_entity_index));
                }
            } else if hidden {
                reclaimed = true;
            }
            if first_impact_reclaim.is_none() && let Some(first) = fight_impacts.iter().find(|impact|
                fight_launches.first().is_some_and(|launch| launch.projectile == impact.projectile))
                && sim.source_arena().resolve(first.impact).is_none()
            {
                first_impact_reclaim = Some((
                    turn,
                    first.clone(),
                    fight_consumed.iter().any(|visit| visit.writer.is_some_and(|writer|
                        writer.source == area_damage::PlayerMailboxSource::Effect(first.impact))),
                ));
            }
            if constructing_reward && sim.session.state.live.core.runtime.alternate_volcano() && reward.is_none() {
                reward = sim.entities().iter().filter(|row| row.active && row.class == 10
                    && row.model == 9 && row.action == 9 && row.max_life == 11
                    && row.source_id == actual_player.source_id)
                    .filter_map(|row| sim.source_arena().handle(row.index).map(|handle| (handle, row.life, turn)))
                    .find(|(handle, _, _)| !pre_volcanoes.contains(handle));
            }
            if let Some((handle, initial_life, born_turn)) = reward
                && turn > born_turn && sim.source_arena().handle(handle.source_slot) == Some(handle)
            {
                reward_consumed |= sim.entities()[usize::from(handle.source_slot)].life < initial_life;
            }
            if let Some(before) = &reward_before && before.life == 4 {
                assert!(sim.session.state.live.core.runtime.alternate_volcano(),
                    "actual reward Volcano callback consumes the source alternate-mode flag");
                reward_product = sim.entities().iter().filter(|row|
                    row.active && row.class == 10 && row.model == 91
                        && row.source_id == actual_player.source_id
                        && (row.position.x, row.position.y) == (before.position.x, before.position.y))
                    .filter_map(|row| sim.source_arena().handle(row.index))
                    .find(|handle| !pre_reward_products.contains(handle));
            }
            if reclaimed && reward_consumed && reward_product.is_some() && death_meteor.is_some() { break; }
        }
        let terminal = fight_consumed.last().unwrap_or_else(|| panic!(
            "held public Meteor produced no boss consumption: retreat={retreat:?} muzzle={muzzle:?} sampled_ground={sampled_ground:?} launches={} impacts={} first_acquisition={first_acquisition:?} first_impact_reclaim={first_impact_reclaim:?} drift={first_drift:?}",
            fight_launches.len(), fight_impacts.len()));
        assert_eq!((terminal.life_before, terminal.life_after), (300, 8),
            "the final actual source hit must clamp the original boss to life8: interval={interval_bound} retreat={retreat:?} muzzle={muzzle:?} sampled_ground={sampled_ground:?} first_acquisition={first_acquisition:?} first_impact_reclaim={first_impact_reclaim:?} drift={first_drift:?} launches={} impacts={} consumed={}",
            fight_launches.len(), fight_impacts.len(), fight_consumed.len());
        let terminal_writer = terminal.writer.expect("terminal source mailbox writer");
        assert_eq!(terminal.mailbox.1, player);
        assert!(fight_impacts.iter().any(|impact| terminal_writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
            && fight_launches.iter().any(|launch| launch.projectile == impact.projectile)
            && fight_effects.iter().any(|receipt| receipt.source_scoped
                && receipt.source_slot == Some(impact.impact.source_slot)
                && receipt.target_slot == boss_slot && receipt.target_generation == boss.generation
                && receipt.pulse.owner == player && receipt.pulse.payload == 1600
                && receipt.writer_sequence == Some(terminal_writer.sequence))),
            "the terminal actual writer must be the current player's paid Meteor impact");
        assert!(fight_launches.iter().all(|launch| fight_payments.iter().any(|payment|
            payment.projectile == launch.projectile && payment.holder_before.max_mana == cost
                && payment.caster_after.mana_regen == if payment.caster_before.mana_regen >= 0 {
                    -cost
                } else { payment.caster_before.mana_regen.wrapping_sub(cost) })),
            "every recorded player Meteor launch must queue the real source cost");
        assert_eq!(life_chain, 8);
        assert!(phase13 && phase14 && phase15 && visible13 && visible14 && negative_retained
            && death_sound10 && death_sound63 && hidden && reclaimed
            && death_meteor.is_some() && reward.is_some() && reward_consumed && reward_product.is_some()
            && [13, 14, 15].iter().all(|phase| completed_timers.contains(phase)),
            "natural death must visit/present phases13/14, prepare source10/63, drop living membership, schedule its Meteor/Volcano and reclaim the original boss: phases={phase13}/{phase14}/{phase15} visible={visible13}/{visible14} nonliving={negative_retained} audio={death_sound10}/{death_sound63} hidden={hidden} reclaimed={reclaimed} death_meteor={death_meteor:?} reward={reward:?} reward_consumed={reward_consumed} reward_product={reward_product:?} timers={completed_timers:?} remaining={life_chain} interval={interval_bound} first_acquisition={first_acquisition:?} first_impact_reclaim={first_impact_reclaim:?} drift={first_drift:?}");
        assert_eq!(campaign.simulation().session.state.live.core.runtime.final_boss_active_counter(), Some(0));
        eprintln!("Level25 natural boss defeat: start_life={stage_life} consumed={} paid_launches={} terminal={terminal:?} death_meteor={death_meteor:?} reward={reward:?} reward_product={reward_product:?} timers={completed_timers:?} first_drift={first_drift:?}",
            fight_consumed.len(), fight_launches.len());

        // Preserve the same paid fight and its actual target-death result.
        // If its fixed firing position did not already satisfy the earlier
        // objective, place only the player at the authored reach point. No
        // objective, switch, lifetime or result state is staged.
        assert_eq!(campaign.simulation().runtime().objectives[0].stage_selectors[1], 2,
            "the actual authored boss's negative-life visit satisfies its bound objective");
        if campaign.simulation().runtime().objectives[0].stage_selectors[0] == 1 {
            let mut point = Position { x: 40 * 256, y: 212 * 256, z: 0 };
            point.z = campaign.simulation().canonical_terrain().ground_height(point).wrapping_add(768);
            campaign.simulation_mut().set_level2_entity_position(player, point)
                .expect("declared real-player pose at the authored reach objective");
        }
        for _ in 0..32 {
            if group31_publications == 1
                && campaign.simulation().source_arena().resolve(exit_root_handle).is_none() { break; }
            campaign.step(crate::PlayerCommand::default(), 0);
            let sim = campaign.simulation();
            assert!(!sim.dispatch_events().iter().any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
                "final objective and group31: {:?}", sim.dispatch_events());
            group31_publications += sim.dispatch_events().iter().filter(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 31, authored_indices, .. }
                    if *source_slot == exit_root && authored_indices.as_ref() == group31_rows.as_slice())).count();
            if sim.source_arena().handle(exit_root) == Some(exit_root_handle) {
                exit_root_hidden |= sim.entities()[usize::from(exit_root)].drawing_disabled();
            }
        }
        let sim = campaign.simulation();
        let objective = &sim.runtime().objectives[0];
        assert_eq!((&objective.stage_selectors[..2], objective.is_level_end), (&[2, 2][..], 1));
        assert_eq!(group31_publications, 1, "row392 publishes its source-ordered group once");
        assert!(exit_root_hidden && sim.source_arena().resolve(exit_root_handle).is_none(),
            "row392 hides and its original incarnation is reclaimed");
        assert_final_exit_consumer(campaign, &package, &cache, player, true);
    }

    fn assert_final_exit_consumer(
        mut campaign: crate::campaign::CampaignSession,
        package: &std::path::Path,
        cache: &std::path::Path,
        player: u16,
        paid_fight: bool,
    ) {
        use crate::native_world::Mc2DispatchEvent;
        let sim = campaign.simulation();
        let portal = sim.economy().authored_class14().iter()
            .find(|row| row.active && row.authored_index == 393).copied()
            .expect("row393 is the group31 visible portal");
        let trigger = sim.economy().authored_switch_producers().iter()
            .find(|row| row.active && row.authored_index == 394).copied()
            .expect("row394 is the group31 exit trigger");
        assert_eq!((portal.model, portal.action, trigger.model, trigger.action), (3, 8, 12, 12));
        let trigger_handle = sim.source_arena().handle(trigger.source_entity_index).unwrap();
        assert!(trigger.source_entity_index < player,
            "this source allocation visits the row394 trigger before the player");
        campaign.simulation_mut().set_level2_entity_position(player, trigger.position)
            .expect("declared real-player pose overlaps the actual row394 trigger");
        // A long paid encounter exceeds the bounded retained journal window;
        // a slice length is not an absolute event cursor once old rows expire.
        let mut exit_cursor = campaign.effect_cursor();
        let mut exit_records = Vec::new();
        for _ in 0..16 {
            if campaign.simulation().level_exit_sequence().active { break; }
            campaign.step(crate::PlayerCommand::default(), 0);
            assert!(!campaign.simulation().dispatch_events().iter().any(|event|
                matches!(event, Mc2DispatchEvent::Error { .. })));
            let page = campaign.effects_after(exit_cursor, usize::MAX)
                .expect("each trigger turn retains its new production effects");
            exit_cursor = page.next_cursor;
            exit_records.extend(page.records.into_vec());
        }
        let sim = campaign.simulation();
        let exit = sim.level_exit_sequence();
        assert!(exit.active && !exit.complete);
        // sub_6F2B0 writes action12/phase0 at the lower trigger slot; the
        // same ascending UpdateEntities scan then visits the player and runs
        // sub_5E8C0 phase0 to phase1. Capture both callback order and the
        // phase0-only effects, rather than expecting an end-of-turn phase0.
        let receipts = sim.cf10_production_schedule_receipts();
        let trigger_visit = receipts.iter().position(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                if *slot == trigger.source_entity_index)).expect("row394 callback");
        let player_visit = receipts.iter().position(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, selector, .. }
                if *slot == player && selector.registry_key().selector ==
                    (crate::campaign::spec::BehaviorSelector::ClassAction { class: 3, action: 12 })))
            .expect("same-pass player action12 callback");
        assert!(trigger_visit < player_visit);
        let start = exit_records.iter().position(|record| matches!(record.effect,
            crate::campaign::journal::CampaignEffect::Dispatch(Mc2DispatchEvent::LevelExitSequenceStarted {
                trigger_entity_index, portal_entity_index, ..
            }) if trigger_entity_index == trigger.source_entity_index
                && portal_entity_index == portal.source_entity_index)).expect("real exit start");
        let phase_zero_sound = exit_records.iter().position(|record| matches!(record.effect,
            crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::ExitSequenceSwitch, entity_index,
                sound_id: crate::world::portals::EXIT_SWITCH_SOUND, ..
            }) if entity_index == player)).expect("player phase0 sound");
        assert!(start < phase_zero_sound);
        assert_eq!((exit.trigger_entity_index, exit.portal_entity_index, exit.phase,
            exit.palette_submode, exit.player_status_flags),
            (trigger.source_entity_index, portal.source_entity_index, 1,
                crate::world::portals::EXIT_PALETTE_ENTRY_FLASH_REQUEST, 0x20));
        assert_eq!((sim.entities()[usize::from(player)].action, sim.entities()[usize::from(player)].record[74]),
            (12, 1));
        assert_eq!(sim.economy().authored_class14().iter()
            .find(|row| row.source_entity_index == portal.source_entity_index).unwrap().control_flags & 1, 0);
        for _ in 0..700 {
            if campaign.simulation().level_exit_sequence().complete { break; }
            campaign.step(crate::PlayerCommand::default(), 0);
            assert!(!campaign.simulation().dispatch_events().iter().any(|event|
                matches!(event, Mc2DispatchEvent::Error { .. })));
        }
        let sim = campaign.simulation();
        assert!(sim.source_arena().resolve(trigger_handle).is_none(), "row394 is reclaimed after hide");
        assert!(sim.level_exit_sequence().complete);
        assert_eq!((sim.level_exit_sequence().result_code, sim.source_exit_result_byte()), (0x0a, 0x0a));
        if paid_fight {
            crate::campaign::backend::tests::assert_final_campaign_return_for_test(
                campaign.clone(), package, cache,
            );
        }
        // Move the actual completed session into a matching backend to exercise
        // the real runtime consumer. This configured admission does not claim
        // a save restore or an earned preceding campaign frontier.
        use crate::campaign::backend::{load_mc2_campaign_with_owner_cache, Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs};
        use crate::game_runtime::{CampaignMenuPhase, GameRuntime, ReplayOperation, ResolvedCompletionTarget, RuntimeMode};
        use crate::game_session::GameCommand;
        use sim_definition::runtime_flow::RuntimeFlowDestination;
        let expected_hash = campaign.state_hash();
        let expected_tick = campaign.tick();
        let expected_stats = campaign.simulation().completion_statistics();
        let definition = std::sync::Arc::new(load_mc2_campaign_with_owner_cache(
            &package, &cache, &crate::game_definition::default_catalog().unwrap(),
        ).expect("actual final mission definition and cache"));
        let mut runtime = GameRuntime::<Mc2CampaignBackend>::title(61925).unwrap();
        let title_guard = runtime.identity().guard();
        runtime.restore_playing_resolved(
            title_guard, definition, &Mc2CampaignAdapter, 61925,
            std::iter::empty::<ReplayOperation<GameCommand>>(), false,
            crate::game_runtime::DisplaySettings::default(), Vec::new(), Vec::new(),
            |_| Ok(Mc2CampaignInputs::with_owner_cache(&package, &cache)),
            |session| {
                assert_eq!(session.backend().compatibility_campaign().definition().package_sha256,
                    campaign.definition().package_sha256);
                *session.backend_mut().compatibility_campaign_mut_for_test() = campaign;
                Ok(())
            },
            |_, _| Ok(()),
            |session| {
                assert_eq!(session.backend().compatibility_campaign().state_hash(), expected_hash);
                assert_eq!(session.identity().tick, expected_tick);
                Ok(())
            },
            |_, _, _| {},
            |session, notice| match notice {
                crate::game_runtime::DisplayNotice::IconsMap(visible) => session.backend_mut().publish_display_notice(visible),
                crate::game_runtime::DisplayNotice::PanelTransparency(opaque) => session.backend_mut().publish_panel_transparency_notice(opaque),
                crate::game_runtime::DisplayNotice::FlatShading(enabled) => session.backend_mut().publish_flat_shading_notice(enabled),
            },
            |_| {},
        ).expect("controlled admission of the unchanged completed session");
        let playing = runtime.identity();
        let receipt = runtime.accept_transition_resolved(
            playing.guard(), playing.session.as_ref().unwrap().guard(),
            |_, destination| {
                assert!(matches!(destination, RuntimeFlowDestination::Title { .. }));
                Ok(ResolvedCompletionTarget::Title)
            },
        ).expect("actual result reaches the runtime completion consumer");
        assert!(receipt.verify());
        assert_eq!(receipt.completion.state_hash, expected_hash);
        assert_eq!(receipt.completion.statistics, Some(expected_stats.clone()));
        assert_eq!(receipt.after.mode, RuntimeMode::Campaign);
        let menu = receipt.after.campaign_menu.as_ref().unwrap();
        assert_eq!(menu.phase, CampaignMenuPhase::Results);
        assert_eq!(menu.completion_statistics, Some(expected_stats));
        assert_eq!(menu.completed_levels, vec![receipt.completed_level_id.clone()]);
        assert!(runtime.session().is_none());
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
            let player_row = &mut live.economy.entities[usize::from(player)];
            player_row.set_max_life(200_000);
            player_row.set_life(200_000);
            let player_row = player_row.clone();
            live.player_life = player_row.life;
            live.shared_spells.sync_player_entity_from_authority(&player_row);
        }

        let player_snapshot = |simulation: &crate::native_world::Mc2Simulation| {
            let row = &simulation.entities()[usize::from(player)];
            (row.life, row.max_life, row.control_flags, row.position, simulation.state.position)
        };
        let initial_player = player_snapshot(campaign.simulation());
        let mut child = None;
        let mut first_position = None;
        let mut last_boss = None;
        let mut first_wait = None;
        let mut last_wait = None;
        let mut first_attack = None;
        let mut final_player = initial_player;
        let mut mode6_ready_visits = 0_u32;
        let mut attack44_calls = 0_usize;
        for turn in 0..4096 {
            if load(&campaign.simulation().entities()[usize::from(boss)])
                .is_some_and(|actor| actor.phase == 9 && actor.summon_mode == 6 && actor.attack_remaining != 0)
            {
                mode6_ready_visits += 1;
            }
            campaign.simulation().start_creature17_trace();
            campaign.step(crate::PlayerCommand::default(), 0);
            let trace = campaign.simulation().take_creature17_trace();
            attack44_calls += trace.boss_sound_calls.iter().filter(|call|
                call.caller == Some(parent) && call.site == SoundSite::Attack && call.sample == 44).count();
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
            final_player = player_snapshot(simulation);
            if last_boss.is_some_and(|(phase, _, _, _, _)| phase == 9) {
                first_attack.get_or_insert((turn, last_boss, final_player));
            }
            if last_boss.is_some_and(|(phase, _, timer, _, _)| phase == 3 && timer <= 0) {
                let source_boss = &simulation.entities()[usize::from(boss)];
                let source_player = &simulation.entities()[usize::from(player)];
                let snapshot = (turn, source_boss.position, source_player.position,
                    source_player.life, source_player.control_flags, source_player.action,
                    simulation.state.position, super::super::distance(source_boss.position, source_player.position));
                first_wait.get_or_insert(snapshot);
                last_wait = Some(snapshot);
            }
            if child.is_none() {
                let census = arena.creature_runtime_census(&BTreeMap::new(), &BTreeMap::new());
                child = census.entries().iter().find(|entry|
                    entry.raw_model == Some(19)
                        && entry.role == CreatureRuntimeRole::Root
                        && entry.origin == CreatureRootOrigin::CreatureConversion { producer_slot: boss }
                        && arena.creature_parent_binding(entry.handle) == Some(parent))
                    .map(|entry| entry.handle);
                if let Some(handle) = child {
                    let call = trace.boss_sound_calls.iter().find(|call|
                        call.caller == Some(parent)
                            && call.site == SoundSite::Attack
                            && call.emitter == boss_owner
                            && call.sample == 44)
                        .expect("the original boss's real Firefly birth reaches source sound44");
                    assert_eq!(call.rng_before, call.rng_after,
                        "source sound preparation reads but does not advance RNG");
                    let record = simulation.effect_journal().records().iter()
                        .find(|record| record.sequence == call.journal_sequence)
                        .expect("the actual sound call reaches the prepared Flight journal");
                    assert!(matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(
                        crate::FlightEvent::PrepareSound {
                            audio_site: crate::MissionAudioSite::BossAttack,
                            entity_index, player_index: -1, sound_id: 44,
                        }) if entity_index == boss_owner));
                    let audio = record.audio.expect("the actual Firefly attack prepares native audio");
                    assert!(audio.command.audible, "the real nearby listener hears the attack");
                    assert_eq!(audio.play_type, 3);
                    if boss_owner == 0 {
                        assert!(call.source.is_none() && call.listener.is_none());
                        assert_eq!(audio.command.rate_delta_percent, 0);
                    } else {
                        let (emitter, source_position, _, flags) = call.source
                            .expect("non-null source resolves at the actual sound call");
                        let (listener, listener_position, _) = call.listener
                            .expect("non-muted source reads the actual listener");
                        assert_eq!((emitter.source_slot, listener.source_slot), (boss_owner, player));
                        assert_eq!(flags & 0x80, 0);
                        assert_eq!((audio.command.relative_x, audio.command.relative_y, audio.command.relative_z),
                            (i32::from(source_position.x) - i32::from(listener_position.x),
                             i32::from(source_position.y) - i32::from(listener_position.y),
                             i32::from(source_position.z) - i32::from(listener_position.z)));
                        let rate = (call.rng_before.wrapping_mul(9_377).wrapping_add(9_439) % 30) as i16 - 15;
                        assert_eq!(audio.command.rate_delta_percent, rate);
                    }
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
        panic!("unaltered Level25 boss did not produce and launch Firefly: boss={last_boss:?} child={child:?} initial_player={initial_player:?} first_attack={first_attack:?} final_player={final_player:?} mode6_ready_visits={mode6_ready_visits} attack44_calls={attack44_calls} first_postturn_wait={first_wait:?} last_postturn_wait={last_wait:?}");
    }

    // These existing checks isolate boss phase/constructor behavior. The
    // source-time callback/order and actual carrier mutations have separate
    // regressions; they are not inferred from this phase-only observer.
    fn ignore_world_action(
        arena: &mut SourceArena,
        _: &mut SessionTerrain,
        _: &mut RuntimeSnapshot,
        action: EncounterAction,
    ) -> Result<EncounterResponse, String> {
        Ok(test_world_response(arena, action))
    }

    fn test_world_response(arena: &SourceArena, action: EncounterAction) -> EncounterResponse {
        match action {
            EncounterAction::ReadPlayer { slot } => {
                let row = &arena.entities[usize::from(slot)];
                EncounterResponse::PlayerPose { position: row.position, yaw: row.yaw }
            }
            EncounterAction::ReadPlayerId { slot } =>
                EncounterResponse::PlayerId(arena.entities[usize::from(slot)].source_id),
            _ => EncounterResponse::Done,
        }
    }

    struct Source24Fixture {
        arena: SourceArena,
        runtime: RuntimeSnapshot,
        terrain: SessionTerrain,
        particles: Vec<ParticleParameter>,
        entity_parameters: Vec<[u8; 34]>,
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
        let entity_parameters = presentation.entity_parameters.to_vec();
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
            entity_parameters,
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

    #[test]
    fn source_summon_rejects_null_dead_and_byte1_bit4_player() {
        let fixture = source24_fixture();
        let mut player = fixture.arena.entities[usize::from(fixture.player_slot)].clone();
        assert_ne!(player.index, 0);
        player.set_life(1);
        player.set_control_flags(4);
        assert!(source_player_attack_eligible(&player), "low-byte 0x04 is not source byte1 0x04");
        player.set_control_flags(0x400);
        assert!(!source_player_attack_eligible(&player));
        player.set_control_flags(0);
        player.set_life(-1);
        assert!(!source_player_attack_eligible(&player));
        player.set_life(0);
        assert!(source_player_attack_eligible(&player));
        player.index = 0;
        assert!(!source_player_attack_eligible(&player));
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
            assert!(!cumulative.is_empty());
            let actual = source_bitmap_class_offsets(class).unwrap();
            assert_eq!(
                actual, cumulative[..cumulative.len() - 1],
                "AddE7EE0(0,{class}) visits all intermediate classes and omits only the final return-2 pair"
            );
            for previous_class in 0..class {
                let prior = crate::level1_mission::source_search_offsets(previous_class);
                assert!(actual.contains(prior.last().unwrap()),
                    "the final pair of intermediate class{previous_class} returns1");
            }
            if class >= 7 { assert!(actual.contains(&(0, 0)), "the interior class0 is visited"); }
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
            step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
                if matches!(action, EncounterAction::ReadPlayer { .. }) {
                    return Ok(test_world_response(arena, action));
                }
                assert_eq!(arena.economy.model13_spawned_children().len(), before);
                observed.push(action);
                Ok(EncounterResponse::Done)
            })
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
                &fixture.entity_parameters,
                boss_slot,
                frame,
                &mut |_, _, _, _| Err("world action failed".to_owned()),
            )
            .is_err()
        );
        assert_eq!(
            fixture.arena.economy.model13_spawned_children().len(),
            before
        );
    }

    #[test]
    fn configured_death_sounds_follow_source_phase_and_counter_order() {
        for (counter, expected_periodic) in [(0_u8, true), (1, false)] {
            let (mut fixture, boss_slot) = boss_fixture();
            let ground = fixture.terrain.ground_height(fixture.boss_position);
            let sound_z = ground.wrapping_add(384);
            let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
            boss.set_position(Position { z: sound_z, ..boss.position });
            boss.set_source_byte_46(14);
            boss.set_source_dword_10(1);
            boss.set_phase(counter);
            let frame = boss_frame(&fixture);
            let mut observed = Vec::new();
            let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
                let row = &arena.entities[usize::from(boss_slot)];
                observed.push((action, row.record[74], row.source_dword_10(), row.life, row.position.z));
                Ok(test_world_response(arena, action))
            }).unwrap();
            assert_eq!(observed[0].0, EncounterAction::Sound {
                site: SoundSite::DeathRumble, emitter: SoundEmitter::Slot(boss_slot), sample: 10,
            });
            assert_eq!((observed[0].1, observed[0].2), (14, 1),
                "the source rumble precedes the phase-14 decrement");
            let periodic = observed.iter().find(|entry| matches!(entry.0,
                EncounterAction::Sound { site: SoundSite::DeathPeriodic, .. }));
            assert_eq!(periodic.is_some(), expected_periodic);
            if let Some(entry) = periodic {
                assert_eq!(entry.0, EncounterAction::Sound {
                    site: SoundSite::DeathPeriodic, emitter: SoundEmitter::Slot(boss_slot), sample: 63,
                });
                assert_eq!((entry.1, entry.2, entry.3), (15, 60, -1),
                    "the periodic source tail sees the post-transition actor");
                assert_eq!(entry.4, sound_z, "sound63 precedes source22270 grounding");
            }
            assert_eq!(fixture.arena.entities[usize::from(boss_slot)].position.z, ground);
            assert_eq!(effects.sounds.contains(&10), true);
            assert_eq!(effects.sounds.contains(&63), expected_periodic);
        }
    }

    #[test]
    fn configured_pressure_sound_follows_actual_world_movement() {
        let (mut fixture, boss_slot) = boss_fixture();
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.record[71] = 7;
        boss.set_subspell_index(2);
        let frame = boss_frame(&fixture);
        let mut observed = Vec::new();
        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
            if matches!(action, EncounterAction::ReadPlayer { .. }) {
                return Ok(test_world_response(arena, action));
            }
            if let EncounterAction::Pressure(pressure) = action {
                assert_eq!(arena.entities[usize::from(boss_slot)].subspell_index & 2, 0);
                arena.entities[usize::from(pressure.player_slot)].set_position(pressure.proposed_position);
            }
            observed.push((action, arena.entities[usize::from(frame.player_slot)].position));
            Ok(EncounterResponse::Done)
        }).unwrap();
        let pressure = effects.player_pressure.unwrap();
        assert_eq!(observed, vec![
            (EncounterAction::Pressure(pressure), pressure.proposed_position),
            (EncounterAction::Sound {
                site: SoundSite::Pressure, emitter: SoundEmitter::CurrentSourceId(boss_slot), sample: 19,
            }, pressure.proposed_position),
        ]);
        assert_eq!(effects.sounds, vec![19]);
    }

    #[test]
    fn configured_source_tail_grounds_then_uses_current_player_and_frame_timing() {
        for (yaw, variant, timing, expected_yaw, expected_roll, expected_variant) in [
            (256_i16, 0_u8, 0_u8, 384_i16, 0_i16, 0_u8),
            (1664, 0, 0, 6, 0, 0),
            (1024, 2, 0, 1002, 512, 1),
            (1024, 2, 1, 1046, 1536, 1),
            (1024, 3, 0, 1024, 1024, 3),
        ] {
            let (mut fixture, boss_slot) = boss_fixture();
            let ground = fixture.terrain.ground_height(fixture.boss_position);
            let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
            boss.set_position(Position { z: ground.wrapping_add(384), ..boss.position });
            boss.set_yaw(yaw);
            boss.set_source_roll(0);
            boss.record[72] = variant;
            boss.record[48..50].copy_from_slice(&22_u16.to_le_bytes());
            let mut frame = boss_frame(&fixture);
            frame.source_frame_timing = timing;
            finish_source_tail(
                &mut fixture.arena, &mut fixture.terrain, &mut fixture.runtime,
                &fixture.entity_parameters, boss_slot, frame,
                &mut |arena, _, _, action| {
                    assert_eq!(arena.entities[usize::from(boss_slot)].position.z, ground,
                        "source22270 grounds before reading the current player");
                    assert_eq!(action, EncounterAction::ReadPlayer { slot: frame.player_slot });
                    Ok(EncounterResponse::PlayerPose { position: frame.player_position, yaw: 0 })
                },
            ).unwrap();
            let boss = &fixture.arena.entities[usize::from(boss_slot)];
            assert_eq!((boss.yaw, boss.source_roll(), boss.record[72]),
                (expected_yaw, expected_roll, expected_variant));
        }
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
        let boss_owner = fixture.arena.entities[usize::from(boss_slot)].source_id;
        let mut reached = Vec::new();
        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, runtime, action| {
            if let EncounterAction::ClearFootprint { position, width, height, owner } = action {
                let row = &arena.entities[usize::from(boss_slot)];
                assert_eq!((position, width, height, owner), (boss_position, 38, 38, boss_owner));
                assert_eq!((row.record[74], row.subspell_index, row.source_dword_10(), word(row, 48), word(row, 154)),
                    (1, 8, 15, 22, frame.player_slot), "source actor prefix is current before burial");
                assert_eq!(runtime.final_boss_active_counter(), Some(1));
                assert_eq!(i16::from_le_bytes([runtime.bytes[0x87a], runtime.bytes[0x87b]]), 60);
            }
            if !matches!(action, EncounterAction::ReadPlayer { .. }) { reached.push(action); }
            Ok(test_world_response(arena, action))
        })
        .unwrap();
        assert_eq!(reached.first(), Some(&EncounterAction::ClearFootprint {
            position: boss_position, width: 38, height: 38, owner: boss_owner,
        }));
        assert!(matches!(reached.get(1), Some(EncounterAction::Sound { site: SoundSite::Approach, sample: 10, .. })),
            "burial precedes the first source approach callback");

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
    fn source_approach_player_band_uses_xy_and_strict_2560_boundary() {
        for (dx, dz, close) in [(0_u16, 16_000_i16, true), (2_560, 0, false)] {
            let (mut fixture, boss_slot) = boss_fixture();
            let mut actor = load(&fixture.arena.entities[usize::from(boss_slot)]).unwrap();
            actor.subspell_index = 0x50;
            actor.timer = 0;
            let mut frame = boss_frame(&fixture);
            frame.player_position = Position {
                x: actor.position.x.wrapping_add(dx),
                z: actor.position.z.wrapping_add(dz),
                ..actor.position
            };
            let mut effects = Effects::default();
            approach_step(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain,
                &mut actor, frame, &mut effects, &mut ignore_world_action).unwrap();
            if close {
                assert_eq!((actor.subspell_index & 0x70, actor.timer), (0x60, 30),
                    "same-XY player remains close despite large Z difference");
            } else {
                assert_eq!((actor.subspell_index & 0x70, actor.timer), (0x10, 0),
                    "the source2560 boundary is not inside the close band");
            }
        }
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
            let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut ignore_world_action)
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
                let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut ignore_world_action)
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
        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut ignore_world_action)
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
        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut ignore_world_action)
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
        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut ignore_world_action).unwrap();
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
        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut ignore_world_action)
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
                let mut reached = Vec::new();
                let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
                    if let EncounterAction::PlayerFeedback { slot } = action {
                        assert_eq!(slot, frame.player_slot);
                        let owner = arena.entities[usize::from(boss_slot)].source_id;
                        if model == 0 {
                            assert!(arena.economy.fireball_projectiles.iter().any(|row|
                                row.active && row.origin == crate::native_world::FireballProjectileOrigin::FinalBoss
                                    && row.owner_entity_index == owner && row.source_subspell_index == 800));
                        } else {
                            assert!(arena.entities.iter().any(|row| row.active && row.class == 9
                                && row.model == model && row.source_id == owner
                                && (row.record[71], row.record[72], row.record[74]) == (10, impact, lifetime)),
                                "feedback observes the real initialized projectile and postwrites");
                        }
                    }
                    if matches!(action, EncounterAction::PlayerFeedback { .. } | EncounterAction::Sound { .. }) {
                        reached.push(action);
                    }
                        Ok(test_world_response(arena, action))
                })
                .unwrap();
                assert!(effects.summons.is_empty());
                assert_eq!(effects.sounds.contains(&sound), !full);
                let expected = if full { Vec::new() } else { vec![
                    EncounterAction::PlayerFeedback { slot: frame.player_slot },
                    EncounterAction::Sound { site: SoundSite::Attack,
                        emitter: SoundEmitter::CurrentSourceId(boss_slot), sample: sound },
                ] };
                assert_eq!(reached, expected,
                    "successful projectile postwrites precede feedback and sound; failure performs neither");
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
    fn phase12_reads_current_player_id_after_real_meteor_constructor() {
        let (mut fixture, boss_slot) = boss_fixture();
        fixture.arena.entities[usize::from(boss_slot)].set_source_byte_46(12);
        let frame = boss_frame(&fixture);
        let current_player_id = frame.player_source_id.wrapping_add(1);
        fixture.arena.entities[usize::from(frame.player_slot)].source_id = current_player_id;
        let previous_count = fixture.arena.economy.authored_model17.len();
        let mut read_after_birth = false;
        step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles,
            &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
                if let EncounterAction::ReadPlayerId { slot } = action {
                    assert_eq!(slot, frame.player_slot);
                    assert_eq!(arena.economy.authored_model17.len(), previous_count + 1);
                    let newborn = arena.economy.authored_model17.last().unwrap();
                    assert_eq!((newborn.life, newborn.max_life), (10, 10),
                        "the actual Meteor constructor precedes the player-ID read and postwrites");
                    read_after_birth = true;
                }
                Ok(test_world_response(arena, action))
            }).unwrap();
        assert!(read_after_birth);
        let meteor = fixture.arena.economy.authored_model17.last().unwrap();
        assert_eq!((meteor.life, meteor.max_life, meteor.position.z, meteor.entity_id),
            (70, 70, 0, current_player_id));
        assert_eq!(fixture.arena.economy.source_owner(meteor.source_entity_index), current_player_id);

        let (mut fixture, boss_slot) = boss_fixture();
        fixture.arena.entities[usize::from(boss_slot)].set_source_byte_46(12);
        exhaust(&mut fixture.arena);
        let frame = boss_frame(&fixture);
        let mut owner_reads = 0;
        step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles,
            &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
                if matches!(action, EncounterAction::ReadPlayerId { .. }) { owner_reads += 1; }
                Ok(test_world_response(arena, action))
            }).unwrap();
        assert_eq!(owner_reads, 0, "failed Meteor NewEvent does not read the player ID");
    }

    #[test]
    fn phase15_materializes_the_owned_volcano_and_sets_success_flag() {
        let (mut fixture, boss_slot) = boss_fixture();
        let boss_owner = fixture.arena.entities[usize::from(boss_slot)].source_id;
        let boss = &mut fixture.arena.entities[usize::from(boss_slot)];
        boss.set_source_byte_46(FINAL_REWARD_PHASE);
        boss.set_source_dword_10(1);
        fixture.runtime.bytes[68] = 0x5a;
        let frame = boss_frame(&fixture);
        let mut current_player_id = frame.player_source_id.wrapping_add(1);
        if current_player_id == boss_owner { current_player_id = current_player_id.wrapping_add(1); }
        fixture.arena.entities[usize::from(frame.player_slot)].source_id = current_player_id;
        let mut read_after_birth = false;

        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, runtime, action| {
            if let EncounterAction::ReadPlayerId { slot } = action {
                assert_eq!(runtime.bytes[68], 0, "successful constructor clears alternate Volcano mode before the player-ID read");
                let newborn = arena.entities.iter().find(|row|
                    row.active && (row.class, row.model, row.action) == (10, 9, 9))
                    .expect("real Volcano constructor has returned before the player-ID read");
                assert_eq!((newborn.life, newborn.max_life), (17, 11));
                assert_eq!(slot, frame.player_slot);
                read_after_birth = true;
            }
            Ok(test_world_response(arena, action))
        })
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
        assert!(read_after_birth);
        assert_ne!(frame.player_source_id, current_player_id);
        assert_eq!(reward.source_id, current_player_id);
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
        let mut owner_reads = 0;

        let effects = step_row(&mut fixture.arena, &mut fixture.runtime, &mut fixture.terrain, &fixture.particles, &fixture.entity_parameters, boss_slot, frame, &mut |arena, _, _, action| {
            if matches!(action, EncounterAction::ReadPlayerId { .. }) { owner_reads += 1; }
            Ok(test_world_response(arena, action))
        })
        .unwrap();

        assert_eq!(effects.final_death_reward_slot, None);
        assert_eq!(owner_reads, 0, "failed NewEvent does not read the player ID");
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
