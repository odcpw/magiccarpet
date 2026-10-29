//! Damage, pre-kill propagation and model27's two source projectile routes.

use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::native_world::{EntitySnapshot, SessionTerrain, source_target};
use crate::{FlightWorld, Position, move_entity, turn_toward};

use super::multipart::{self, Chain};
use super::{Allocation, PROJECTILE_DESCRIPTOR, PROPERTIES, SOURCE_MODEL};

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put_word(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_dword(row: &mut EntitySnapshot, offset: usize, value: i32) {
    row.record[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn imported(arena: &SourceArena, handle: SourceEntityHandle) -> Option<EntitySnapshot> {
    match arena.resolve(handle)? {
        SourceEntityView::Imported(row) => Some(row.clone()),
        SourceEntityView::Dynamic(_) => None,
    }
}

/// `sub_2A660`: transfer a head's real damage mailbox to the root, apply the
/// source's 76-point cap to that head, and consume the one-shot mailbox.
fn consume_head_damage(
    arena: &mut SourceArena,
    root_handle: SourceEntityHandle,
    head_handle: SourceEntityHandle,
) -> Result<(), String> {
    if arena.slot_generation(root_handle.source_slot) != root_handle.generation {
        return Err(format!(
            "model27 root {} changed generation",
            root_handle.source_slot
        ));
    }
    if arena.slot_generation(head_handle.source_slot) != head_handle.generation {
        return Err(format!(
            "model27 head {} changed generation",
            head_handle.source_slot
        ));
    }
    let Some(head) = arena.entities.get(usize::from(head_handle.source_slot)) else {
        return Err(format!(
            "model27 head {} is outside the arena",
            head_handle.source_slot
        ));
    };
    if !head.active || head.class != 5 || head.model != SOURCE_MODEL {
        return Err(format!(
            "model27 head {} lost its source row",
            head_handle.source_slot
        ));
    }
    let mut head = head.clone();
    let damage_word = word(&head, 102);
    if damage_word == 0 {
        return Ok(());
    }
    let damage = i32::from_le_bytes(
        head.record[98..102]
            .try_into()
            .expect("fixed entity record contains model27 damage word"),
    );
    let killer = damage_word;
    let root_slot = root_handle.source_slot;
    let root = arena
        .entities
        .get_mut(usize::from(root_slot))
        .filter(|root| root.active && root.class == 5 && root.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model27 root {root_slot} lost its source row"))?;
    put_dword(root, 98, damage);
    put_word(root, 102, killer);
    head.set_life(head.life.wrapping_sub(damage.min(76)));
    head.record[102..104].fill(0);
    put_word(&mut head, 42, killer);
    if head.life < 0 {
        head.record[74] = 6;
    }
    arena.entities[usize::from(head_handle.source_slot)] = head;
    Ok(())
}

/// The root/part generation identity returned to the existing class-9 source
/// constructor by the shared dispatcher. Selector 0 and selector 9 are the
/// real `IfSubtypeCallCreatingManaSphere(&head.position, 9, selector)` routes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectileIntent {
    pub head: SourceEntityHandle,
    pub root: SourceEntityHandle,
    pub target: u16,
    pub selector: u8,
    pub descriptor: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub source_owner: u16,
    pub source_subspell: u16,
    pub target_class: u8,
    pub target_model: u8,
    pub target_relation: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CombatEffects {
    pub projectiles: Vec<ProjectileIntent>,
    pub projectile_slots: Vec<u16>,
    pub sounds: Vec<(u16, u8)>,
}

fn slope_measure(position: Position, terrain: &SessionTerrain) -> i32 {
    let tile_x = (position.x >> 8) as u8;
    let tile_y = (position.y >> 8) as u8;
    let height = |x: u8, y: u8| {
        i32::from(terrain.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let north_west = height(tile_x, tile_y);
    let north_east = height(tile_x.wrapping_add(1), tile_y);
    let south_east = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let south_west = height(tile_x, tile_y.wrapping_add(1));
    (north_west + south_west - north_east - south_east)
        .abs()
        .max((north_west + north_east - south_west - south_east).abs())
}

fn root_candidate_blocked(position: Position, terrain: &SessionTerrain) -> bool {
    terrain.cave_collision_blocked(position) || slope_measure(position, terrain) >= 32
}

/// Source sub_2AF10's forward probe, terrain settle, alternating 91-unit
/// heading search and descriptor-limited yaw correction. Model27 itself is
/// rejected on Cave maps, so water remains admissible under descriptor97's
/// all-material mask; the terrain slope and cave hooks still gate movement.
pub(super) fn step_root_motion(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    root: SourceEntityHandle,
    move_forward: bool,
) -> Result<u8, String> {
    let mut row = imported(arena, root)
        .ok_or_else(|| "model27 movement lost its root generation".to_owned())?;
    let source_slot = root.source_slot;
    let mut flags = row.control_flags;
    if flags & 0x0010_0000 != 0 {
        flags &= !0x0010_0000;
    } else {
        flags &= !1;
    }
    if flags & 0x0000_0800 != 0 {
        flags &= !0x0000_0800;
        row.set_control_flags(flags);
        row.set_source_action(216);
        row.set_source_dword_10(0);
        arena.entities[usize::from(source_slot)] = row;
        return Ok(4);
    }

    let before = row.position;
    let ground = terrain.ground_height(before);
    let mut candidate = before;
    if move_forward {
        move_entity(&mut candidate, row.yaw, 0, row.actual_speed);
    }
    candidate.z = ground;
    let same_tile = (before.x >> 8, before.y >> 8) == (candidate.x >> 8, candidate.y >> 8);
    let mut moved = false;
    let mut turn = false;
    let mut result = 0_u8;
    if move_forward && same_tile {
        moved = true;
        turn = true;
        result = 1;
        flags &= !0x0004_0000;
    } else if !root_candidate_blocked(candidate, terrain) {
        moved = move_forward;
        turn = true;
        result = 2;
        flags &= !0x0004_0000;
    } else {
        flags |= 0x0004_0000;
        if row.yaw == row.source_roll() {
            let mut magnitude = 91_i16;
            let mut direction = 1_i16;
            while magnitude <= 1_024 {
                let heading = row.yaw.wrapping_add(direction * magnitude) & 0x07ff;
                let mut retry = before;
                if move_forward {
                    move_entity(&mut retry, heading, 0, row.actual_speed);
                }
                retry.z = terrain.ground_height(retry);
                if !root_candidate_blocked(retry, terrain) {
                    row.set_source_roll(heading);
                    candidate = retry;
                    moved = move_forward;
                    turn = true;
                    result = 3;
                    break;
                }
                if direction == 1 {
                    direction = -1;
                } else {
                    direction = 1;
                    magnitude = magnitude.saturating_add(91);
                }
            }
            if result == 0 {
                result = 4;
            }
        } else {
            turn = true;
            result = 3;
        }
    }

    row.set_control_flags(flags);
    if moved {
        row.set_position(candidate);
    }
    if turn {
        row.set_yaw(
            row.yaw.wrapping_add(turn_toward(
                row.yaw,
                row.source_roll(),
                PROPERTIES.root_turn_limit,
            )) & 0x07ff,
        );
    }
    let mut position = row.position;
    position.z = terrain.ground_height(position);
    row.set_position(position);
    if result == 4 {
        row.set_source_action(216);
        row.set_source_dword_10(0);
    }
    let changed = arena.entities[usize::from(source_slot)].position != row.position;
    arena.entities[usize::from(source_slot)] = row;
    if changed {
        crate::world::effects::sync_position(arena, terrain, source_slot, position, true, false)?;
    }
    Ok(result)
}

/// Materialize the parameters written after `sub_2A7F0` allocates its model9
/// product. The dispatcher must pass the returned intent to the ordinary
/// source class-9 producer in the same callback pass.
pub(crate) fn projectile_intent(
    arena: &SourceArena,
    targets: source_target::Context<'_>,
    allocation: &Allocation,
    head: SourceEntityHandle,
    target: u16,
    selector: u8,
) -> Result<ProjectileIntent, String> {
    let head_row = imported(arena, head)
        .ok_or_else(|| "model27 projectile lost its head generation".to_owned())?;
    projectile_intent_from_row(
        arena, targets, allocation, head, target, &head_row, selector,
    )
}

fn projectile_intent_from_row(
    arena: &SourceArena,
    targets: source_target::Context<'_>,
    allocation: &Allocation,
    head: SourceEntityHandle,
    target: u16,
    head_row: &EntitySnapshot,
    selector: u8,
) -> Result<ProjectileIntent, String> {
    if !matches!(selector, 0 | 9) {
        return Err(format!(
            "unsupported model27 projectile selector {selector}"
        ));
    }
    let root = imported(arena, allocation.root)
        .ok_or_else(|| "model27 projectile lost its root generation".to_owned())?;
    let target_row = live_target(arena, targets, target)?
        .ok_or_else(|| "model27 projectile lost its current target".to_owned())?;
    if head_row.class != 5 || head_row.model != SOURCE_MODEL || head_row.action != 233 {
        return Err(format!(
            "model27 projectile head {} changed identity",
            head.source_slot
        ));
    }
    let (yaw, pitch) =
        crate::level1_mission::projectile_guidance_angles(head_row.position, target_row.position);
    let position = Position {
        z: head_row
            .position
            .z
            .wrapping_add(head_row.array_fov().wrapping_div(2)),
        ..head_row.position
    };
    Ok(ProjectileIntent {
        head,
        root: allocation.root,
        target,
        selector,
        descriptor: PROJECTILE_DESCRIPTOR,
        position,
        yaw,
        pitch,
        source_owner: root.source_id,
        source_subspell: 850,
        target_class: target_row.class,
        target_model: target_row.model,
        target_relation: word(head_row, 154),
    })
}

fn live_target(
    arena: &SourceArena,
    targets: source_target::Context<'_>,
    slot: u16,
) -> Result<Option<source_target::View>, String> {
    Ok(source_target::read(arena, targets, slot)?
        .filter(|row| row.life >= 0 && !row.drawing_disabled()))
}

pub(super) fn choose_target(
    arena: &SourceArena,
    targets: source_target::Context<'_>,
    origin: Position,
    heading: i16,
    class3: &[u16],
    range: u16,
    cone: u16,
) -> Result<Option<u16>, String> {
    let signed_range = i32::from(range as i16);
    let range_squared = signed_range.wrapping_mul(signed_range);
    let mut best = None;
    // sub_2A6F0 traverses the frozen list without rechecking the current
    // class, life or hidden flag of rows changed earlier in the pass.
    for &slot in class3 {
        let Some(row) = source_target::read(arena, targets, slot)? else {
            continue;
        };
        let dx = i32::from(origin.x.wrapping_sub(row.position.x) as i16);
        let dy = i32::from(origin.y.wrapping_sub(row.position.y) as i16);
        let distance_squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
        if distance_squared >= range_squared || distance_squared >= 0x1000_0000 {
            continue;
        }
        let (yaw, _) = crate::level1_mission::projectile_guidance_angles(origin, row.position);
        let delta = (i32::from(yaw) - i32::from(heading)).rem_euclid(2048);
        let delta = delta.min(2048 - delta);
        if delta < i32::from(cone as i16)
            && best.is_none_or(|(previous, _)| distance_squared < previous)
        {
            best = Some((distance_squared, slot));
        }
    }
    Ok(best.map(|(_, slot)| slot))
}

pub(super) fn step_head_steering(head: &mut EntitySnapshot, terrain: &SessionTerrain) {
    match word(head, 48) {
        0 => {
            let speed_limit = i16::from_le_bytes([head.record[138], head.record[139]]);
            let curvature = word(head, 58) as i16;
            head.set_source_roll(
                head.source_roll()
                    .wrapping_add(curvature)
                    .wrapping_add(speed_limit)
                    .wrapping_add(73),
            );
            head.set_fov(
                head.fov
                    .wrapping_add(curvature)
                    .wrapping_add(speed_limit)
                    .wrapping_add(62),
            );
            if head.actual_speed != 192 {
                let minimum = i16::from_le_bytes([head.record[136], head.record[137]]);
                let next = head.actual_speed.wrapping_add(minimum);
                if next.unsigned_abs() > 192 {
                    head.set_actual_speed(if minimum <= 0 { -192 } else { 192 });
                    head.record[136..138].copy_from_slice(&minimum.wrapping_neg().to_le_bytes());
                } else {
                    head.set_actual_speed(next);
                }
            }
            if head.record[66] & 1 == 0 {
                let curvature = word(head, 58);
                if curvature != 0 {
                    put_word(head, 58, curvature.wrapping_sub(1));
                }
            }
            if head.record[71] == 3 && head.actual_speed == 192 {
                head.record[136..138].copy_from_slice(&(-16_i16).to_le_bytes());
                head.set_actual_speed(head.actual_speed.wrapping_sub(16));
            } else if head.record[71] == 0 {
                let rng = crate::native_world::level2_next_rng16(head.entity_rng);
                head.set_entity_rng(rng);
                head.record[138..140].copy_from_slice(&((rng % 28) as i16).to_le_bytes());
            }
        }
        1 => {
            let target_sign = i16::from_le_bytes([head.record[136], head.record[137]]);
            if head.actual_speed.unsigned_abs() < 192 {
                head.set_actual_speed(head.actual_speed.wrapping_add(target_sign));
            }
            if target_sign <= 0 && head.actual_speed < -192 {
                head.set_actual_speed(-192);
            } else if target_sign > 0 && head.actual_speed > 192 {
                head.set_actual_speed(192);
            }
        }
        2 => {
            let step =
                i16::from_le_bytes([head.record[136], head.record[137]]).unsigned_abs() as i16;
            let speed = head.actual_speed;
            head.set_actual_speed(if speed.unsigned_abs() <= step.unsigned_abs() {
                0
            } else if speed.is_negative() {
                speed.saturating_add(step)
            } else {
                speed.saturating_sub(step)
            });
        }
        3 | 4 => {
            let countdown = head.source_dword_10();
            let speed = match countdown {
                4 => 192,
                3 => -23,
                2 => -130,
                1 => -192,
                _ => head.actual_speed,
            };
            head.set_actual_speed(speed);
        }
        6 => {
            let descent = word(head, 58) as i16;
            let mut position = head.position;
            position.z = position.z.wrapping_sub(descent);
            head.set_actual_speed(
                head.actual_speed
                    .wrapping_sub(i16::from_le_bytes([head.record[136], head.record[137]])),
            );
            head.set_position(position);
            let ground = terrain.ground_height(position);
            if position.z < ground {
                head.set_position(Position {
                    z: ground,
                    ..position
                });
            }
        }
        _ => {}
    }
}

pub(super) fn move_attached_head(head: &mut EntitySnapshot, root: &EntitySnapshot) {
    if head.actual_speed == 0 {
        return;
    }
    let mut position = head.position;
    move_entity(
        &mut position,
        head.source_roll().wrapping_add(root.yaw),
        head.fov.wrapping_add(root.pitch),
        head.actual_speed,
    );
    head.set_position(position);
}

/// Root actions217/218 run before the same-pass 29A90 child tail. Damage copied
/// from head mailboxes by that tail is therefore consumed on the next root
/// callback, matching sub_2A6B0's source order. Idle action217 does not pick a
/// root target: each head runs its own source cone scan in sub_2A6F0.
pub(crate) fn step_root_action(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    targets: source_target::Context<'_>,
    root: SourceEntityHandle,
    callback_action: u8,
    class3: &[u16],
    attack_range: u16,
    attack_cone: u16,
) -> Result<(), String> {
    let root_row = imported(arena, root)
        .ok_or_else(|| "model27 root action lost its generation".to_owned())?;
    if !matches!(callback_action, 217 | 218) || root_row.action != callback_action {
        return Ok(());
    }
    let root_slot = root.source_slot;
    let damaged_by = word(&root_row, 102);
    if damaged_by != 0 {
        let root = &mut arena.entities[usize::from(root_slot)];
        put_word(root, 42, damaged_by);
        put_word(root, 102, 0);
        if root_row.record[63] == 0 {
            root.set_source_action(220);
            put_word(root, 40, damaged_by);
        } else {
            root.set_life(1_000_000);
            root.set_source_action(218);
            put_word(root, 154, damaged_by);
        }
        if callback_action == 218 && root_row.record[63] == 0 {
            return Ok(());
        }
    }

    if callback_action == 217 {
        let movement = step_root_motion(arena, terrain, root, true)?;
        if movement >= 3 {
            return Ok(());
        }
        let row = &mut arena.entities[usize::from(root_slot)];
        if row.record[66] & 0x3f == 0 {
            let rng = crate::native_world::level2_next_rng16(row.entity_rng);
            row.set_entity_rng(rng);
            let desired = (i32::from(rng % 0x01c7) + i32::from(row.source_roll()) - 227)
                .rem_euclid(2048) as i16;
            row.set_source_roll(desired);
        }
        return Ok(());
    }

    let row_after_damage = &arena.entities[usize::from(root_slot)];
    let move_forward = row_after_damage.record[74] == 0;
    let movement = step_root_motion(arena, terrain, root, move_forward)?;
    if movement == 4 {
        return Ok(());
    }

    let target_slot = word(&arena.entities[usize::from(root_slot)], 154);
    let Some(target_row) = live_target(arena, targets, target_slot)? else {
        let root_row = &mut arena.entities[usize::from(root_slot)];
        root_row.set_source_action(217);
        put_word(root_row, 154, 0);
        root_row.record[74] = 0;
        set_root_visual(arena, particles, root_slot, super::ROOT_PARTICLE)?;
        return Ok(());
    };
    let root_position = arena.entities[usize::from(root_slot)].position;
    let source_phase = arena.entities[usize::from(root_slot)].record[66];
    if source_phase & 0x1f == 0
        && multipart::distance_3d(root_position, target_row.position) >= i32::from(attack_range)
    {
        let root_row = &mut arena.entities[usize::from(root_slot)];
        root_row.set_source_action(217);
        put_word(root_row, 154, 0);
        root_row.record[74] = 0;
        set_root_visual(arena, particles, root_slot, super::ROOT_PARTICLE)?;
        return Ok(());
    }

    let desired_heading =
        crate::level1_mission::projectile_guidance_angles(root_position, target_row.position).0;
    if source_phase & 3 == 0
        && movement != 3
        && arena.entities[usize::from(root_slot)].record[74] == 0
    {
        arena.entities[usize::from(root_slot)].set_source_roll(desired_heading);
    }
    if source_phase & 0x1f == 0 {
        let row = &mut arena.entities[usize::from(root_slot)];
        let difference = turn_toward(row.yaw, desired_heading, 1_024).unsigned_abs();
        let aligned = difference <= attack_cone;
        row.record[74] = u8::from(aligned);
        set_root_visual(
            arena,
            particles,
            root_slot,
            if aligned { 337 } else { super::ROOT_PARTICLE },
        )?;
    }
    Ok(())
}

pub(crate) fn set_root_visual(
    arena: &mut SourceArena,
    particles: &[crate::native_world::ParticleParameter],
    root_slot: u16,
    particle: u16,
) -> Result<(), String> {
    let root = arena
        .entities
        .get_mut(usize::from(root_slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model27 root {root_slot} lost its visual row"))?;
    if root.particle_parameter_index != Some(particle) {
        crate::world::effects::bind_particle(root, particle, particles)?;
        root.record[94..96].copy_from_slice(&particle.to_le_bytes());
        root.record[96] = 0;
    }
    Ok(())
}

/// Advance enabled model27 heads from the live source target list. The
/// selectors in the returned intents must be passed to the regular class-9
/// source producer before the callback returns, so newborns join the same
/// pass tail in source allocation order.
pub(crate) fn step_heads(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    targets: source_target::Context<'_>,
    allocation: &Allocation,
    chain: &Chain,
    class3: &[u16],
    list_epoch: u32,
    attack_range: u16,
    attack_cone: u16,
    setting_30: u16,
) -> Result<CombatEffects, String> {
    let root_row = imported(arena, allocation.root)
        .ok_or_else(|| "model27 combat lost its root generation".to_owned())?;
    let root_target = word(&root_row, 154);
    let mut effects = CombatEffects::default();
    // sub_29A90 keeps this temporary across the whole linked-head loop.
    let mut shared_roll = 0x2b_u8;

    for (head_index, head_handle) in chain.heads.iter().copied().enumerate() {
        if arena.slot_generation(head_handle.source_slot) != head_handle.generation {
            return Err(format!(
                "model27 head {} changed generation",
                head_handle.source_slot
            ));
        }
        let head_slot = head_handle.source_slot;
        // sub_29A90 increments every head before selecting its state branch;
        // sub_2A660 is called only for the initial states 0 through 5.
        let initial_state = arena.entities[usize::from(head_slot)].record[74];
        if initial_state <= 5 {
            consume_head_damage(arena, allocation.root, head_handle)?;
        }
        let mut head = arena
            .entities
            .get(usize::from(head_slot))
            .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
            .cloned()
            .ok_or_else(|| format!("model27 head {head_slot} lost its source row"))?;

        head.set_phase(head.record[66].wrapping_add(1));
        let mut state1_roll = None;
        if initial_state <= 5 {
            let rng = crate::native_world::level2_next_rng16(head.entity_rng);
            head.set_entity_rng(rng);
            head.record[71] = (rng % 20) as u8;
            head.set_position(multipart::pose_head(&root_row, head_index));
            if head.record[74] == 1 {
                let rng = crate::native_world::level2_next_rng16(head.entity_rng);
                head.set_entity_rng(rng);
                let target_roll = (rng & 7) as u8;
                shared_roll = target_roll;
                state1_roll = Some(target_roll);
                let attacker = word(&head, 42);
                if attacker != 0 {
                    if target_roll < 4 {
                        put_word(&mut head, 154, attacker);
                        head.record[74] = 2;
                        put_word(&mut head, 42, 0);
                        let growth = word(&head, 58).saturating_add(22).min(68);
                        put_word(&mut head, 58, growth);
                    }
                } else if target_roll < 4 && root_target != 0 && head.record[66] & 7 == 0 {
                    put_word(&mut head, 154, root_target);
                    head.record[74] = 2;
                }
            }
        }

        let mut current_state = head.record[74];
        match current_state {
            0 => {
                put_word(&mut head, 154, 0);
                head.record[74] = 1;
                put_word(&mut head, 48, 0);
                put_word(&mut head, 58, 0);
                put_word(&mut head, 136, 16);
                current_state = 1;
                state1_roll = Some(shared_roll);
            }
            2 => {
                head.record[74] = 3;
                head.record[72] = 0;
                put_word(&mut head, 48, 2);
                put_word(&mut head, 136, 16);
                current_state = 3;
            }
            4 => {
                head.record[74] = 5;
                head.record[72] = 0;
                put_word(&mut head, 48, 2);
                put_word(&mut head, 136, 16);
                current_state = 5;
            }
            6 => {
                head.set_source_dword_10(0);
                put_word(&mut head, 48, 2);
                head.record[74] = 7;
                head.record[72] = 0;
                put_word(&mut head, 136, 80);
                current_state = 7;
            }
            _ => {}
        }

        if current_state == 1 && root_row.record[61] != 0 && head.record[66] & 7 == 0 {
            if let Some(roll) = state1_roll {
                if roll == 0
                    && let Some(target) = choose_target(
                        arena,
                        targets,
                        head.position,
                        head.yaw,
                        if arena.economy.replacement_list_epoch == list_epoch {
                            class3
                        } else {
                            &[]
                        },
                        attack_range,
                        attack_cone,
                    )?
                {
                    put_word(&mut head, 154, target);
                    head.record[74] = 2;
                    current_state = 2;
                } else if roll > 4 {
                    head.record[74] = 4;
                    current_state = 4;
                }
            }
            if current_state == 1 && attack_cone != 0 {
                let roll = state1_roll.unwrap_or(0);
                if roll & 1 == 0 {
                    let rng = crate::native_world::level2_next_rng16(head.entity_rng);
                    head.set_entity_rng(rng);
                    let (yaw_offset, _) = multipart::head_angles(head_index);
                    let turn = i16::try_from(rng % attack_cone).unwrap_or(0);
                    head.set_yaw(
                        root_row
                            .yaw
                            .wrapping_add(yaw_offset)
                            .wrapping_sub(attack_cone as i16)
                            .wrapping_add(turn),
                    );
                }
            }
        }

        let mut pending_fire = None;
        match current_state {
            1 => {}
            3 => {
                let target_slot = word(&head, 154);
                if let Some(target_row) = live_target(arena, targets, target_slot)? {
                    match head.record[72] {
                        0 if head.actual_speed == 0 => {
                            let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
                                head.position,
                                target_row.position,
                            );
                            head.record[72] = 1;
                            put_word(&mut head, 48, 1);
                            put_word(&mut head, 136, 16);
                            head.set_yaw(yaw);
                            head.set_pitch(pitch);
                            head.set_source_roll(yaw.wrapping_sub(root_row.yaw));
                            head.set_fov(pitch.wrapping_sub(root_row.pitch));
                        }
                        1 if head.actual_speed == 192 => {
                            head.record[72] = 3;
                            put_word(&mut head, 48, 3);
                            head.set_source_dword_10(4);
                            pending_fire = Some((target_slot, true));
                        }
                        3 => {
                            pending_fire = Some((target_slot, false));
                            let timer = head.source_dword_10().saturating_sub(1);
                            head.set_source_dword_10(timer);
                            if timer == 0 {
                                head.record[74] = 0;
                                head.set_source_dword_10(1);
                            }
                        }
                        _ => {}
                    }
                } else {
                    head.record[74] = 0;
                    put_word(&mut head, 154, 0);
                }
            }
            5 => match head.record[72] {
                0 if head.actual_speed == 0 => {
                    head.record[72] = 1;
                    put_word(&mut head, 48, 1);
                    put_word(&mut head, 136, (-16_i16) as u16);
                    let (yaw_offset, pitch_offset) = multipart::head_angles(head_index);
                    head.set_source_roll(yaw_offset);
                    head.set_fov(pitch_offset);
                    head.set_yaw(root_row.yaw.wrapping_add(yaw_offset));
                    head.set_pitch(root_row.pitch.wrapping_add(pitch_offset));
                }
                1 if head.actual_speed == -192 => {
                    head.record[72] = 2;
                    head.set_source_dword_10(2);
                }
                2 => {
                    let timer = head.source_dword_10().saturating_sub(1);
                    head.set_source_dword_10(timer);
                    if timer == 0 {
                        put_word(&mut head, 48, 4);
                        head.record[72] = 6;
                        head.set_source_dword_10(1);
                    }
                }
                5 => {
                    let timer = head.source_dword_10().saturating_sub(1);
                    head.set_source_dword_10(timer);
                    if timer <= 4 {
                        head.record[74] = 0;
                        head.set_source_dword_10(4);
                    }
                }
                6 => {
                    effects.sounds.push((allocation.root.source_slot, 17));
                    let timer = head.source_dword_10().wrapping_add(1);
                    head.set_source_dword_10(timer);
                    if timer >= 4 {
                        head.record[72] = 5;
                    }
                }
                _ => {}
            },
            7 => {
                head.set_position(multipart::pose_head(&root_row, head_index));
                let mut wobble_yaw = false;
                match head.record[72] {
                    0 if head.actual_speed == 0 => {
                        head.record[72] = 1;
                        put_word(&mut head, 48, 1);
                        let (yaw_offset, pitch_offset) = multipart::head_angles(head_index);
                        head.set_source_roll(yaw_offset);
                        head.set_fov(pitch_offset);
                    }
                    1 => {
                        wobble_yaw = true;
                        if head.actual_speed == 192 {
                            head.record[72] = 7;
                            put_word(&mut head, 48, 5);
                            head.set_source_dword_10(8);
                        }
                    }
                    7 => {
                        wobble_yaw = true;
                        let timer = head.source_dword_10().saturating_sub(1);
                        head.set_source_dword_10(timer);
                        if timer == 0 {
                            head.record[72] = 8;
                            put_word(&mut head, 48, 6);
                            head.set_fov(0);
                            head.record[71] = 0;
                            put_word(&mut head, 58, 0);
                            put_word(&mut head, 136, 12);
                            head.set_source_dword_10(0);
                        }
                    }
                    8 => {
                        let timer = head.source_dword_10();
                        if timer > 10 {
                            head.record[74] = 8;
                        } else {
                            let segment = if timer == 0 {
                                None
                            } else {
                                Some((9_usize.saturating_sub(timer as usize)).min(8))
                            };
                            let handle = segment
                                .map(|segment| chain.bodies[head_index][segment])
                                .unwrap_or(chain.heads[head_index]);
                            set_part_visibility(arena, handle, false)?;
                            head.set_source_dword_10(timer.wrapping_add(1));
                            head.record[71] = head.record[71].wrapping_add(1);
                            let curve =
                                word(&head, 58).wrapping_add(u16::from(head.record[71]) * 28);
                            put_word(&mut head, 58, curve);
                        }
                    }
                    _ => {}
                }
                if wobble_yaw {
                    let (yaw_offset, _) = multipart::head_angles(head_index);
                    let wobble = if head.record[66] & 1 != 0 { -204 } else { 204 };
                    head.set_yaw(root_row.yaw.wrapping_add(yaw_offset).wrapping_add(wobble));
                }
                // The state-7 branch runs these once before the final-state
                // switch; that switch only poses bodies for states 6 and 7.
                step_head_steering(&mut head, terrain);
                move_attached_head(&mut head, &root_row);
            }
            8 => {
                let root = &mut arena.entities[usize::from(allocation.root.source_slot)];
                root.byte_3b = root.byte_3b.wrapping_sub(1);
                root.record[63] = root.byte_3b;
                head.record[74] = 9;
                // Source case 8 falls through to case 9 in this visit.
                head.set_source_dword_10(99);
            }
            9 => {
                let timer = head.source_dword_10().wrapping_sub(1);
                head.set_source_dword_10(timer);
                if timer == 0 {
                    head.record[74] = 10;
                }
            }
            10 => {
                let root = &mut arena.entities[usize::from(allocation.root.source_slot)];
                root.byte_3b = root.byte_3b.wrapping_add(1);
                root.record[63] = root.byte_3b;
                head.record[74] = 11;
                head.record[72] = 5;
                // Source case 10 falls through to case 11 in this visit.
                head.set_source_dword_10(6);
                head.set_control_flags((head.control_flags & !0x09) | 0x08);
                put_word(&mut head, 154, chain.bodies[head_index][0].source_slot);
                head.set_actual_speed(156);
                let (yaw_offset, pitch_offset) = multipart::head_angles(head_index);
                head.set_source_roll(yaw_offset);
                head.set_fov(pitch_offset);
            }
            11 => {
                let timer = head.source_dword_10().wrapping_sub(1);
                head.set_source_dword_10(timer);
                if timer <= 0 {
                    head.record[74] = 12;
                    head.set_source_dword_10(0);
                }
            }
            12 => {
                let timer = head.source_dword_10();
                if timer < PROPERTIES.body_parts_per_head as i32 {
                    let body = chain.bodies[head_index][timer.max(0) as usize];
                    set_part_visibility(arena, body, true)?;
                    let next = word(&arena.entities[usize::from(body.source_slot)], 56);
                    put_word(&mut head, 154, next);
                    head.set_source_dword_10(timer.wrapping_add(1));
                    if head.source_dword_10() >= PROPERTIES.body_parts_per_head as i32 {
                        let rng = crate::native_world::level2_next_rng16(head.entity_rng);
                        head.set_entity_rng(rng);
                        head.set_life(920 + i32::from(rng % 0x398));
                        head.record[102..104].fill(0);
                        head.record[74] = 0;
                    }
                }
            }
            13 => {
                head.record[74] = 14;
                head.record[71] = 10;
                head.set_source_dword_10(10);
                head.set_source_dword_10(head.source_dword_10().saturating_sub(1));
            }
            14 => {
                let timer = head.source_dword_10().saturating_sub(1);
                head.set_source_dword_10(timer);
                if timer == 0 {
                    head.record[74] = 15;
                }
            }
            15 => {
                head.record[74] = 8;
                set_part_visibility(arena, chain.heads[head_index], false)?;
                for body in chain.bodies[head_index] {
                    set_part_visibility(arena, body, false)?;
                }
            }
            _ => {}
        }

        let final_state = head.record[74];
        if matches!(final_state, 11..=12) {
            head.set_position(multipart::pose_head(&root_row, head_index));
        }
        if matches!(final_state, 0..=5) {
            step_head_steering(&mut head, terrain);
        }
        if matches!(final_state, 0..=5 | 11..=12) {
            move_attached_head(&mut head, &root_row);
        }
        let position = head.position;
        arena.entities[usize::from(head_slot)] = head;
        crate::world::effects::sync_position(arena, terrain, head_slot, position, true, false)?;
        multipart::pose_bodies(arena, terrain, chain, Some(head_index))?;
        if matches!(final_state, 11..=12) {
            // Regrowth copies the current linked target after posing bodies.
            let target_slot = word(&arena.entities[usize::from(head_slot)], 154);
            let position = arena
                .entities
                .get(usize::from(target_slot))
                .ok_or_else(|| {
                    format!("model27 regrowth target {target_slot} is outside the source arena")
                })?
                .position;
            arena.entities[usize::from(head_slot)].set_position(position);
            crate::world::effects::sync_position(arena, terrain, head_slot, position, true, false)?;
        }
        if let Some((target, choose_variant)) = pending_fire {
            let mut head = arena.entities[usize::from(head_slot)].clone();
            let before = effects.projectiles.len();
            fire_head(
                arena,
                targets,
                allocation,
                head_handle,
                target,
                &mut head,
                choose_variant,
                setting_30,
                &mut effects,
            )?;
            arena.entities[usize::from(head_slot)] = head;
            if effects.projectiles.len() > before {
                let shot = effects.projectiles.pop().expect("a shot was just emitted");
                if let Some(slot) =
                    super::mc2::launch_projectile(arena, terrain, particles, targets, shot)?
                {
                    effects.projectile_slots.push(slot);
                    let sound_owner = if slot == head_slot {
                        slot
                    } else {
                        arena.entities[usize::from(head_slot)].source_id
                    };
                    effects
                        .sounds
                        .push((sound_owner, if shot.selector == 0 { 15 } else { 23 }));
                }
            }
            effects.sounds.push((allocation.root.source_slot, 17));
        }
        if matches!(final_state, 0..=5 | 11..=12)
            && arena.slot_generation(head_slot) == head_handle.generation
        {
            let position = multipart::head_tail_position(
                &root_row,
                head_index,
                arena.entities[usize::from(head_slot)].position,
            );
            arena.entities[usize::from(head_slot)].set_position(position);
            crate::world::effects::sync_position(arena, terrain, head_slot, position, true, false)?;
        }
    }
    Ok(effects)
}

fn set_part_visibility(
    arena: &mut SourceArena,
    handle: SourceEntityHandle,
    visible: bool,
) -> Result<(), String> {
    if arena.slot_generation(handle.source_slot) != handle.generation {
        return Err(format!(
            "model27 part {} changed generation",
            handle.source_slot
        ));
    }
    let row = arena
        .entities
        .get_mut(usize::from(handle.source_slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("model27 part {} lost its source row", handle.source_slot))?;
    row.set_control_flags(if visible {
        row.control_flags & !1
    } else {
        (row.control_flags | 1) & !8
    });
    Ok(())
}

fn fire_head(
    arena: &SourceArena,
    targets: source_target::Context<'_>,
    allocation: &Allocation,
    head_handle: SourceEntityHandle,
    target_slot: u16,
    head: &mut EntitySnapshot,
    choose_variant: bool,
    setting_30: u16,
    effects: &mut CombatEffects,
) -> Result<(), String> {
    if choose_variant {
        let rng = crate::native_world::level2_next_rng16(head.entity_rng);
        head.set_entity_rng(rng.wrapping_add(setting_30));
        head.mana_regen = if rng % 12 > 7 { 2 } else { 1 };
        head.record[140..144].copy_from_slice(&head.mana_regen.to_le_bytes());
    }
    let selector = match head.mana_regen {
        1 if choose_variant => Some(0),
        1 => None,
        2 => Some(9),
        _ => None,
    };
    if let Some(selector) = selector {
        effects.projectiles.push(projectile_intent_from_row(
            arena,
            targets,
            allocation,
            head_handle,
            target_slot,
            head,
            selector,
        )?);
    }
    Ok(())
}

/// `sub_298B0` followed by `PreKillEntity_1C890(root, 216)`: the root and all
/// linked parts change to action221. Kill products and drawing shutdown happen
/// in the later action221 callback.
pub(crate) fn prekill(
    arena: &mut SourceArena,
    root: SourceEntityHandle,
) -> Result<Option<u16>, String> {
    let chain = multipart::read_chain(arena, root)?;
    let root_row = imported(arena, root)
        .ok_or_else(|| format!("model27 root {} changed generation", root.source_slot))?;
    if root_row.action != 220 {
        return Err(format!(
            "model27 pre-kill expected action220, found {}",
            root_row.action
        ));
    }
    arena.entities[usize::from(root.source_slot)].set_life(-1);
    let credit =
        crate::native_world::creatures::raw_lifecycle::prekill(arena, root.source_slot, 216)?;
    let post = imported(arena, root)
        .ok_or_else(|| "model27 pre-kill lost its root generation".to_owned())?;
    if post.action != 221
        || chain
            .ordered
            .iter()
            .any(|part| arena.entities[usize::from(part.source_slot)].action != 221)
    {
        return Err("model27 pre-kill did not propagate action221 through all parts".to_owned());
    }
    Ok(credit)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DeathOutcome {
    pub mana_sphere_ids: Vec<u32>,
    pub credited_killer: Option<u16>,
    pub secondary_source_effect_slot: Option<u16>,
}

/// `sub_298D0`: transform the dying root to a mana sphere using source's
/// fraction flag (including its one-sphere/full-remainder shadowing behavior),
/// request the optional source 10/1 product, then hide the complete chain.
pub(crate) fn kill(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    root: SourceEntityHandle,
) -> Result<DeathOutcome, String> {
    let bindings = std::collections::BTreeMap::new();
    let family_root = if let Some(parent) = arena.model27_part_root(root) {
        parent
    } else {
        arena
            .imported_creature_runtime_identity(root.source_slot, &bindings)
            .filter(|identity| {
                identity.handle == root
                    && identity.kind == crate::level_definition::CreatureKind::Creature27
            })
            .map(|identity| identity.handle)
            .ok_or_else(|| {
                format!(
                    "model27 death slot {} lost its generation-bound family",
                    root.source_slot
                )
            })?
    };
    let mut suffix = vec![root];
    let mut next = word(&arena.entities[usize::from(root.source_slot)], 56);
    while next != 0 {
        let Some(part) = arena
            .handle(next)
            .filter(|part| arena.model27_part_root(*part) == Some(family_root))
        else {
            break;
        };
        if suffix.contains(&part) {
            return Err(format!("model27 death chain cycles at {next}"));
        }
        suffix.push(part);
        next = word(&arena.entities[usize::from(next)], 56);
    }
    let row = imported(arena, root)
        .ok_or_else(|| format!("model27 root {} changed generation", root.source_slot))?;
    if row.action != 221 {
        return Err(format!(
            "model27 death expected action221, found {}",
            row.action
        ));
    }
    arena.entities[usize::from(root.source_slot)].set_life(-1);
    let mut sphere_ids = Vec::new();
    if row.mana > 0 {
        // The pinned 36BA0 shadowed fraction leaves the outer count at one.
        // Allocate through NewEvent's shared free/replacement path, then
        // publish the real 10/39 carrier and its map links before continuing.
        let next_rng = crate::native_world::level2_next_rng16(row.entity_rng);
        arena.entities[usize::from(root.source_slot)].set_entity_rng(next_rng);
        arena.economy.source_rng.insert(root.source_slot, next_rng);
        if let Some(slot) = crate::world::effects::spawn_death_mana_sphere(
            arena,
            terrain,
            root.source_slot,
            row.position,
            row.yaw,
            row.mana,
            row.source_player_entity_index(),
        )? {
            sphere_ids.push(u32::from(slot));
        }
        if !crate::world::effects::clear_replacement_mana_owner(arena, root.source_slot)? {
            arena.entities[usize::from(root.source_slot)].set_source_player_entity_index(0);
        }
        arena.economy.source_owner.insert(root.source_slot, 0);
    }
    let killer = word(&row, 40);
    let credited_killer = arena
        .entities
        .get(usize::from(killer))
        .filter(|target| killer != 0 && target.active && target.class == 3)
        .map(|target| target.index);
    let (position, source_id, flags) = if let Some(sphere) = arena
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == root.source_slot)
    {
        (
            sphere.position,
            sphere.source_entity_index,
            sphere.control_flags,
        )
    } else {
        let current = &arena.entities[usize::from(root.source_slot)];
        (current.position, current.source_id, current.control_flags)
    };
    let secondary_source_effect_slot = if flags & 0x0010_0000 == 0 {
        crate::world::effects::spawn_death_burst(
            arena,
            terrain,
            particles,
            root.source_slot,
            position,
            source_id,
        )?
    } else {
        None
    };
    let replacement_hidden = crate::world::effects::hide_birth_replacement(
        arena,
        root.source_slot,
        secondary_source_effect_slot,
    )?;
    if !replacement_hidden && arena.slot_generation(root.source_slot) == root.generation {
        for part in suffix {
            if arena.slot_generation(part.source_slot) == part.generation {
                arena.entities[usize::from(part.source_slot)].disable_drawing();
            }
        }
    } else if !replacement_hidden {
        return Err(format!(
            "model27 death slot {} has an unsupported replacement",
            root.source_slot
        ));
    }
    Ok(DeathOutcome {
        mana_sphere_ids: sphere_ids,
        credited_killer,
        secondary_source_effect_slot,
    })
}
