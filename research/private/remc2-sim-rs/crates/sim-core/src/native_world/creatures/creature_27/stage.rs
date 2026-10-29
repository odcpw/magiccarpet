//! Model27's authored StageVar action over its authoritative source row.

use crate::campaign::source_arena::{SourceArena, SourceEntityHandle, SourceEntityView};
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{
    EntitySnapshot, ParticleParameter, RuntimeSnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{Position, angle_distance};

use super::{PROPERTIES, SOURCE_MODEL, combat};

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn source_target(arena: &SourceArena, slot: u16) -> Option<EntitySnapshot> {
    if slot == 0 {
        return None;
    }
    match arena.resolve(arena.handle(slot)?)? {
        SourceEntityView::Imported(row) => Some(row.clone()),
        SourceEntityView::Dynamic(_) => None,
    }
}

fn jitter(row: &mut EntitySnapshot) {
    let first = level2_next_rng16(row.entity_rng);
    let second = level2_next_rng16(first);
    row.set_entity_rng(second);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    row.set_source_roll(
        (i32::from(row.source_roll()) + (i32::from(second & 255) + 85) * direction) as i16 & 2047,
    );
}

fn follow(
    arena: &mut SourceArena,
    root: SourceEntityHandle,
    point: Position,
    target_width: Option<i16>,
    peers: &[u16],
) {
    let snapshot = arena.entities[usize::from(root.source_slot)].clone();
    if snapshot.record[66] & 7 != 0 {
        return;
    }
    let mut row = snapshot.clone();
    if row.control_flags & 0x0004_0000 == 0 {
        row.set_source_roll(
            crate::level1_mission::projectile_guidance_angles(row.position, point).0,
        );
        if row.record[66] & 63 == 0 {
            jitter(&mut row);
        }
    }
    for &slot in peers {
        let Some(peer) = source_target(arena, slot) else {
            continue;
        };
        if peer.source_id != row.source_id
            && (i32::from(row.position.x as i16) - i32::from(peer.position.x as i16)).abs()
                < i32::from(row.array_pitch())
            && (i32::from(row.position.y as i16) - i32::from(peer.position.y as i16)).abs()
                < i32::from(row.array_pitch())
        {
            row.set_source_roll(
                crate::level1_mission::projectile_guidance_angles(peer.position, row.position).0,
            );
            break;
        }
    }
    if let Some(width) = target_width
        && (i32::from(row.position.x) - i32::from(point.x)).abs()
            < i32::from(row.array_pitch()) + i32::from(width)
        && (i32::from(row.position.y) - i32::from(point.y)).abs()
            < i32::from(row.array_pitch()) + i32::from(width)
    {
        row.set_source_roll(
            crate::level1_mission::projectile_guidance_angles(point, row.position).0,
        );
    }
    arena.entities[usize::from(root.source_slot)] = row;
}

fn stage_actor(
    arena: &mut SourceArena,
    root: SourceEntityHandle,
    descriptor: crate::native_world::creatures::archer::ArcherStageDescriptor,
    families: &[Vec<u16>; 29],
) -> Option<EntitySnapshot> {
    if descriptor.flags & 2 == 0 {
        return source_target(arena, descriptor.actor);
    }
    let slot = usize::from(root.source_slot);
    let mut relation = word(&arena.entities[slot], 78);
    if relation == 0 && descriptor.flags & 1 != 0 {
        let stage_index = arena.entities[slot].record[76];
        relation = families[usize::from(SOURCE_MODEL)]
            .iter()
            .filter_map(|&peer| source_target(arena, peer))
            .find(|peer| peer.record[76] == stage_index && word(peer, 78) != 0)
            .map_or(0, |peer| word(&peer, 78));
    }
    if relation == 0 {
        let origin = arena.entities[slot].position;
        relation = families
            .get(usize::from(descriptor.model))
            .into_iter()
            .flatten()
            .filter_map(|&candidate| source_target(arena, candidate))
            .min_by_key(|target| {
                let dx = i32::from(target.position.x.wrapping_sub(origin.x) as i16);
                let dy = i32::from(target.position.y.wrapping_sub(origin.y) as i16);
                dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
            })
            .map_or(0, |target| target.index);
    }
    put(&mut arena.entities[slot], 78, relation);
    source_target(arena, relation)
}

fn follow_summon(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    root: SourceEntityHandle,
    target: u16,
) -> Result<(), String> {
    let slot = usize::from(root.source_slot);
    let mut row = arena.entities[slot].clone();
    let attacker = word(&row, 102);
    if attacker != 0 {
        let damage = i32::from_le_bytes(
            row.record[98..102]
                .try_into()
                .expect("source mailbox extent"),
        );
        row.set_life(row.life.wrapping_sub(damage));
        put(&mut row, 102, 0);
        put(&mut row, 42, attacker);
    } else {
        put(&mut row, 42, 0);
    }
    if row.life < 0 {
        put(&mut row, 40, attacker);
        put(&mut row, 50, 1);
        arena.entities[slot] = row;
        return Ok(());
    }
    arena.entities[slot] = row;
    combat::step_root_motion(arena, terrain, root, true)?;
    if attacker != 0 {
        let parent = word(&arena.entities[slot], 44);
        if attacker != parent
            && source_target(arena, attacker)
                .is_none_or(|attacker| attacker.class != 5 || attacker.model != SOURCE_MODEL)
        {
            let row = &mut arena.entities[slot];
            put(row, 154, attacker);
            row.set_source_action(218);
        }
    } else if arena.entities[slot].record[66] & 7 == 0
        && let Some(target) = source_target(arena, target)
        && arena.entities[slot].control_flags & 0x0004_0000 == 0
    {
        let row = &mut arena.entities[slot];
        row.set_source_roll(
            crate::level1_mission::projectile_guidance_angles(row.position, target.position).0,
        );
        if row.record[66] & 63 == 0 {
            jitter(row);
        }
    }
    Ok(())
}

fn summoned(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    root: SourceEntityHandle,
    class3: &[u16],
    mode: u8,
) -> Result<(), String> {
    let slot = usize::from(root.source_slot);
    let parent = arena
        .creature_parent(root)
        .and_then(|parent| source_target(arena, parent.source_slot))
        .filter(|parent| parent.life >= 0 && !parent.drawing_disabled());
    let remaining = word(&arena.entities[slot], 50) as i16;
    let remaining = if parent.is_none() {
        0
    } else if mode == 13 {
        remaining.wrapping_sub(1)
    } else {
        remaining
    };
    put(&mut arena.entities[slot], 50, remaining as u16);
    if remaining <= 0 {
        return super::super::creature_24::mc2::expire(
            arena,
            terrain,
            particles,
            root.source_slot,
            mode == 13,
        );
    }
    let parent = parent.expect("live summon parent was checked");
    let mut target = word(&arena.entities[slot], 154);
    if source_target(arena, target).is_some_and(|row| row.life <= 0 || row.drawing_disabled()) {
        target = 0;
        put(&mut arena.entities[slot], 154, 0);
    }
    if target == 0 && arena.entities[slot].record[66] & 7 == 0 {
        target = class3
            .iter()
            .filter_map(|&slot| source_target(arena, slot))
            .filter(|row| {
                matches!(row.model, 0 | 1)
                    && row.source_id != parent.source_id
                    && row.life > 0
                    && !row.drawing_disabled()
            })
            .min_by_key(|row| {
                let dx = i32::from(parent.position.x.wrapping_sub(row.position.x) as i16);
                let dy = i32::from(parent.position.y.wrapping_sub(row.position.y) as i16);
                dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
            })
            .map_or(0, |row| row.index);
        put(&mut arena.entities[slot], 154, target);
    }
    if target == 0 {
        put(&mut arena.entities[slot], 154, parent.index);
        follow_summon(arena, terrain, root, parent.index)?;
        let row = &mut arena.entities[slot];
        put(row, 154, 0);
        let remaining = word(row, 50).wrapping_sub(4);
        put(row, 50, remaining);
    } else {
        follow_summon(arena, terrain, root, target)?;
        if arena.entities[slot].record[66] & 7 == 0
            && let Some(target) = source_target(arena, target)
            && super::multipart::distance_3d(arena.entities[slot].position, target.position)
                < i32::from(PROPERTIES.root_target_range)
        {
            arena.entities[slot].set_source_action(218);
        }
    }
    Ok(())
}

fn alliance_blink(
    arena: &mut SourceArena,
    root: SourceEntityHandle,
    selection: u8,
) -> Result<(), String> {
    let chain = super::multipart::read_chain(arena, root)?;
    let bits = match selection {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    for part in std::iter::once(root).chain(chain.ordered) {
        if arena.slot_generation(part.source_slot) == part.generation {
            let row = &mut arena.entities[usize::from(part.source_slot)];
            row.set_control_flags((row.control_flags & !0x0600_0000) | bits);
        }
    }
    Ok(())
}

fn alliance(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    root: SourceEntityHandle,
) -> Result<(), String> {
    let slot = usize::from(root.source_slot);
    let remaining = (word(&arena.entities[slot], 50) as i16).wrapping_sub(1);
    put(&mut arena.entities[slot], 50, remaining as u16);
    let parent = arena
        .creature_parent(root)
        .and_then(|parent| source_target(arena, parent.source_slot))
        .filter(|parent| parent.life > 0);
    if remaining <= 0 || parent.is_none() {
        alliance_blink(arena, root, 0)?;
        let row = &mut arena.entities[slot];
        row.record[77] = 10;
        put(row, 154, 0);
        put(row, 44, 0);
        let previous_stage = word(row, 78) as u8;
        row.record[76] = previous_stage;
        return Ok(());
    }
    let row = &arena.entities[slot];
    let duration = word(row, 52) as i16;
    let phase = row.record[66];
    let blink = if remaining <= duration.wrapping_sub(16) && remaining >= 16 {
        if phase & 0x1c == 16 { phase & 3 } else { 0 }
    } else if phase & 1 == 0 {
        2
    } else {
        0
    };
    alliance_blink(arena, root, blink)?;
    let parent = parent.expect("live Alliance parent was checked");
    let previous = word(&arena.entities[slot], 154);
    put(&mut arena.entities[slot], 154, parent.index);
    follow_summon(arena, terrain, root, parent.index)?;
    let inherited = if word(&parent, 154) != 0 {
        word(&parent, 154)
    } else {
        word(&parent, 42)
    };
    let target = if inherited != 0 { inherited } else { previous };
    put(&mut arena.entities[slot], 154, target);
    if let Some(target) = source_target(arena, target) {
        if (word(&target, 44) == parent.index && target.record[77] == 14)
            || target.life <= 0
            || target.drawing_disabled()
        {
            put(&mut arena.entities[slot], 154, 0);
        } else if super::multipart::distance_3d(arena.entities[slot].position, target.position)
            < i32::from(PROPERTIES.root_target_range)
        {
            arena.entities[slot].set_source_action(218);
        }
    }
    Ok(())
}

/// sub_1DDA0 / sub_1DBF0 / sub_1E1C0. Their common mailbox path excludes
/// model27's linked segments, exactly as the source does.
pub(super) fn step(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    root: SourceEntityHandle,
    class3: &[u16],
    families: &[Vec<u16>; 29],
    mode: u8,
) -> Result<(), String> {
    if matches!(mode, 13 | 16) {
        return summoned(arena, terrain, particles, root, class3, mode);
    }
    if mode == 14 {
        return alliance(arena, terrain, root);
    }
    if mode == 12 {
        let slot = usize::from(root.source_slot);
        let parent = arena
            .creature_parent(root)
            .and_then(|parent| source_target(arena, parent.source_slot));
        let row = &mut arena.entities[slot];
        if parent.as_ref().is_none_or(|parent| parent.life < 0) {
            row.disable_drawing();
        }
        let mut position = parent
            .as_ref()
            .map_or(Position::default(), |parent| parent.position);
        position.z = position.z.max(0);
        row.set_position(position);
        row.set_yaw(parent.as_ref().map_or(0, |parent| parent.yaw));
        row.set_source_roll(parent.as_ref().map_or(0, |parent| parent.source_roll()));
        crate::world::effects::sync_position(
            arena,
            terrain,
            root.source_slot,
            position,
            true,
            false,
        )?;
        return Ok(());
    }
    if mode == 17 {
        let slot = usize::from(root.source_slot);
        combat::step_root_motion(arena, terrain, root, true)?;
        if arena.entities[slot].life < 0 {
            arena.entities[slot].disable_drawing();
            return Ok(());
        }
        let target = source_target(arena, word(&arena.entities[slot], 154))
            .filter(|target| target.life >= 0 && !target.drawing_disabled());
        if let Some(target) = target {
            let row = &mut arena.entities[slot];
            row.set_source_roll(
                crate::level1_mission::projectile_guidance_angles(row.position, target.position).0,
            );
            row.set_actual_speed(row.actual_speed.wrapping_sub(8));
            if row.actual_speed <= 16 {
                row.record[77] = 16;
            }
        } else {
            arena.entities[slot].record[77] = 16;
        }
        return Ok(());
    }
    if !matches!(mode, 1..=9) {
        return Err(format!("model27 StageVar mode {mode} is not implemented"));
    }
    let slot = usize::from(root.source_slot);
    let mut row = arena.entities[slot].clone();
    let attacker = word(&row, 102);
    if attacker != 0 {
        let damage = i32::from_le_bytes(
            row.record[98..102]
                .try_into()
                .expect("source mailbox extent"),
        );
        row.set_life(row.life.wrapping_sub(damage));
        put(&mut row, 102, 0);
        put(&mut row, 42, attacker);
    } else {
        put(&mut row, 42, 0);
    }
    if row.life < 0 {
        put(&mut row, 40, attacker);
        row.set_source_action(220);
        arena.entities[slot] = row;
        return Ok(());
    }
    if attacker != 0 {
        if source_target(arena, attacker)
            .is_none_or(|target| target.class != 5 || target.model != SOURCE_MODEL)
        {
            put(&mut row, 154, attacker);
            row.record[77] = 10;
            row.set_source_action(218);
        }
        let mut position = row.position;
        crate::ground_creature_motion::relax_to_terrain(&mut position, terrain, 0, -512);
        row.set_position(position);
        arena.entities[slot] = row;
        crate::world::effects::sync_position(
            arena,
            terrain,
            root.source_slot,
            position,
            true,
            false,
        )?;
        return Ok(());
    }
    arena.entities[slot] = row;
    let descriptor = if matches!(mode, 1..=5) {
        Some(
            runtime
                .descriptor(arena.entities[slot].record[76])
                .ok_or("model27 authored StageVar descriptor missing")?,
        )
    } else {
        None
    };
    let point_route = descriptor.is_some_and(|descriptor| {
        mode == 1
            || (mode == 2
                && ((i32::from(arena.entities[slot].position.x as i16)
                    - i32::from(descriptor.point.x as i16))
                .abs()
                    > 3072
                    || (i32::from(arena.entities[slot].position.y as i16)
                        - i32::from(descriptor.point.y as i16))
                    .abs()
                        > 3072))
    });
    // Descriptor97 has behavior bit2, so the source 1E1C0 idle branch is a no-op.
    if point_route {
        combat::step_root_motion(arena, terrain, root, true)?;
        follow(
            arena,
            root,
            descriptor.expect("point route has descriptor").point,
            None,
            &families[usize::from(SOURCE_MODEL)],
        );
    }
    if matches!(mode, 3..=5) {
        combat::step_root_motion(arena, terrain, root, true)?;
        if arena.entities[slot].record[66] & 7 == 0 {
            let target = stage_actor(
                arena,
                root,
                descriptor.expect("actor route has descriptor"),
                families,
            );
            if let Some(target) = target {
                follow(
                    arena,
                    root,
                    target.position,
                    Some(target.array_pitch()),
                    &families[usize::from(SOURCE_MODEL)],
                );
                if matches!(mode, 3 | 4) {
                    let attack_target = if mode == 4 {
                        source_target(arena, word(&target, 154))
                    } else {
                        Some(target)
                    };
                    if let Some(target) = attack_target
                        && super::multipart::distance_3d(
                            arena.entities[slot].position,
                            target.position,
                        ) <= i32::from(PROPERTIES.root_target_range)
                    {
                        let row = &mut arena.entities[slot];
                        put(row, 154, target.index);
                        row.record[77] = 10;
                        row.set_source_action(218);
                    }
                }
            } else {
                let row = &mut arena.entities[slot];
                if row.record[66] & 15 == 0 && row.control_flags & 0x0004_0000 == 0 {
                    let rng = level2_next_rng16(row.entity_rng);
                    row.set_entity_rng(rng);
                    row.set_source_roll(
                        row.source_roll().wrapping_add((rng % 113 + 142) as i16) & 2047,
                    );
                }
            }
        }
    }
    if mode == 2
        && arena.entities[slot].action == 223
        && arena.entities[slot].record[77] == 2
        && arena.entities[slot].record[61] != 0
        && arena.entities[slot].record[66] % 16 == 0
    {
        let row = &arena.entities[slot];
        let mut nearest = None;
        let mut nearest_distance = u32::MAX;
        for &candidate in class3 {
            let Some(target) = source_target(arena, candidate) else {
                continue;
            };
            if target.control_flags & 0x20 != 0 {
                continue;
            }
            let dx = i32::from(target.position.x) - i32::from(row.position.x);
            let dy = i32::from(target.position.y.wrapping_sub(row.position.y) as i16);
            let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
            let heading =
                crate::level1_mission::projectile_guidance_angles(row.position, target.position).0;
            if distance <= u32::from(PROPERTIES.root_target_range).pow(2)
                && distance < nearest_distance
                && i32::from(angle_distance(i32::from(row.yaw), i32::from(heading)))
                    < i32::from(PROPERTIES.root_target_half_angle)
            {
                nearest = Some(candidate);
                nearest_distance = distance;
            }
        }
        if let Some(target) = nearest {
            let row = &mut arena.entities[slot];
            put(row, 154, target);
            row.record[77] = 10;
            row.set_source_action(218);
        }
    }
    Ok(())
}
