//! Real Skeleton births requested by the source field controller.

use crate::native_world::{EntitySnapshot, source_target};

#[derive(Default)]
pub(crate) struct SkeletonStep {
    pub(crate) feedback: Vec<(u16, u8)>,
    pub(crate) target_feedback: Option<u16>,
}
use crate::{
    FlightWorld,
    campaign::source_arena::{CreatureRootOrigin, SourceArena},
    level_definition::CreatureKind,
    native_world::{ParticleParameter, SessionTerrain, campaign_shared_families},
    world::fields::skeleton_summon::Birth,
};

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct SkeletonArrowLaunch {
    parent: crate::campaign::source_arena::SourceEntityHandle,
    child: crate::campaign::source_arena::SourceEntityHandle,
    source_class: u8,
    model: u8,
    particle: u16,
    payload: u16,
    attacker: u16,
    target: u16,
}

#[cfg(test)]
std::thread_local! {
    static SKELETON_ARROW_LAUNCHES: std::cell::RefCell<Vec<SkeletonArrowLaunch>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn take_skeleton_arrow_launches() -> Vec<SkeletonArrowLaunch> {
    SKELETON_ARROW_LAUNCHES.with(|receipts| std::mem::take(&mut *receipts.borrow_mut()))
}

fn blink(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    clear: bool,
    context: source_target::Context<'_>,
) -> Result<(), String> {
    let bits = crate::native_world::level2_class5::firefly::alliance_blink_bits(
        word(row, 50),
        word(row, 52),
        row.phase,
        clear,
    );
    row.set_control_flags((row.control_flags & !0x0600_0000) | bits);
    crate::native_world::level2_class5::firefly::alliance_blink_chain(
        arena,
        row.word_34(),
        bits,
        context,
    )
}

/// Shared source damage prelude, before either movement or Alliance filtering.
fn consume(
    arena: &SourceArena,
    row: &mut EntitySnapshot,
    context: source_target::Context<'_>,
) -> Result<u8, String> {
    let mailbox = word(row, 102);
    let mut hit = 0;
    let mut attacker = 0;
    if mailbox != 0 {
        row.set_life(
            row.life
                .wrapping_sub(i32::from_le_bytes(row.record[98..102].try_into().unwrap())),
        );
        row.clear_source_upkeep_owner();
        attacker = mailbox;
        hit = 1;
    }
    let mut child = row.word_34();
    let mut visits = 0;
    while child != 0 {
        if visits >= arena.entities.len() {
            return Err("Skeleton linked damage chain cycles".into());
        }
        let Some(current) = source_target::read(arena, context, child)? else {
            return Err(format!(
                "Skeleton child address {child} is outside the source arena"
            ));
        };
        if current.life < row.life {
            row.set_life(current.life);
            attacker = current.relations.attacker.unwrap_or(0);
            hit = 1;
            break;
        }
        child = current.relations.next.unwrap_or(0);
        visits += 1;
    }
    let killer = if row.life < 0 {
        attacker
    } else {
        row.word_24()
    };
    row.set_damage_result_words(killer, attacker);
    Ok(if row.life < 0 { 2 } else { hit })
}

fn movement(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    terrain: &SessionTerrain,
) -> Result<(), String> {
    let (_, _, decrement) = campaign_shared_families::step_model9_source_movement(row, terrain)
        .map_err(|error| error.to_string())?;
    if decrement {
        arena.economy.decrement_source_creature_population();
    }
    Ok(())
}

fn bind_particle(
    row: &mut EntitySnapshot,
    particle: u16,
    particles: &[ParticleParameter],
) -> Result<(), String> {
    let descriptor = *particles
        .get(usize::from(particle))
        .ok_or_else(|| format!("Skeleton requires retained particle{particle}"))?;
    crate::world::terrain::cave_features::bind_particle(row, particle, descriptor)
}

/// Source sub_20EC0, including its same-owner refusal.
fn attack_visual(
    arena: &SourceArena,
    row: &mut EntitySnapshot,
    particles: &[ParticleParameter],
    context: source_target::Context<'_>,
) -> Result<(), String> {
    let target_slot = row.source_word_96();
    let target = if target_slot == 0 {
        arena
            .entities
            .first()
            .map(|target| (target.source_id, target.class, target.model))
    } else {
        source_target::read(arena, context, target_slot)?
            .map(|target| (target.source_id, target.class, target.model))
    }
    .ok_or_else(|| format!("Skeleton target address {target_slot} is invalid"))?;
    if row.source_id == target.0 {
        row.set_source_action(73);
    } else {
        row.set_actual_speed(0);
        bind_particle(row, campaign_shared_families::MC2_SKELETON_ASSETS.reaction_particle, particles)?;
        row.record[69] = target.1;
        row.record[70] = target.2;
    }
    Ok(())
}

/// sub_1CDA0 uses the ordinary source9/13 arrow carrier and its existing
/// collision/damage/reclaim consumer. Its tail differs from Archer's tail.
fn shoot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    row: &EntitySnapshot,
    target: source_target::View,
    particles: &[ParticleParameter],
) -> Result<(bool, Option<u16>), String> {
    use crate::native_world::creatures::archer::ArcherChildRequest;
    #[cfg(test)]
    let producer_handle = arena.handle(row.index);
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok((false, None));
    };
    let replaced_root = slot == row.index;
    let base = if replaced_root {
        crate::Position::default()
    } else {
        row.position
    };
    let request = ArcherChildRequest {
        parent_entity_index: row.index,
        parent_entity_id: if replaced_root { slot } else { row.source_id },
        parent_word_0x96: if replaced_root {
            0
        } else {
            row.source_word_96()
        },
        parent_roll: row.source_roll(),
        parent_position: base,
        parent_fov: 0,
        target_entity_index: target.slot,
        target_class: if slot == target.slot { 9 } else { target.class },
        target_model: if slot == target.slot {
            13
        } else {
            target.model
        },
        target_position: if slot == target.slot {
            base
        } else {
            target.position
        },
    };
    arena
        .economy
        .initialize_class9_model13_child(slot, request, false)
        .ok_or_else(|| format!("Skeleton arrow allocation lost source slot {slot}"))?;
    let particle = particles
        .get(usize::from(campaign_shared_families::MC2_SKELETON_ASSETS.arrow_particle))
        .ok_or("Skeleton arrow needs particle203")?;
    let position = crate::Position {
        z: base.z.wrapping_add(if replaced_root {
            // The source parent is already the newly bound particle195 row
            // when sub_1CDA0 reads its current array.roll for this lift.
            let particle = particles
                .get(195)
                .ok_or("Skeleton arrow needs particle195")?;
            (particle.speed / 2).wrapping_mul(2) as i16
        } else {
            row.array_roll()
        }),
        ..base
    };
    let child = arena
        .economy
        .authored_class5_archer_children
        .iter_mut()
        .find(|child| child.source_entity_index == slot)
        .ok_or("Skeleton arrow lost its owned carrier")?;
    child.position = position;
    child.particle_index = campaign_shared_families::MC2_SKELETON_ASSETS.arrow_particle;
    child.subspell_index = if !replaced_root && row.source_player_entity_index() != 0 {
        crate::level_definition::SKELETON_CLASS_PROPERTIES.owned_arrow_damage
    } else {
        crate::level_definition::SKELETON_CLASS_PROPERTIES.arrow_damage
    };
    // Particle array.fov is a collision axis, not the entity's angular fov.
    child.fov = 0;
    child.shift_rot_x = (particle.speed / 2).wrapping_mul(2) as i16;
    child.shift_rot_y = (particle.rotation_speed / 2).wrapping_mul(2) as i16;
    child.collision_half_width = (particle.speed / 2).wrapping_mul(2) as i16;
    child.collision_vertical_offset = (particle.rotation_speed / 2) as i16;
    child.collision_half_height = (particle.rotation_speed / 2).wrapping_mul(2) as i16;
    #[cfg(test)]
    let birth = (
        child.source_class, child.model, child.particle_index, child.subspell_index,
        child.entity_id, child.word_0x96,
    );
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    #[cfg(test)]
    if let (Some(producer), Some(child)) = (producer_handle, arena.handle(slot)) {
        SKELETON_ARROW_LAUNCHES.with(|receipts| {
            receipts.borrow_mut().push(SkeletonArrowLaunch {
                parent: producer, child, source_class: birth.0, model: birth.1,
                particle: birth.2, payload: birth.3, attacker: birth.4, target: birth.5,
            });
        });
    }
    let player_feedback =
        crate::native_world::Level2EconomyState::apply_creature_fireball_target_feedback(
            &mut arena.entities,
            target.slot,
        );
    if replaced_root {
        // sub_20C50 rereads this address after sub_1CDA0. The replacement
        // is no longer action74, so sub_20F20 applies to the current arrow.
        let particle = particles
            .get(usize::from(campaign_shared_families::MC2_SKELETON_ASSETS.movement_particle))
            .ok_or("Skeleton exit needs particle201")?;
        let child = arena
            .economy
            .authored_class5_archer_children
            .iter_mut()
            .find(|child| child.source_entity_index == slot)
            .ok_or("replacement arrow lost its carrier")?;
        child.actual_speed = child.minimum_speed;
        child.particle_index = campaign_shared_families::MC2_SKELETON_ASSETS.movement_particle;
        child.xtype = 3;
        child.xsubtype = u8::MAX;
        child.dword_0x10 = 50;
        child.collision_half_width = (particle.speed / 2) as i16;
        child.collision_vertical_offset = (particle.rotation_speed / 2) as i16;
        child.collision_half_height = (particle.rotation_speed / 2) as i16;
        child.fov = 0;
        child.shift_rot_x = child.collision_half_width;
        child.shift_rot_y = child.collision_half_height;
    }
    Ok((replaced_root, player_feedback.then_some(target.slot)))
}

/// The source action79 Alliance branch and actual action74 attack. Normal
/// action73's mimic scan remains in its existing owner; action74 is sub_20C50.
pub(crate) fn step_alliance_source_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parameters: &[[u8; 34]],
    slot: u16,
    frozen_model9: &[u16],
    context: source_target::Context<'_>,
) -> Result<SkeletonStep, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.active && row.class == 5 && row.model == 9 && matches!(row.action, 74 | 79)
        })
        .cloned()
        .ok_or_else(|| format!("Skeleton callback lost source slot {slot}"))?;
    let descriptor = parameters
        .get(usize::from(row.parameter_relation.unwrap_or(campaign_shared_families::MC2_SKELETON_ASSETS.descriptor)))
        .ok_or("Skeleton descriptor is absent")?;
    let scan_period = u16::from_le_bytes([descriptor[26], descriptor[27]]);
    let range = u16::from_le_bytes([descriptor[28], descriptor[29]]);
    let mut outcome = SkeletonStep::default();
    if row.action == 79 {
        if row.record[77] != 14 {
            return Err("Skeleton Alliance stage requires mode14".into());
        }
        let remaining = (word(&row, 50) as i16).wrapping_sub(1);
        put(&mut row, 50, remaining as u16);
        if remaining > 0 {
            blink(arena, &mut row, false, context)?;
        }
        let parent_slot = word(&row, 44);
        let parent = if remaining > 0 {
            source_target::read(arena, context, parent_slot)?.filter(|parent| parent.life > 0)
        } else {
            None
        };
        if let Some(parent) = parent {
            let saved_target = row.source_word_96();
            row.set_source_word_96(parent_slot);
            let hit = consume(arena, &mut row, context)?;
            if hit == 2 {
                put(&mut row, 50, 1);
            } else {
                movement(arena, &mut row, terrain)?;
                if hit == 1 {
                    let attacker = row.word_26();
                    let same_model = source_target::read(arena, context, attacker)?
                        .is_some_and(|actor| actor.class == 5 && actor.model == 9);
                    if attacker != parent_slot && !same_model {
                        row.set_source_word_96(attacker);
                        row.set_source_action(74);
                        outcome.feedback.push((parent_slot, 19));
                    }
                } else if row.phase & 7 == 0 {
                    if row.control_flags & 0x40000 == 0 {
                        row.set_source_roll(crate::heading_for_positions(
                            row.position,
                            parent.position,
                        ) as i16);
                        if row.phase & 63 == 0 {
                            let first = row.entity_rng.wrapping_mul(9377).wrapping_add(9439);
                            let second = first.wrapping_mul(9377).wrapping_add(9439);
                            row.set_entity_rng(second);
                            let sign = 2 * (i32::from(first % 157) / 79) - 1;
                            row.set_source_roll(
                                ((i32::from(row.source_roll())
                                    + sign * (i32::from(second & 255) + 85))
                                    & 2047) as i16,
                            );
                        }
                    }
                    for &peer in frozen_model9 {
                        let Some(peer) = source_target::read(arena, context, peer)? else {
                            continue;
                        };
                        if peer.source_id != row.source_id
                            && (i32::from(row.position.x) - i32::from(peer.position.x)).abs()
                                < i32::from(row.array_pitch())
                            && (i32::from(row.position.y) - i32::from(peer.position.y)).abs()
                                < i32::from(row.array_pitch())
                        {
                            row.set_source_roll(crate::heading_for_positions(
                                peer.position,
                                row.position,
                            ) as i16);
                            break;
                        }
                    }
                }
            }
            let selected = parent
                .relations
                .target
                .filter(|target| *target != 0)
                .or(parent.relations.attacker.filter(|target| *target != 0))
                .unwrap_or(saved_target);
            row.set_source_word_96(selected);
            if let Some(target) = source_target::read(arena, context, selected)? {
                if (target.relations.parent == Some(parent_slot)
                    && target.relations.stage_mode == Some(14))
                    || target.life <= 0
                    || target.control_flags & 0x400 != 0
                {
                    row.set_source_word_96(0);
                } else if crate::distance_3d_wrapped(row.position, target.position)
                    < u32::from(range)
                {
                    row.set_source_action(74);
                    outcome.feedback.push((parent_slot, 24));
                }
            }
        } else {
            row.record[77] = 10;
            row.record[76] = word(&row, 78) as u8;
            put(&mut row, 44, 0);
            row.set_source_word_96(0);
            blink(arena, &mut row, true, context)?;
        }
        if row.action == 74 {
            attack_visual(arena, &mut row, particles, context)?;
        }
    } else {
        let hit = consume(arena, &mut row, context)?;
        if hit == 2 {
            row.set_source_action(76);
        } else if hit == 1 {
            row.set_source_word_96(row.word_26());
        } else {
            movement(arena, &mut row, terrain)?;
            let mut allowed = true;
            if row.record[77] == 14 {
                blink(arena, &mut row, false, context)?;
                let remaining = (word(&row, 50) as i16).wrapping_sub(1);
                put(&mut row, 50, remaining as u16);
                allowed = remaining > 0;
                if allowed
                    && let Some(parent) = source_target::read(arena, context, word(&row, 44))?
                {
                    let selected = parent
                        .relations
                        .target
                        .filter(|target| *target != 0)
                        .or(parent.relations.attacker.filter(|target| *target != 0))
                        .unwrap_or(0);
                    allowed = row.source_word_96() != parent.slot
                        && (selected == 0 || selected == row.source_word_96());
                }
            }
            let target = if allowed {
                source_target::read(arena, context, row.source_word_96())?
            } else {
                None
            };
            if let Some(target) =
                target.filter(|target| target.life >= 0 && target.control_flags & 0x400 == 0)
            {
                if row.phase % 10 == 0 {
                    row.set_source_roll(
                        crate::heading_for_positions(row.position, target.position) as i16,
                    );
                }
                if scan_period == 0 {
                    return Err("Skeleton source attack scan period is zero".into());
                }
                if u16::from(row.phase) % scan_period == 0 {
                    let target_range = i32::from(range)
                        + if target.class == 3 && target.model == 2 {
                            target
                                .collision
                                .map_or(0, |shape| i32::from(shape.half_width))
                        } else {
                            0
                        };
                    if i64::from(crate::distance_3d_wrapped(row.position, target.position))
                        < i64::from(target_range)
                    {
                        // Publish movement before the source allocator can
                        // replace the very address this callback is using.
                        arena.entities[usize::from(slot)] = row.clone();
                        crate::world::effects::sync_position(
                            arena,
                            terrain,
                            slot,
                            row.position,
                            true,
                            false,
                        )?;
                        let (replaced, feedback) = shoot(arena, terrain, &row, target, particles)?;
                        outcome.target_feedback = feedback;
                        if replaced {
                            return Ok(outcome);
                        }
                    } else {
                        row.set_source_action(73);
                    }
                }
            } else {
                row.set_source_action(73);
            }
        }
        if row.action != 74 {
            row.set_actual_speed(row.minimum_speed());
            bind_particle(&mut row, campaign_shared_families::MC2_SKELETON_ASSETS.movement_particle, particles)?;
            row.record[69] = 3;
            row.record[70] = u8::MAX;
            row.set_source_dword_10(50);
            row.set_source_byte_46(0);
        }
    }
    let position = row.position;
    row.set_map_links(
        arena.economy.map_chains.old_map_entity(slot),
        arena.economy.map_chains.next_entity(slot),
    );
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(outcome)
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct ActorStageVisit {
    source: crate::campaign::source_arena::SourceEntityHandle,
    before: EntitySnapshot,
    after: EntitySnapshot,
    target: Option<(
        crate::campaign::source_arena::SourceEntityHandle,
        source_target::View,
    )>,
    follow_heading: Option<i16>,
}

#[cfg(test)]
std::thread_local! {
    static ACTOR_STAGE_VISITS: std::cell::RefCell<Vec<ActorStageVisit>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// sub_20FC0 -> sub_1D7C0/1D700/1D8C0: follow a current actor,
/// optionally attacking it (mode3) or its target (mode4). The shared route
/// prepass owns cache retirement and continuation; this callback never treats
/// a model selector in the StageVar union as a point coordinate.
pub(crate) fn step_actor_stage_in_arena(
    arena: &mut SourceArena,
    runtime: &crate::native_world::RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    parameters: &[[u8; 34]],
    slot: u16,
    frozen_models: &[Vec<u16>; 29],
    context: source_target::Context<'_>,
) -> Result<(), String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.active
                && (row.class, row.model, row.action) == (5, 9, 79)
                && matches!(row.record[77], 3..=5)
        })
        .cloned()
        .ok_or_else(|| format!("Skeleton {slot} lost its actor stage"))?;
    let mode = row.record[77];
    let stage_index = row.record[76];
    let stage = runtime
        .stage_variables
        .get(usize::from(stage_index))
        .filter(|_| stage_index != 0)
        .ok_or_else(|| format!("Skeleton {slot} lost actor StageVar {stage_index}"))?;
    let descriptor = parameters
        .get(usize::from(row.parameter_relation.unwrap_or(
            campaign_shared_families::MC2_SKELETON_ASSETS.descriptor,
        )))
        .ok_or_else(|| format!("Skeleton {slot} lacks its actor descriptor"))?;
    if descriptor[32] & 8 != 0 {
        return Err(format!(
            "Skeleton {slot} actor descriptor selects unsupported flee action78"
        ));
    }
    #[cfg(test)]
    let before = row.clone();
    #[cfg(test)]
    let mut observed_target = None;
    #[cfg(test)]
    let mut follow_heading = None;

    let hit = consume(arena, &mut row, context)?;
    match hit {
        0 => {
            movement(arena, &mut row, terrain)?;
            if row.phase & 7 == 0 {
                let target_slot = if stage.flags as u8 & 2 != 0 {
                    if word(&row, 78) == 0 {
                        let mut selected = 0;
                        if stage.flags as u8 & 1 != 0 {
                            for &peer in &frozen_models[9] {
                                let Some(peer) = source_target::read(arena, context, peer)? else {
                                    continue;
                                };
                                if peer.relations.stage_index == Some(stage_index)
                                    && let Some(cached) =
                                        peer.relations.stage_relation.filter(|slot| *slot != 0)
                                {
                                    selected = cached;
                                    break;
                                }
                            }
                        }
                        if selected == 0 {
                            let model = match stage.relation {
                                crate::native_world::RuntimeRelation::AxisScalar(model)
                                | crate::native_world::RuntimeRelation::Scalar(model) => model,
                                other => {
                                    return Err(format!(
                                        "Skeleton {slot} actor selector is {other:?}"
                                    ));
                                }
                            };
                            let mut nearest_distance = u32::MAX;
                            for &candidate in
                                frozen_models.get(model as usize).into_iter().flatten()
                            {
                                let Some(target) = source_target::read(arena, context, candidate)?
                                else {
                                    continue;
                                };
                                let dx = i32::from(
                                    target.position.x.wrapping_sub(row.position.x) as i16
                                );
                                let dy = i32::from(
                                    target.position.y.wrapping_sub(row.position.y) as i16
                                );
                                let distance =
                                    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                                if distance < nearest_distance {
                                    nearest_distance = distance;
                                    selected = candidate;
                                }
                            }
                        }
                        put(&mut row, 78, selected);
                    }
                    word(&row, 78)
                } else {
                    match stage.relation {
                        crate::native_world::RuntimeRelation::LiveEntity(slot)
                        | crate::native_world::RuntimeRelation::LiveEntityIndex(slot) => slot,
                        crate::native_world::RuntimeRelation::Null => 0,
                        other => return Err(format!("Skeleton {slot} actor pointer is {other:?}")),
                    }
                };
                if let Some(target) = source_target::read(arena, context, target_slot)? {
                    #[cfg(test)]
                    {
                        observed_target = arena.handle(target_slot).map(|handle| (handle, target));
                    }
                    if row.control_flags & 0x0004_0000 == 0 {
                        row.set_source_roll(crate::heading_for_positions(
                            row.position,
                            target.position,
                        ) as i16);
                        #[cfg(test)]
                        {
                            follow_heading = Some(row.source_roll());
                        }
                        if row.phase & 63 == 0 {
                            let first = campaign_shared_families::next_source_rng16(row.entity_rng);
                            let second = campaign_shared_families::next_source_rng16(first);
                            row.set_entity_rng(second);
                            let sign = 2 * (i32::from(first % 157) / 79) - 1;
                            row.set_source_roll(
                                ((i32::from(row.source_roll())
                                    + sign * (i32::from(second & 255) + 85))
                                    & 2047) as i16,
                            );
                        }
                    }
                    let width = i32::from(row.array_pitch());
                    for &peer in &frozen_models[9] {
                        let Some(peer) = source_target::read(arena, context, peer)? else {
                            continue;
                        };
                        if peer.source_id != row.source_id
                            && (i32::from(row.position.x) - i32::from(peer.position.x)).abs()
                                < width
                            && (i32::from(row.position.y) - i32::from(peer.position.y)).abs()
                                < width
                        {
                            row.set_source_roll(crate::heading_for_positions(
                                peer.position,
                                row.position,
                            ) as i16);
                            break;
                        }
                    }
                    let overlap_width = width
                        + target
                            .collision
                            .map_or(0, |shape| i32::from(shape.half_width));
                    if (i32::from(row.position.x) - i32::from(target.position.x)).abs()
                        < overlap_width
                        && (i32::from(row.position.y) - i32::from(target.position.y)).abs()
                            < overlap_width
                    {
                        row.set_source_roll(crate::heading_for_positions(
                            target.position,
                            row.position,
                        ) as i16);
                    }
                } else if row.phase & 15 == 0 && row.control_flags & 0x0004_0000 == 0 {
                    let rng = campaign_shared_families::next_source_rng16(row.entity_rng);
                    row.set_entity_rng(rng);
                    row.set_source_roll(
                        row.source_roll().wrapping_add((rng % 113 + 142) as i16) & 2047,
                    );
                }
            }
        }
        1 => {
            let attacker_slot = row.word_26();
            let attacker_kind = if attacker_slot == 0 {
                arena
                    .entities
                    .first()
                    .map(|sentinel| (sentinel.class, sentinel.model))
            } else {
                source_target::read(arena, context, attacker_slot)?
                    .map(|attacker| (attacker.class, attacker.model))
            }
            .ok_or_else(|| format!("Skeleton {slot} lost damage attacker {attacker_slot}"))?;
            if attacker_kind != (row.class, row.model) {
                row.set_source_word_96(attacker_slot);
                row.record[77] = 10;
                row.set_source_action(74);
            }
            let ground = terrain.ground_height(row.position);
            let clearance = i16::from_le_bytes([descriptor[12], descriptor[13]]);
            let descent = i16::from_le_bytes([descriptor[14], descriptor[15]]);
            if row.position.z > ground {
                row.position.z = row.position.z.wrapping_add(descent);
            }
            if i32::from(row.position.z as i16) <= i32::from(ground) + i32::from(clearance) {
                row.position.z = ground.wrapping_add(clearance);
            }
            row.set_position(row.position);
        }
        _ => row.set_source_action(76),
    }
    // The mode3/4 wrapper still runs this tail after damage or pre-kill.
    // It rereads the existing cache; it does not perform another acquisition.
    if matches!(mode, 3 | 4) && row.phase & 7 == 0 {
        let actor_slot = if stage.flags as u8 & 2 != 0 {
            word(&row, 78)
        } else {
            match stage.relation {
                crate::native_world::RuntimeRelation::LiveEntity(slot)
                | crate::native_world::RuntimeRelation::LiveEntityIndex(slot) => slot,
                _ => 0,
            }
        };
        if let Some(actor) = source_target::read(arena, context, actor_slot)? {
            let target = if mode == 3 {
                Some(actor)
            } else {
                match actor.relations.target.filter(|slot| *slot != 0) {
                    Some(target) => source_target::read(arena, context, target)?,
                    None => None,
                }
            };
            if let Some(target) = target
                && crate::distance_3d_wrapped(row.position, target.position)
                    <= u32::from(u16::from_le_bytes([descriptor[28], descriptor[29]]))
            {
                row.set_source_word_96(target.slot);
                row.record[77] = 10;
                row.set_source_action(74);
            }
        }
    }
    if row.action == 74 {
        attack_visual(arena, &mut row, particles, context)?;
    }
    #[cfg(test)]
    ACTOR_STAGE_VISITS.with(|visits| {
        let mut visits = visits.borrow_mut();
        if visits.len() < 128 {
            visits.push(ActorStageVisit {
                source: arena.handle(slot).unwrap(),
                before,
                after: row.clone(),
                target: observed_target,
                follow_heading,
            });
        }
    });
    let position = row.position;
    row.set_map_links(
        arena.economy.map_chains.old_map_entity(slot),
        arena.economy.map_chains.next_entity(slot),
    );
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
}

/// Read word94 through the current occupant of a frozen model9-list address.
/// Membership is historical; ownership is read anew, including after reuse.
pub(crate) fn current_source_player_entity_index(
    arena: &SourceArena,
    slot: u16,
) -> Result<Option<u16>, String> {
    use crate::native_world::Level2DynamicArenaKind as Kind;

    if slot == 0 {
        return Ok(None);
    }
    let Some(raw) = arena.entities.get(usize::from(slot)) else {
        return Ok(None);
    };
    let kind = arena
        .economy
        .live_arena_cursor_row(slot)
        .map(|cursor| cursor.kind)
        .or_else(|| {
            (!arena.economy.free_source_slots().contains(&slot))
                .then(|| arena.economy.retained_projectile_kind(slot))
                .flatten()
        });
    let Some(kind) = kind else {
        return Ok(Some(raw.source_player_entity_index()));
    };
    let owner = match kind {
        Kind::ManaSphere => arena
            .economy
            .spheres()
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
            .map(|row| row.player_entity_index)
            .ok_or_else(|| format!("Skeleton count lost current sphere at {slot}"))?,
        Kind::CastleRoot => arena
            .economy
            .castle_at(slot)
            .map(|row| row.player_entity_index)
            .ok_or_else(|| format!("Skeleton count lost current Castle at {slot}"))?,
        // Typed creature/common-upkeep ownership is held by the economy's
        // word94 carrier. NewEvent clears that carrier before every reuse;
        // an absent entry is the new occupant's zero, never an old raw row.
        _ => arena.economy.source_owner(slot),
    };
    Ok(Some(owner))
}

/// Source sub_357C0 calls the ordinary sub_4BBB0 constructor, then writes
/// ownership, mana and pose. Rejected placement still consumes phase and RNG.
pub(crate) fn allocate_from_controller(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    descriptor: &[u8; 34],
    birth: Birth,
) -> Result<bool, String> {
    let controller = arena
        .handle(birth.controller_slot)
        .ok_or("Skeleton birth lost its field controller")?;
    let particle = particles
        .get(usize::from(campaign_shared_families::MC2_SKELETON_ASSETS.initial_particle))
        .ok_or("Skeleton constructor needs particle220")?;
    let animation_limit = campaign_shared_families::particle_animation_limit(particle.render_mode)
        .ok_or("Skeleton particle220 has no source animation limit")?;
    let pose = campaign_shared_families::ParticlePose {
        animation_limit,
        array_yaw: (particle.rotation_speed / 2) as i16,
        array_pitch: (particle.speed / 2) as i16,
        array_roll: (particle.speed / 2) as i16,
        array_fov: (particle.rotation_speed / 2) as i16,
    };
    let scan_period = u16::from_le_bytes([descriptor[26], descriptor[27]]);
    if scan_period == 0 {
        return Err("Skeleton descriptor80 has a zero scan period".into());
    }
    let material_mask = u32::from_le_bytes(descriptor[20..24].try_into().unwrap());
    let ground_clearance = i16::from_le_bytes([descriptor[12], descriptor[13]]);
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(false);
    };
    let phase = arena.economy.class5_model9_constructor_phase;
    arena.economy.class5_model9_constructor_phase = phase.wrapping_add(1);
    let mut position = birth.position;
    position.z = terrain.ground_height(position);
    let row = campaign_shared_families::construct_skeleton_source(
        slot,
        arena.economy.new_event_rng(slot),
        position,
        phase,
        scan_period,
        pose,
    );
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if campaign_shared_families::skeleton_placement_blocked(
        terrain,
        position,
        material_mask,
        ground_clearance,
    ) {
        let rng = arena.entities[usize::from(slot)].entity_rng;
        crate::world::effects::sync_position(arena, terrain, slot, position, false, false)?;
        arena.entities[usize::from(slot)].reclaim_source_row();
        arena.economy.release_slot(slot, Some(rng));
        return Ok(false);
    }
    // NewEvent can reoccupy the controller's fixed address. Source reads its
    // id only after construction; the historical generation stays provenance.
    let owner = arena.entities[usize::from(birth.controller_slot)].source_id;
    let row = &mut arena.entities[usize::from(slot)];
    row.source_id = owner;
    row.record[30..32].copy_from_slice(&owner.to_le_bytes());
    row.set_source_player_entity_index(owner);
    row.set_mana(i32::from(birth.mana));
    row.set_yaw(birth.yaw);
    row.set_source_roll(birth.yaw);
    let child = arena
        .handle(slot)
        .ok_or("Skeleton birth lost its allocated address")?;
    if !arena.bind_raw_creature_origin(
        child,
        CreatureKind::Skeleton,
        CreatureRootOrigin::FieldSummon { controller },
    ) {
        return Err("Skeleton birth lost its generation-bound field origin".into());
    }
    Ok(true)
}

#[cfg(test)]
mod alliance_tests {
    use super::*;

    #[test]
    fn ordinary_level_five_skeleton_emergence_follows_current_archer() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{Mc2DispatchEvent, RuntimeRelation};

        let package = std::env::var_os("MC_LEVEL5_PACKAGE").map_or_else(
            || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/dragon-bee-20261105-integration/level5-owner-current/level5"),
            std::path::PathBuf::from,
        );
        let campaign = CampaignSession::open(package, CampaignSelection::ordinary(5).unwrap())
            .expect("unmodified whole Level5");
        let mut session = campaign.simulation().clone();
        let reset_hash = session.state_hash();
        let roots = session.source_arena().rebuilt_class5_model_slots()[9]
            .iter()
            .map(|&slot| session.source_arena().handle(slot).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 39);
        assert_eq!(
            session.source_arena().rebuilt_class5_model_slots()[4].len(),
            33
        );
        let route = session.runtime().stage_variables[1];
        assert_eq!(
            (route.kind, route.flags, route.relation),
            (3, 3, RuntimeRelation::AxisScalar(4))
        );
        assert!(roots.iter().all(|root| {
            let row = &session.entities()[usize::from(root.source_slot)];
            row.action == 72 && word(row, 78) == 1
        }));
        ACTOR_STAGE_VISITS.with(|visits| visits.borrow_mut().clear());
        let mut followed = std::collections::BTreeSet::new();
        let mut retained_cache = false;
        let mut moved = false;
        let mut distinguished_point = false;
        for turn in 0..40 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                session
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "ordinary actor route turn{turn}: {:?}",
                session.dispatch_events()
            );
            let visits =
                ACTOR_STAGE_VISITS.with(|visits| std::mem::take(&mut *visits.borrow_mut()));
            for visit in visits {
                assert!(roots.contains(&visit.source));
                assert_eq!(
                    session.source_arena().handle(visit.source.source_slot),
                    Some(visit.source)
                );
                assert_eq!((visit.before.record[76], visit.before.record[77]), (1, 3));
                if let Some((target_handle, target)) = visit.target {
                    assert_eq!((target.class, target.model), (5, 4));
                    assert_eq!(word(&visit.after, 78), target_handle.source_slot);
                    assert_eq!(
                        session.source_arena().handle(target_handle.source_slot),
                        Some(target_handle)
                    );
                    assert!(
                        session
                            .creature_identity_for_source_slot(target_handle.source_slot)
                            .is_some_and(
                                |identity| identity.kind == CreatureKind::Archer && identity.living
                            )
                    );
                    followed.insert(visit.source.source_slot);
                    if let Some(heading) = visit.follow_heading {
                        assert_eq!(
                            heading,
                            crate::heading_for_positions(visit.after.position, target.position)
                                as i16,
                            "steering reads the current actor after Skeleton movement"
                        );
                        distinguished_point |= heading
                            != crate::heading_for_positions(
                                visit.after.position,
                                crate::Position { x: 4, y: 0, z: 0 },
                            ) as i16;
                    }
                    moved |= (visit.before.position.x, visit.before.position.y)
                        != (visit.after.position.x, visit.after.position.y);
                }
                retained_cache |= word(&visit.before, 78) != 0
                    && word(&visit.before, 78) == word(&visit.after, 78);
                assert!(
                    session
                        .economy()
                        .map_chains()
                        .is_linked(visit.source.source_slot)
                );
                let current = source_target::read(
                    session.source_arena(),
                    Default::default(),
                    visit.source.source_slot,
                )
                .unwrap()
                .unwrap();
                assert_eq!(current.position, visit.after.position);
                assert_eq!(
                    current.relations.stage_relation,
                    Some(word(&visit.after, 78))
                );
                assert_eq!(
                    session.entities()[usize::from(visit.source.source_slot)]
                        .particle_parameter_index,
                    Some(campaign_shared_families::MC2_SKELETON_ASSETS.movement_particle)
                );
            }
            if followed.len() == roots.len() && retained_cache && moved && distinguished_point {
                break;
            }
        }
        assert_eq!(
            followed.len(),
            roots.len(),
            "every genuine reset root emerges into actor follow"
        );
        assert!(retained_cache && moved && distinguished_point);
        let presented_slot = *followed.first().unwrap();
        let row = &session.entities()[usize::from(presented_slot)];
        let sprite = session
            .campaign_entity_presentations(0)
            .unwrap()
            .into_iter()
            .find(|sprite| sprite.entity_index == presented_slot)
            .expect("actual moving Skeleton sprite");
        assert_eq!(
            (
                sprite.position,
                sprite.source_id,
                sprite.render_parameter_index
            ),
            (
                row.position,
                row.source_id,
                campaign_shared_families::MC2_SKELETON_ASSETS.movement_particle
            )
        );
        session.reset();
        assert_eq!(session.state_hash(), reset_hash);
    }

    #[test]
    fn configured_actor_follow_damage_tail_and_route_continuation_preserve_source_order() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        let package = std::env::var_os("MC_LEVEL5_PACKAGE").map_or_else(
            || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/dragon-bee-20261105-integration/level5-owner-current/level5"),
            std::path::PathBuf::from,
        );
        let mut campaign =
            CampaignSession::open(package, CampaignSelection::ordinary(5).unwrap()).unwrap();
        for _ in 0..32 {
            let _ = campaign.step(PlayerCommand::default(), 0);
        }
        let session = campaign.simulation();
        let source = session
            .entities()
            .iter()
            .find(|row| {
                row.active
                    && (row.class, row.model, row.action, row.record[77]) == (5, 9, 79, 3)
                    && word(row, 78) != 0
            })
            .unwrap()
            .clone();
        let slot = source.index;
        let target = word(&source, 78);
        let player = session.state.entity_index;
        let particles = &session.session.definition.presentation.particles;
        let parameters = &session.session.definition.presentation.entity_parameters;
        let original_runtime = &session.session.state.live.core.runtime;
        let target_position =
            source_target::read(session.source_arena(), Default::default(), target)
                .unwrap()
                .unwrap()
                .position;
        // Declared near/far and damage exception controls, separate from the
        // ordinary untouched Level5 emergence/follow check above.
        for (damage, near) in [(0, true), (100, true), (1001, true), (1001, false)] {
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            let mut runtime = original_runtime.clone();
            let mut position = target_position;
            position.x = position.x.wrapping_add(if near { 256 } else { 4096 });
            position.z = terrain.ground_height(position);
            let row = &mut arena.entities[usize::from(slot)];
            row.set_position(position);
            row.set_phase(8);
            if damage != 0 {
                row.apply_model76_mailbox(player, damage);
            }
            let models = arena.rebuilt_class5_model_slots();
            step_actor_stage_in_arena(
                &mut arena,
                &runtime,
                &mut terrain,
                particles,
                parameters,
                slot,
                &models,
                Default::default(),
            )
            .unwrap();
            let row = &arena.entities[usize::from(slot)];
            assert_eq!(row.life, 1000 - i32::from(damage));
            assert_eq!(row.source_upkeep_word_62(), 0);
            if near {
                assert_eq!(
                    (row.action, row.record[77], row.source_word_96()),
                    (74, 10, target),
                    "mode3's near tail still follows hit or lethal common-prefix return"
                );
                assert_eq!(
                    row.particle_parameter_index,
                    Some(campaign_shared_families::MC2_SKELETON_ASSETS.reaction_particle)
                );
            } else {
                assert_eq!(row.action, 76);
            }
            if damage != 0 {
                assert_eq!(
                    (row.position.x, row.position.y),
                    (position.x, position.y),
                    "damage suppresses movement and fresh acquisition"
                );
                assert_eq!(row.word_26(), player);
                if damage == 1001 {
                    assert_eq!(row.word_24(), player);
                }
            }
            if damage == 100 {
                // The actual attack callback leaves a now-distant target;
                // the next shared source prepass restores its original route.
                let mut far = target_position;
                far.x = far.x.wrapping_add(4096);
                far.z = terrain.ground_height(far);
                arena.entities[usize::from(slot)].set_position(far);
                arena.entities[usize::from(slot)].set_phase(25);
                step_alliance_source_slot(
                    &mut arena,
                    &mut terrain,
                    particles,
                    parameters,
                    slot,
                    &models[9],
                    Default::default(),
                )
                .unwrap();
                assert_eq!(arena.entities[usize::from(slot)].action, 73);
                super::super::stage_route::prepare_source_pass(
                    &mut arena,
                    &mut runtime,
                    Default::default(),
                    &[slot],
                )
                .unwrap();
                let resumed = &arena.entities[usize::from(slot)];
                assert_eq!(
                    (
                        resumed.action,
                        resumed.record[76],
                        resumed.record[77],
                        word(resumed, 78)
                    ),
                    (79, 1, 3, 0)
                );
            }
        }
        // Configured linked-life exception: sub_1D8C0 may inherit attacker0,
        // then dereferences the actual sentinel rather than refusing damage.
        // No ordinary package linked-Skeleton producer is claimed here.
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let child = arena.rebuilt_class5_model_slots()[9]
            .iter()
            .copied()
            .find(|child| *child != slot)
            .unwrap();
        let row = &mut arena.entities[usize::from(slot)];
        row.record[77] = 5;
        row.set_phase(9);
        put(row, 56, child);
        let linked = &mut arena.entities[usize::from(child)];
        linked.set_life(999);
        linked.set_damage_result_words(0, 0);
        put(linked, 56, 0);
        let models = arena.rebuilt_class5_model_slots();
        step_actor_stage_in_arena(
            &mut arena,
            original_runtime,
            &mut terrain,
            particles,
            parameters,
            slot,
            &models,
            Default::default(),
        )
        .unwrap();
        let row = &arena.entities[usize::from(slot)];
        assert_eq!(
            (
                row.life,
                row.word_26(),
                row.source_word_96(),
                row.action,
                row.record[77]
            ),
            (999, 0, 0, 74, 10)
        );
        assert_eq!(
            (row.record[69], row.record[70]),
            (arena.entities[0].class, arena.entities[0].model)
        );

        // The direct-pointer union also admits null. It ignores any old
        // cached model-selection relation and takes the no-target wander
        // branch only on the source's sixteen-phase cadence.
        for phase in [8, 16, 17] {
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            let mut runtime = original_runtime.clone();
            runtime.stage_variables[1].flags = 0;
            runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::Null;
            arena.entities[usize::from(slot)].set_phase(phase);
            let before = arena.entities[usize::from(slot)].clone();
            let models = arena.rebuilt_class5_model_slots();
            step_actor_stage_in_arena(
                &mut arena,
                &runtime,
                &mut terrain,
                particles,
                parameters,
                slot,
                &models,
                Default::default(),
            )
            .unwrap();
            let after = &arena.entities[usize::from(slot)];
            assert_eq!(
                (
                    after.action,
                    after.record[77],
                    word(after, 78),
                    after.source_word_96()
                ),
                (79, 3, word(&before, 78), before.source_word_96())
            );
            let draws = phase & 15 == 0 && after.control_flags & 0x0004_0000 == 0;
            let expected_rng = if draws {
                campaign_shared_families::next_source_rng16(before.entity_rng)
            } else {
                before.entity_rng
            };
            assert_eq!(after.entity_rng, expected_rng);
            assert_eq!(
                after.source_roll(),
                if draws {
                    before
                        .source_roll()
                        .wrapping_add((expected_rng % 113 + 142) as i16)
                        & 2047
                } else {
                    before.source_roll()
                }
            );
        }
        for relation in [
            crate::native_world::RuntimeRelation::LiveEntity(target),
            crate::native_world::RuntimeRelation::LiveEntityIndex(target),
        ] {
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            let mut runtime = original_runtime.clone();
            runtime.stage_variables[1].flags = 0;
            runtime.stage_variables[1].relation = relation;
            arena.entities[usize::from(slot)].set_phase(8);
            let models = arena.rebuilt_class5_model_slots();
            ACTOR_STAGE_VISITS.with(|visits| visits.borrow_mut().clear());
            step_actor_stage_in_arena(
                &mut arena,
                &runtime,
                &mut terrain,
                particles,
                parameters,
                slot,
                &models,
                Default::default(),
            )
            .unwrap();
            let visits =
                ACTOR_STAGE_VISITS.with(|visits| std::mem::take(&mut *visits.borrow_mut()));
            assert!(visits.iter().any(|visit| {
                visit.target.is_some_and(|(handle, current)| {
                    handle.source_slot == target && (current.class, current.model) == (5, 4)
                })
            }));
        }

        // This configured descriptor cannot select the explicitly unsupported
        // action78. Reject it before damage, motion, RNG or route writes.
        let mut arena = session.source_arena().clone();
        let before = arena.clone();
        let mut terrain = session.canonical_terrain().clone();
        let mut unsupported = parameters.to_vec();
        let descriptor = usize::from(
            source
                .parameter_relation
                .unwrap_or(campaign_shared_families::MC2_SKELETON_ASSETS.descriptor),
        );
        unsupported[descriptor][32] |= 8;
        let models = arena.rebuilt_class5_model_slots();
        let error = step_actor_stage_in_arena(
            &mut arena,
            original_runtime,
            &mut terrain,
            particles,
            &unsupported,
            slot,
            &models,
            Default::default(),
        )
        .unwrap_err();
        assert!(error.contains("unsupported flee action78"));
        assert_eq!(arena, before);
    }

    #[test]
    fn ordinary_level_seven_paid_fireballs_kill_and_reclaim_skeleton() {
        use crate::campaign::{
            CampaignSession,
            schedule::{CurrentOccupant, ScheduleReceipt},
            spec::CampaignSelection,
        };
        use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
        use crate::{PlayerCommand, Position};
        use std::{collections::BTreeSet, path::PathBuf};

        let package = std::env::var_os("MC_LEVEL7_HOUSE_PACKAGE").map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/settlement-20261105-integration/owner-collision-closure-v4/level7"),
            PathBuf::from,
        );
        let mut session = CampaignSession::open(package, CampaignSelection::ordinary(7).unwrap())
            .expect("whole Level7 owner package")
            .simulation()
            .clone();
        let player = session.state.entity_index;
        let player_handle = session.source_arena().handle(player).unwrap();
        session.apply_carried_spell_families(&[0], Some(0)).unwrap();
        {
            // Declared player-only resources; no source actor state is staged.
            let live = &mut session.session.state.live;
            live.economy
                .economy
                .owner_base_max_mana
                .insert(player, 200_000);
            live.economy
                .economy
                .rebuild_source_owner_max_mana_projection();
            let capacity = live.economy.economy.owner_max_mana_capacity(player);
            live.player_life = 200_000;
            live.shared_spells.set_player_entity_life(200_000);
            live.shared_spells.set_local_player_mana_for_test(200_000);
            let parent = &mut live.economy.entities[usize::from(player)];
            parent.set_life(200_000);
            parent.set_mana(200_000);
            parent.max_mana = capacity;
            parent.record[144..148].copy_from_slice(&capacity.to_le_bytes());
            let parent = parent.clone();
            live.shared_spells
                .sync_player_entity_from_authority(&parent);
            live.shared_spells
                .sync_typed_spell_parent_caches_from_player_entity();
            if let Some(fire) = live.core.context.spell_fire.as_mut() {
                fire.caster.mana = 200_000;
            }
        }
        for _ in 0..256 {
            if session.session.state.live.player_protection_ticks == 0 {
                break;
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert_eq!(session.session.state.live.player_protection_ticks, 0);
        let switch = session.entities()[279].clone();
        assert_eq!(
            (switch.class, switch.model, switch.action, switch.source_id),
            (11, 0, 0, 19)
        );
        session
            .set_level2_entity_position(player, switch.position)
            .unwrap();
        let mut skeleton = None;
        for _ in 0..32 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            if session.dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 279, group: 19, authored_indices, .. }
                    if authored_indices.contains(&454)))
            {
                skeleton = session.creature_runtime_census().roots_of_kind(CreatureKind::Skeleton)
                    .find(|row| row.living && row.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 454 }))
                    .map(|row| row.handle);
                break;
            }
        }
        let skeleton = skeleton.expect("ordinary switch279 publishes Skeleton454");
        assert!(
            session.source_arena().rebuilt_class5_model_slots()[9].contains(&skeleton.source_slot)
        );
        let fire = session.shared_spells().local_player().enabled_spell_holders[0];
        let fire_ui = crate::SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|family| *family == 0)
            .unwrap() as i8;
        let mut fixed_pose = None;
        let mut paid = BTreeSet::new();
        let mut queued: Option<(Vec<(u16, u32, u16)>, i32)> = None;
        let mut births = BTreeSet::new();
        let mut callbacks = Vec::new();
        let mut fatal = None;
        let mut damage_visits = Vec::new();
        let mut saw_76 = false;
        let mut saw_77 = false;
        let mut sphere = false;
        let mut burst = false;
        let mut hidden = false;
        let mut reclaimed = false;
        let mut following_target_loss = false;
        let mut no_skeleton_kill_increment = false;
        let mut trace = Vec::new();
        for turn in 0..360 {
            let same = session.source_arena().handle(skeleton.source_slot) == Some(skeleton);
            if !same {
                if hidden {
                    reclaimed = true;
                    break;
                }
                panic!("Skeleton generation changed before hide: {trace:?}");
            }
            let before = session.entities()[usize::from(skeleton.source_slot)].clone();
            let mailbox = (
                i32::from_le_bytes(before.record[98..102].try_into().unwrap()),
                u16::from_le_bytes(before.record[102..104].try_into().unwrap()),
            );
            let callback_start = callbacks.len();
            let was_fatal = fatal.is_some();
            let credit_before = session.economy().authored_bee_kill_census(player);
            let ready = session.shared_spells().pickup(fire).is_some_and(|holder| {
                holder.active_value == 0
                    && session.shared_spells().player_entity().mana >= holder.max_mana
            });
            let mut command = PlayerCommand::default();
            if fatal.is_none()
                && before.life >= 0
                && ready
                && before.action == 73
                && fixed_pose.is_none()
            {
                // One fixed player-only firing line: left-hand offset is applied
                // by the public command, and the real projectile moves 384 on its first visit.
                let yaw = 0_i16;
                let mut launch = before.position;
                crate::move_entity(&mut launch, yaw.wrapping_add(1024), 0, 384);
                let mut position = launch;
                let parent = session.shared_spells().player_entity();
                let parent_yaw = yaw
                    .wrapping_sub(session.shared_spells().beyond_sight_auxiliary().next_entity)
                    & 0x07ff;
                crate::move_entity(&mut position, parent_yaw.wrapping_add(512), 0, 256);
                let centre = before.position.z.wrapping_add(before.array_yaw());
                position.z = centre
                    .wrapping_sub(parent.array_fov())
                    .max(
                        session
                            .canonical_terrain()
                            .ground_height(position)
                            .wrapping_add(256),
                    )
                    .max(
                        session
                            .canonical_terrain()
                            .ground_height(launch)
                            .wrapping_add(256),
                    );
                launch.z = position.z.wrapping_add(parent.array_fov());
                let (_, pitch) = crate::level1_mission::projectile_guidance_angles(
                    launch,
                    Position {
                        z: centre,
                        ..before.position
                    },
                );
                session
                    .set_level2_entity_position(player, position)
                    .unwrap();
                session.set_level2_player_aim_for_test(yaw, pitch);
                fixed_pose = Some((position, yaw, pitch));
            }
            if fatal.is_none() && before.life >= 0 && ready && fixed_pose.is_some() {
                command = PlayerCommand {
                    opcode: 6,
                    argument_1: fire_ui,
                    flags: 0x40,
                    ..PlayerCommand::default()
                };
            }
            let cost = session.shared_spells().pickup(fire).unwrap().max_mana;
            let event_start = session.economy().events().len();
            area_damage::start_effect_damage_trace(skeleton.source_slot);
            area_damage::start_fireball_launch_trace();
            session.start_model9_callback_trace(skeleton);
            session.step_outer_turn(command, 0);
            let visits = session.take_model9_callback_trace();
            let launches = area_damage::take_fireball_launch_trace()
                .into_iter()
                .filter(|launch| launch.2 == player)
                .collect::<Vec<_>>();
            births.extend(area_damage::take_fireball_impact_birth_trace());
            callbacks.extend(area_damage::take_effect_damage_trace());
            for visit in visits {
                assert_eq!(visit.source, skeleton);
                assert_eq!(
                    visit.exit_handle,
                    Some(skeleton),
                    "the original callback must retain its generation"
                );
                assert!(
                    visit.entry_damage_trace_cursor <= visit.exit_damage_trace_cursor
                        && callback_start + visit.exit_damage_trace_cursor <= callbacks.len()
                );
                if visit.entry.life >= 0 && visit.exit.life < visit.entry.life {
                    let consumed = visit.entry.mailbox;
                    assert_eq!(
                        consumed,
                        (crate::level1_mission::FIREBALL_PAYLOAD, player),
                        "each actual life reduction must consume one player Fireball"
                    );
                    assert_eq!(
                        visit.exit.life,
                        visit.entry.life - crate::level1_mission::FIREBALL_PAYLOAD
                    );
                    assert_eq!(
                        visit.exit.mailbox.1, 0,
                        "the real callback clears the consumed owner"
                    );
                    let entry_cursor = callback_start + visit.entry_damage_trace_cursor;
                    assert!(entry_cursor <= callbacks.len());
                    let writer = callbacks[..entry_cursor]
                        .iter()
                        .rposition(|receipt| receipt.mailbox_before != receipt.mailbox_after)
                        .expect("a real mailbox writer precedes the callback");
                    assert_eq!(
                        callbacks[writer].mailbox_after,
                        Some(consumed),
                        "the latest writer before the callback supplies the consumed mailbox"
                    );
                    damage_visits.push((writer, visit.entry.life, consumed, visit.exit.life));
                    if visit.exit.life < 0 {
                        assert_eq!(visit.exit.killer, player);
                        fatal = Some((writer, visit.entry.life, consumed, visit.exit.life));
                    }
                }
            }
            for receipt in session.cf10_production_schedule_receipts() {
                let ScheduleReceipt::EpilogueCompleted {
                    slot,
                    original,
                    current: CurrentOccupant::Known { selector, .. },
                    ..
                } = receipt
                else {
                    continue;
                };
                let (Some(before), Some(after)) = (
                    original.observed_player_resources(),
                    selector.observed_player_resources(),
                ) else {
                    continue;
                };
                if slot == fire && !launches.is_empty() {
                    assert!(cost > 0 && queued.is_none());
                    let regen = if before.2 >= 0 {
                        -cost
                    } else {
                        before.2.wrapping_sub(cost)
                    };
                    assert_eq!(
                        (before.0, after.0, after.1, after.2),
                        (player_handle, player_handle, before.1, regen)
                    );
                    queued = Some((launches.clone(), regen));
                } else if slot == player
                    && let Some((_, regen)) = queued.as_ref()
                    && before.0 == player_handle
                    && before.2 == *regen
                {
                    assert_eq!(
                        (after.0, after.1),
                        (
                            player_handle,
                            before.1.wrapping_add(before.2).clamp(0, before.3)
                        )
                    );
                    paid.extend(queued.take().unwrap().0);
                }
            }
            assert!(
                session
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
            );
            let same = session.source_arena().handle(skeleton.source_slot) == Some(skeleton);
            if same {
                let row = &session.entities()[usize::from(skeleton.source_slot)];
                if before.action == 76 {
                    assert_eq!(
                        session.economy().authored_bee_kill_census(player),
                        credit_before,
                        "source PreKill excludes model9 from the player kill percentage"
                    );
                    no_skeleton_kill_increment = true;
                }
                if was_fatal {
                    assert!(
                        !session.source_arena().rebuilt_class5_model_slots()[9]
                            .contains(&skeleton.source_slot)
                    );
                    assert!(
                        !session
                            .creature_runtime_census()
                            .roots_of_kind(CreatureKind::Skeleton)
                            .any(|root| root.handle == skeleton && root.living)
                    );
                    following_target_loss = true;
                }
                if fatal.is_some() && row.life < 0 {
                    assert_eq!(row.word_24(), player);
                }
                saw_76 |= row.action == 76;
                saw_77 |= row.action == 77;
                for event in &session.economy().events()[event_start..] {
                    if let Level2EconomyEvent::ManaSphereSpawned {
                        sphere_id,
                        source_entity_index,
                        mana: 500,
                    } = event
                        && *source_entity_index == skeleton.source_slot
                    {
                        let actual = session
                            .economy()
                            .spheres()
                            .iter()
                            .find(|candidate| candidate.id == *sphere_id)
                            .expect("the actual death sphere remains observable on its birth pass");
                        assert_eq!((actual.mana, actual.owner_entity_index), (500, 0));
                        sphere = true;
                    }
                    burst |= matches!(event, Level2EconomyEvent::DeathBurstSpawned { parent_entity_index, .. } if *parent_entity_index == skeleton.source_slot);
                }
                if row.drawing_disabled() {
                    hidden = true;
                    assert!(
                        !session
                            .campaign_entity_presentations(0)
                            .unwrap()
                            .iter()
                            .any(|view| view.entity_index == skeleton.source_slot)
                    );
                }
            } else if hidden {
                reclaimed = true;
                break;
            } else {
                panic!("original Skeleton generation replaced before hide: {trace:?}");
            }
            if turn % 20 == 0 {
                trace.push((
                    turn,
                    before.life,
                    before.action,
                    before.position,
                    mailbox,
                    paid.len(),
                    callbacks.len(),
                ));
            }
        }
        let (fatal_index, before_life, mailbox, after_life) = fatal.expect(&format!(
            "no real fatal hit: pose={fixed_pose:?} trace={trace:?} paid={paid:?}"
        ));
        assert_eq!(mailbox, (crate::level1_mission::FIREBALL_PAYLOAD, player));
        assert_eq!(
            damage_visits
                .iter()
                .map(|row| (row.1, row.3))
                .collect::<Vec<_>>(),
            [(1000, 750), (750, 500), (500, 250), (250, 0), (0, -250)],
            "five actual single-pulse damage visits produce source-negative life"
        );
        for &(index, _, visit_mailbox, _) in &damage_visits {
            let hit = &callbacks[index];
            let impact = hit
                .source_impact
                .map(|(generation, row)| {
                    (
                        row.source_entity_index,
                        generation,
                        row.parent_entity_index,
                        row.parent_entity_id,
                    )
                })
                .or_else(|| {
                    hit.source_raw_impact
                        .as_ref()
                        .and_then(|(generation, row)| {
                            births
                                .iter()
                                .find(|&&(slot, born, _, _, owner)| {
                                    slot == row.index
                                        && born == *generation
                                        && owner == row.source_id
                                })
                                .map(|&(_, _, parent, _, owner)| {
                                    (row.index, *generation, parent, owner)
                                })
                        })
                });
            let (slot, generation, parent, owner) = impact.expect("actual fatal impact source");
            assert!(
                hit.target_slot == skeleton.source_slot
                    && hit.target_generation == skeleton.generation
                    && hit.source_slot == Some(slot)
                    && owner == player
                    && hit.pulse.owner == player
                    && hit.pulse.payload == crate::level1_mission::FIREBALL_PAYLOAD as u16
                    && hit.pulse.selector == (u8::MAX, u8::MAX)
                    && births.iter().any(|&(s, g, p, pg, o)| (s, g, p, o)
                        == (slot, generation, parent, owner)
                        && paid.contains(&(p, pg, o)))
                    && hit.mailbox_after == Some(visit_mailbox)
                    && hit
                        .mailbox_before
                        .zip(hit.mailbox_after)
                        .is_some_and(|(before, after)| {
                            after
                                == (
                                    if before.1 == 0 {
                                        crate::level1_mission::FIREBALL_PAYLOAD
                                    } else {
                                        before
                                            .0
                                            .wrapping_add(crate::level1_mission::FIREBALL_PAYLOAD)
                                    },
                                    player,
                                )
                        })
                    && hit.target.is_some_and(|target| {
                        crate::level1_mission::source_entities_overlap(
                            hit.pulse.position,
                            hit.pulse.half_width,
                            hit.pulse.vertical_offset,
                            hit.pulse.half_height,
                            target.position,
                            target.shape.half_width,
                            target.shape.vertical_offset,
                            target.shape.half_height,
                        )
                    }),
                "each mailbox must originate at a paid, generation-matched real impact: hit={hit:?} paid={paid:?}"
            );
        }
        assert_eq!(damage_visits.last().map(|row| row.0), Some(fatal_index));
        assert!(
            before_life >= 0
                && after_life < 0
                && saw_76
                && saw_77
                && sphere
                && burst
                && hidden
                && reclaimed
                && following_target_loss
                && no_skeleton_kill_increment,
            "source death and following reclaim: 76={saw_76} 77={saw_77} sphere={sphere} burst={burst} hidden={hidden} reclaimed={reclaimed} target_loss={following_target_loss} no_kill_increment={no_skeleton_kill_increment} trace={trace:?}"
        );
        assert!(
            !session.source_arena().rebuilt_class5_model_slots()[9].contains(&skeleton.source_slot)
        );
        assert!(
            !session
                .creature_runtime_census()
                .roots_of_kind(CreatureKind::Skeleton)
                .any(|row| row.handle == skeleton && row.living)
        );
        assert_ne!(
            session.source_arena().handle(skeleton.source_slot),
            Some(skeleton)
        );
    }

    #[test]
    fn ordinary_level_seven_skeleton_arrow_damages_player() {
        use crate::PlayerCommand;
        use crate::campaign::{CampaignSession, spec::CampaignSelection};
        use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent};
        use std::{collections::BTreeMap, path::PathBuf};

        let package = std::env::var_os("MC_LEVEL7_HOUSE_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/settlement-20261105-integration/owner-collision-closure-v4/level7")
            },
            PathBuf::from,
        );
        let mut session = CampaignSession::open(package, CampaignSelection::ordinary(7).unwrap())
            .expect("whole Level7 owner package")
            .simulation()
            .clone();
        let player = session.state.entity_index;
        let player_handle = session.source_arena().handle(player).unwrap();
        for _ in 0..256 {
            if session.session.state.live.player_protection_ticks == 0 {
                break;
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert_eq!(session.session.state.live.player_protection_ticks, 0);
        let trigger = session.entities()[279].clone();
        assert_eq!(
            (trigger.class, trigger.model, trigger.source_id),
            (11, 0, 19)
        );
        session
            .set_level2_entity_position(player, trigger.position)
            .unwrap();
        let mut roots = BTreeMap::new();
        let mut selected = None;
        for _ in 0..32 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            if session.dispatch_events().iter().any(|event| matches!(event,
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 279, group: 19, authored_indices, .. }
                    if authored_indices.contains(&454)))
            {
                for root in session.creature_runtime_census().roots_of_kind(CreatureKind::Skeleton) {
                    if root.living && matches!(root.origin, CreatureRootOrigin::AuthoredRecord {
                        authored_record: 430..=445 | 453..=461
                    }) {
                        roots.insert(root.handle.source_slot, root.handle);
                        if root.origin == (CreatureRootOrigin::AuthoredRecord { authored_record: 454 }) {
                            selected = Some(root.handle);
                        }
                    }
                }
                break;
            }
        }
        let selected = selected.expect("actual switch279 publishes Skeleton454");
        let source = &session.entities()[usize::from(selected.source_slot)];
        let mut position = source.position;
        crate::move_entity(&mut position, source.yaw, 0, 1536);
        position.z = position.z.max(
            session
                .canonical_terrain()
                .ground_height(position)
                .wrapping_add(256),
        );
        session
            .set_level2_entity_position(player, position)
            .unwrap();
        let life_before = session.canonical_player_life_for_test();
        let _ = take_skeleton_arrow_launches();
        let _ = session.take_archer_child_damage_receipts();
        let mut launches = BTreeMap::new();
        let mut accepted = None;
        let mut intercepted = false;
        for turn in 0..240 {
            if turn >= 48
                && !intercepted
                && accepted.is_none()
                && let Some(arrow) = session
                    .economy()
                    .authored_class5_archer_children()
                    .iter()
                    .find(|arrow| {
                        arrow.active
                            && arrow.drawing_enabled
                            && arrow.word_0x96 == player
                            && launches
                                .contains_key(&(arrow.source_entity_index, arrow.slot_generation))
                    })
                    .copied()
            {
                // Player-only interception of an already published arrow.
                let mut position = arrow.position;
                crate::move_entity(&mut position, arrow.yaw, arrow.pitch, arrow.actual_speed);
                session
                    .set_level2_entity_position(player, position)
                    .unwrap();
                intercepted = true;
            }
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                session
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "turn {turn}: {:?}",
                session.dispatch_events()
            );
            for launch in take_skeleton_arrow_launches() {
                if roots.get(&launch.parent.source_slot) == Some(&launch.parent) {
                    assert_eq!(
                        (
                            launch.source_class,
                            launch.model,
                            launch.particle,
                            launch.payload
                        ),
                        (
                            9,
                            13,
                            campaign_shared_families::MC2_SKELETON_ASSETS.arrow_particle,
                            crate::level_definition::SKELETON_CLASS_PROPERTIES.arrow_damage
                        )
                    );
                    launches.insert((launch.child.source_slot, launch.child.generation), launch);
                }
            }
            for receipt in session.take_archer_child_damage_receipts() {
                let key = (receipt.child.source_slot, receipt.child.generation);
                let payload = crate::level_definition::SKELETON_CLASS_PROPERTIES.arrow_damage;
                let expected = if receipt.mailbox_before.1 == 0 {
                    i32::from(payload)
                } else {
                    receipt.mailbox_before.0.wrapping_add(i32::from(payload))
                };
                if launches.get(&key).is_some_and(|launch| {
                    receipt.parent == Some(launch.parent)
                        && launch.target == player
                        && launch.attacker == receipt.attacker
                }) && receipt.target == player_handle
                    && receipt.payload == payload
                    && receipt.mailbox_after == (expected, receipt.attacker)
                {
                    accepted = Some(receipt);
                }
            }
            if accepted.is_some_and(|receipt| session.economy().events().get(receipt.event_cursor..)
                .is_some_and(|events| events.iter().any(|event| matches!(event,
                    Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index, damage, .. }
                        if *target_entity_index == player && *attacker_entity_index == receipt.attacker
                            && *damage == i32::from(crate::level_definition::SKELETON_CLASS_PROPERTIES.arrow_damage)))))
            {
                assert!(session.canonical_player_life_for_test() < life_before);
                return;
            }
        }
        panic!(
            "ordinary Skeleton arrow did not reach player: roots={roots:?} launches={launches:?} accepted={accepted:?} intercepted={intercepted} life={}->{}",
            life_before,
            session.canonical_player_life_for_test()
        );
    }

    #[test]
    fn ordinary_skeleton_impact_attack_and_expiry_keep_source_lifecycle() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let particles = &session.session.definition.presentation.particles;
        let parameters = &session.session.definition.presentation.entity_parameters;
        let parent = session.state.entity_index;
        let enemy = session
            .goats()
            .iter()
            .find(|goat| goat.active && goat.life > 0)
            .unwrap()
            .entity_index;
        let position = arena.entities[usize::from(enemy)].position;
        let slot = arena
            .allocate_authored_skeleton_raw(
                999,
                position,
                1_000,
                None,
                &mut terrain,
                particles,
                &parameters[80],
            )
            .unwrap()
            .unwrap();
        let handle = arena.handle(slot).unwrap();
        let effect =
            crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain)
                .unwrap();
        let row = &mut arena.entities[usize::from(effect)];
        row.source_id = parent;
        put(row, 30, parent);
        row.set_source_byte_46(32);
        row.set_subspell_index(40);
        row.set_source_word_96(slot);
        let impact =
            crate::native_world::alliance_spell::step_impact(&mut arena, effect, &[slot]).unwrap();
        assert_eq!(impact.affected_entities, [slot]);
        let acquired = arena.clone();
        put(&mut arena.entities[usize::from(parent)], 154, enemy);
        let stage = step_alliance_source_slot(
            &mut arena,
            &mut terrain,
            particles,
            parameters,
            slot,
            &[slot],
            Default::default(),
        )
        .unwrap();
        assert_eq!(stage.feedback, [(parent, 24)]);
        assert_eq!(arena.entities[usize::from(slot)].action, 74);
        assert_eq!(arena.entities[usize::from(slot)].source_word_96(), enemy);
        assert_eq!(
            arena.entities[usize::from(slot)].particle_parameter_index,
            Some(202)
        );
        let attack_ready = arena.clone();
        // Select the source scan cadence explicitly. The owned arrow then
        // enters the same real action13 arena used by ordinary Archers.
        arena.entities[usize::from(slot)].set_phase(0);
        step_alliance_source_slot(
            &mut arena,
            &mut terrain,
            particles,
            parameters,
            slot,
            &[slot],
            Default::default(),
        )
        .unwrap();
        let arrow = arena
            .economy
            .authored_class5_archer_children
            .iter()
            .find(|arrow| arrow.parent_entity_index == slot)
            .expect("real Skeleton arrow");
        assert_eq!((arrow.source_class, arrow.model, arrow.action), (9, 13, 13));
        assert_eq!((arrow.particle_index, arrow.subspell_index), (203, 400));
        assert_eq!(
            (arrow.fov, arrow.shift_rot_x, arrow.shift_rot_y),
            (0, 44, 60)
        );
        assert_eq!((arrow.word_0x96, arrow.entity_id), (enemy, slot));
        assert!(arena.handle(arrow.source_entity_index).is_some());

        // Controlled allocator pressure exercises the real replacement stack.
        // The current callback address can become the shot; no old Skeleton
        // packet may overwrite it, and the source exit tail targets the arrow.
        let mut replaced = attack_ready.clone();
        let mut replaced_world = terrain.clone();
        for row in replaced.entities.iter_mut() {
            row.set_control_flags(
                row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
        }
        replaced.economy.exhaust_new_event_capacity_for_test();
        let actor = &mut replaced.entities[usize::from(slot)];
        actor.set_control_flags(
            actor.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
        );
        actor.set_phase(0);
        replaced.rebuild_free_arena(&[]);
        replaced.economy.exhaust_new_event_capacity_for_test();
        step_alliance_source_slot(
            &mut replaced,
            &mut replaced_world,
            particles,
            parameters,
            slot,
            &[slot],
            Default::default(),
        )
        .unwrap();
        assert_ne!(replaced.handle(slot), Some(handle));
        let current = replaced
            .economy
            .authored_class5_archer_children
            .iter()
            .find(|child| child.source_entity_index == slot)
            .expect("callback address is now an arrow");
        assert_eq!(
            (current.source_class, current.model, current.action),
            (9, 13, 13)
        );
        assert_eq!(
            (
                current.particle_index,
                current.subspell_index,
                current.word_0x96,
                current.dword_0x10
            ),
            (201, 400, 0, 50)
        );
        assert_eq!(
            current.position,
            crate::Position {
                z: (particles[195].speed / 2).wrapping_mul(2) as i16,
                ..crate::Position::default()
            }
        );
        assert_eq!((current.xtype, current.xsubtype), (3, u8::MAX));

        let mut reused_target = attack_ready;
        let mut target_world = terrain.clone();
        crate::world::terrain::cave_features::reclaim(&mut reused_target, &mut target_world, enemy);
        reused_target.entities[usize::from(slot)].set_phase(0);
        step_alliance_source_slot(
            &mut reused_target,
            &mut target_world,
            particles,
            parameters,
            slot,
            &[slot],
            Default::default(),
        )
        .unwrap();
        let current = reused_target
            .economy
            .authored_class5_archer_children
            .iter()
            .find(|child| child.source_entity_index == enemy)
            .expect("reclaimed selected address becomes the shot");
        assert_eq!((current.xtype, current.xsubtype), (9, 13));
        assert_eq!(reused_target.handle(slot), Some(handle));

        for parent_case in ["free", "reused", "dead"] {
            let mut controlled = acquired.clone();
            let mut world = terrain.clone();
            if parent_case != "dead" {
                crate::world::terrain::cave_features::reclaim(&mut controlled, &mut world, parent);
                if parent_case == "reused" {
                    assert_eq!(
                        controlled.allocate_source_slot_with_terrain(&mut world),
                        Some(parent)
                    );
                    let mut replacement =
                        crate::world::terrain::cave_features::new_event(parent, 9, position);
                    replacement.class = 3;
                    replacement.record[67] = 3;
                    replacement.set_life(100);
                    controlled.entities[usize::from(parent)] = replacement;
                }
            } else {
                controlled.entities[usize::from(parent)].set_life(0);
            }
            controlled.entities[usize::from(parent)].set_source_word_96(0);
            controlled.entities[usize::from(slot)].set_source_word_96(0);
            step_alliance_source_slot(
                &mut controlled,
                &mut world,
                particles,
                parameters,
                slot,
                &[slot],
                Default::default(),
            )
            .unwrap();
            assert_eq!(
                controlled.entities[usize::from(slot)].record[77],
                if parent_case == "dead" { 10 } else { 14 },
                "{parent_case}"
            );
            assert_eq!(controlled.handle(slot), Some(handle));
        }
        let mut expired = acquired;
        put(&mut expired.entities[usize::from(slot)], 50, 1);
        step_alliance_source_slot(
            &mut expired,
            &mut terrain,
            particles,
            parameters,
            slot,
            &[slot],
            Default::default(),
        )
        .unwrap();
        let row = &expired.entities[usize::from(slot)];
        assert_eq!(
            (row.record[77], word(row, 44), row.source_word_96()),
            (10, 0, 0)
        );
        assert_eq!(row.control_flags & 0x0600_0000, 0);
        assert!(!row.drawing_disabled());
        assert_eq!(expired.handle(slot), Some(handle));
    }
}
