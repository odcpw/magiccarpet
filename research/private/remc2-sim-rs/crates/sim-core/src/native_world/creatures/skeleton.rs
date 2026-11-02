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
        bind_particle(row, 202, particles)?;
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
        .get(203)
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
    child.particle_index = 203;
    child.subspell_index = if !replaced_root && row.source_player_entity_index() != 0 {
        600
    } else {
        400
    };
    // Particle array.fov is a collision axis, not the entity's angular fov.
    child.fov = 0;
    child.shift_rot_x = (particle.speed / 2).wrapping_mul(2) as i16;
    child.shift_rot_y = (particle.rotation_speed / 2).wrapping_mul(2) as i16;
    child.collision_half_width = (particle.speed / 2).wrapping_mul(2) as i16;
    child.collision_vertical_offset = (particle.rotation_speed / 2) as i16;
    child.collision_half_height = (particle.rotation_speed / 2).wrapping_mul(2) as i16;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let player_feedback =
        crate::native_world::Level2EconomyState::apply_creature_fireball_target_feedback(
            &mut arena.entities,
            target.slot,
        );
    if replaced_root {
        // sub_20C50 rereads this address after sub_1CDA0. The replacement
        // is no longer action74, so sub_20F20 applies to the current arrow.
        let particle = particles
            .get(201)
            .ok_or("Skeleton exit needs particle201")?;
        let child = arena
            .economy
            .authored_class5_archer_children
            .iter_mut()
            .find(|child| child.source_entity_index == slot)
            .ok_or("replacement arrow lost its carrier")?;
        child.actual_speed = child.minimum_speed;
        child.particle_index = 201;
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
        .get(usize::from(row.parameter_relation.unwrap_or(80)))
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
            bind_particle(&mut row, 201, particles)?;
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
        .get(220)
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
