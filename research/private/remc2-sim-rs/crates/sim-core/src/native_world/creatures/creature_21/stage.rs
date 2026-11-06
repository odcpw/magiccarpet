//! Model21 StageVar callbacks, including the real source boss minion route.

use super::{
    Actor, Context, Effects, RULES, SOURCE_MODEL, Target, distance, jump, living, move_actor,
};
use crate::{FlightWorld, heading_for_positions};
fn stage_target(actor: &mut Actor<'_>, ctx: &Context<'_>, parent: Target) -> Option<Target> {
    let current = actor.target();
    if let Some(target) = ctx
        .target(current)
        .filter(|target| living(*target) && target.life > 0)
    {
        return Some(target);
    }
    actor.set_target(0);
    if actor.row.phase & 7 != 0 {
        return None;
    }
    let mut best = None;
    let mut shortest = u32::MAX;
    for target in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| matches!(target.model, 0 | 1) && target.source_id != parent.source_id)
    {
        let d = super::xy(parent.position, target.position);
        if d < shortest {
            shortest = d;
            best = Some(target);
        }
    }
    actor.set_target(best.map_or(0, |target| target.slot));
    best
}

fn summon_follow(actor: &mut Actor<'_>, ctx: &Context<'_>, target: Target, effects: &mut Effects) {
    let hit = super::consume(actor, ctx, true);
    if hit == 2 {
        actor.put(50, 1);
        return;
    }
    move_actor(actor, ctx, effects);
    if hit == 1 {
        let attacker = actor.attacker();
        if attacker != actor.stage_parent()
            && ctx
                .source_address(attacker)
                .is_none_or(|row| row.class != 5 || row.model != SOURCE_MODEL)
        {
            actor.set_target(attacker);
            actor.row.set_source_action(170);
            effects.feedback = Some((actor.stage_parent(), 19));
        }
    } else if actor.row.phase & 7 == 0 {
        if actor.row.control_flags & 0x40000 == 0 {
            actor
                .row
                .set_source_roll(heading_for_positions(actor.row.position, target.position) as i16);
            if actor.row.phase & 63 == 0 {
                let first = super::level2_next_rng16(actor.row.entity_rng);
                let second = super::level2_next_rng16(first);
                actor.row.set_entity_rng(second);
                let sign = 2 * (i32::from(first % 157) / 79) - 1;
                let roll = (i32::from(actor.row.source_roll())
                    + i32::from((second & 255) + 85) * sign) as i16
                    & 2047;
                actor.row.set_source_roll(roll);
            }
        }
        if let Some(peer) = ctx
            .model21
            .iter()
            .filter_map(|slot| ctx.target(*slot))
            .find(|peer| {
                peer.source_id != actor.row.source_id
                    && (i32::from(actor.row.position.x) - i32::from(peer.position.x)).abs()
                        < i32::from(actor.row.array_pitch())
                    && (i32::from(actor.row.position.y) - i32::from(peer.position.y)).abs()
                        < i32::from(actor.row.array_pitch())
            })
        {
            actor
                .row
                .set_source_roll(heading_for_positions(peer.position, actor.row.position) as i16);
        }
    }
}

fn alliance_blink(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects, clear: bool) {
    let remaining = actor.word(50) as i16;
    let duration = actor.word(52) as i16;
    let phase = if clear {
        0
    } else if remaining <= duration.wrapping_sub(16) && remaining >= 16 {
        if actor.row.phase & 0x1c == 16 {
            actor.row.phase & 3
        } else {
            0
        }
    } else if actor.row.phase & 1 != 0 {
        0
    } else {
        2
    };
    let flags = match phase {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    actor
        .row
        .set_control_flags((actor.row.control_flags & !0x0600_0000) | flags);
    let mut child = actor.word(56);
    for _ in 0..=ctx.targets.len() {
        let Some(linked) = ctx.target(child) else {
            break;
        };
        effects.linked_blink_flags.push((linked.slot, flags));
        child = linked.next;
    }
}

pub(super) fn alliance_target(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    effects: &mut Effects,
    target: Option<Target>,
) -> Option<Target> {
    if actor.stage_mode() != 14 {
        return target;
    }
    alliance_blink(actor, ctx, effects, false);
    let remaining = (actor.word(50) as i16).wrapping_sub(1);
    actor.put(50, remaining as u16);
    if remaining <= 0 {
        return None;
    }
    let Some(parent) = ctx.target(actor.stage_parent()) else {
        return target;
    };
    let target = target?;
    let selected = if parent.target != 0 {
        parent.target
    } else {
        parent.attacker
    };
    (target.slot != parent.slot && (selected == 0 || selected == target.slot)).then_some(target)
}

fn alliance(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let remaining = (actor.word(50) as i16).wrapping_sub(1);
    actor.put(50, remaining as u16);
    let parent = if remaining > 0 {
        alliance_blink(actor, ctx, effects, false);
        ctx.target(actor.stage_parent())
            .filter(|parent| parent.life > 0)
    } else {
        None
    };
    let Some(parent) = parent else {
        actor.row.record[76] = actor.row.record[78];
        actor.row.record[77] = 10;
        actor.put(44, 0);
        actor.set_target(0);
        alliance_blink(actor, ctx, effects, true);
        return;
    };
    let saved_target = actor.target();
    actor.set_target(parent.slot);
    summon_follow(actor, ctx, parent, effects);
    let selected = if parent.target != 0 {
        parent.target
    } else {
        parent.attacker
    };
    actor.set_target(if selected != 0 {
        selected
    } else {
        saved_target
    });
    if let Some(target) = ctx.target(actor.target()) {
        if (target.parent == parent.slot && target.stage.mode == 14)
            || target.life <= 0
            || target.control_flags & 0x400 != 0
        {
            actor.set_target(0);
        } else if distance(actor.row.position, target.position) < RULES.target_range {
            actor.row.set_source_action(170);
            effects.earlier_feedback = effects.feedback.take();
            effects.feedback = Some((parent.slot, 24));
        }
    }
}

/// Source sub_1E320; after reaching the target it falls through to the
/// persistent model21 StageVar16 follow/attack state.
fn stage17(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    move_actor(actor, ctx, effects);
    if actor.row.life < 0 {
        actor.row.disable_drawing();
        return;
    }
    let Some(target) = ctx.target(actor.target()).filter(|target| living(*target)) else {
        actor.row.record[77] = 16;
        return;
    };
    actor
        .row
        .set_source_roll(heading_for_positions(actor.row.position, target.position) as i16);
    actor
        .row
        .set_actual_speed(actor.row.actual_speed.wrapping_sub(8));
    if actor.row.actual_speed <= 16 {
        actor.row.set_actual_speed(RULES.minimum_speed);
        actor.row.record[77] = 16;
    }
}

fn follow_parent(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    if let Some(parent) = ctx.source_address(actor.stage_parent()) {
        if actor.stage_parent() == 0 || parent.life < 0 {
            actor.row.disable_drawing();
        }
        let mut position = parent.position;
        position.z = position.z.max(0);
        ctx.terrain.set_source_predicted_position(position);
        actor.row.set_position(position);
        actor.row.set_yaw(parent.yaw as i16);
        actor.row.set_source_roll(parent.roll as i16);
    } else {
        actor.row.disable_drawing();
        ctx.terrain.set_source_predicted_position(crate::Position::default());
        actor.row.set_position(crate::Position::default());
        actor.row.set_yaw(0);
        actor.row.set_source_roll(0);
    }
}

fn point_route(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    point: crate::Position,
    effects: &mut Effects,
) {
    match super::consume(actor, ctx, true) {
        2 => actor.row.set_source_action(172),
        1 => {
            let attacker = actor.attacker();
            if ctx
                .source_address(attacker)
                .is_none_or(|source| source.class != 5 || source.model != SOURCE_MODEL)
            {
                actor.set_target(attacker);
                actor.row.record[77] = 10;
                actor.row.set_source_action(170);
            }
            let mut position = actor.row.position;
            position.z = ctx
                .terrain
                .ground_height(position)
                .wrapping_add(RULES.ground_clearance);
            actor.row.set_position(position);
        }
        _ => {
            move_actor(actor, ctx, effects);
            steer(actor, ctx, point, true);
        }
    }
}

fn steer(actor: &mut Actor<'_>, ctx: &Context<'_>, point: crate::Position, point_route: bool) {
    if actor.row.phase & 7 != 0 {
        return;
    }
    if actor.row.control_flags & 0x40000 == 0 {
        actor
            .row
            .set_source_roll(heading_for_positions(actor.row.position, point) as i16);
        if actor.row.phase & 63 == 0 {
            let first = super::level2_next_rng16(actor.row.entity_rng);
            let second = super::level2_next_rng16(first);
            actor.row.set_entity_rng(second);
            let sign = 2 * (i32::from(first % 157) / 79) - 1;
            actor.row.set_source_roll(
                (i32::from(actor.row.source_roll()) + i32::from((second & 255) + 85) * sign) as i16
                    & 2047,
            );
        }
    }
    if let Some(peer) = ctx
        .model21
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .find(|peer| {
            let delta = |a: u16, b: u16| if point_route {
                (i32::from(a as i16) - i32::from(b as i16)).abs()
            } else {
                (i32::from(a) - i32::from(b)).abs()
            };
            peer.source_id != actor.row.source_id
                && delta(actor.row.position.x, peer.position.x) < i32::from(actor.row.array_pitch())
                && delta(actor.row.position.y, peer.position.y) < i32::from(actor.row.array_pitch())
        })
    {
        actor
            .row
            .set_source_roll(heading_for_positions(peer.position, actor.row.position) as i16);
    }
}

fn routed_target(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    descriptor: super::super::archer::ArcherStageDescriptor,
) -> Option<Target> {
    let slot = if descriptor.flags & 2 == 0 {
        descriptor.actor
    } else {
        if actor.stage_relation() == 0 {
            let shared = if descriptor.flags & 1 != 0 {
                ctx.model21
                    .iter()
                    .filter_map(|slot| ctx.target(*slot))
                    .find(|peer| {
                        peer.stage.index == actor.stage_index() && peer.stage.relation != 0
                    })
                    .map_or(0, |peer| peer.stage.relation)
            } else {
                0
            };
            let nearest = if shared == 0 {
                ctx.families
                    .get(usize::from(descriptor.model))
                    .into_iter()
                    .flatten()
                    .filter_map(|slot| ctx.target(*slot))
                    .min_by_key(|target| super::xy(actor.row.position, target.position))
                    .map_or(0, |target| target.slot)
            } else {
                shared
            };
            actor.put(78, nearest);
        }
        actor.stage_relation()
    };
    ctx.target(slot)
}

fn idle_turn(actor: &mut Actor<'_>) {
    if actor.row.phase & 15 == 0 && actor.row.control_flags & 0x40000 == 0 {
        let rng = super::level2_next_rng16(actor.row.entity_rng);
        actor.row.set_entity_rng(rng);
        actor
            .row
            .set_source_roll((actor.row.source_roll() + (rng % 113 + 142) as i16) & 2047);
    }
}

fn guard_target(actor: &Actor<'_>, ctx: &Context<'_>) -> Option<Target> {
    let distance = |target: &Target| {
        let dx = i32::from(target.position.x) - i32::from(actor.row.position.x);
        let dy = i32::from(target.position.y.wrapping_sub(actor.row.position.y) as i16);
        dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
    };
    ctx.class3.iter().filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.control_flags & 0x20 == 0)
        .filter(|target| distance(target) <= RULES.target_range.wrapping_mul(RULES.target_range)
            && crate::angle_distance(i32::from(actor.row.yaw),
                heading_for_positions(actor.row.position, target.position)) < RULES.target_half_angle)
        .min_by_key(distance)
}

/// The 1E1C0 wander helper used by modes6–9 never reads StageVar1.
fn wander(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    match super::consume(actor, ctx, true) {
        2 => actor.row.set_source_action(172),
        1 => {
            let attacker = actor.attacker();
            if ctx.source_address(attacker)
                .is_none_or(|source| source.class != 5 || source.model != SOURCE_MODEL)
            {
                actor.set_target(attacker);
                actor.row.record[77] = 10;
                actor.row.set_source_action(170);
            }
            let mut position = actor.row.position;
            position.z = ctx.terrain.ground_height(position).wrapping_add(RULES.ground_clearance);
            actor.row.set_position(position);
        }
        _ if RULES.behavior_flags & 2 == 0 => {
            move_actor(actor, ctx, effects);
            idle_turn(actor);
        }
        _ => {}
    }
}

fn routed(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    descriptor: super::super::archer::ArcherStageDescriptor,
    mode: u8,
    effects: &mut Effects,
) {
    let near_point = (i32::from(actor.row.position.x as i16)
        - i32::from(descriptor.point.x as i16))
    .abs()
        <= 3072
        && (i32::from(actor.row.position.y as i16) - i32::from(descriptor.point.y as i16)).abs()
            <= 3072;
    if mode == 2 && !near_point {
        point_route(actor, ctx, descriptor.point, effects);
    } else {
        match super::consume(actor, ctx, true) {
            2 => actor.row.set_source_action(172),
            1 => {
                let attacker = actor.attacker();
                if ctx
                    .source_address(attacker)
                    .is_none_or(|source| source.class != 5 || source.model != SOURCE_MODEL)
                {
                    actor.set_target(attacker);
                    actor.row.record[77] = 10;
                    actor.row.set_source_action(170);
                }
                let mut position = actor.row.position;
                position.z = ctx
                    .terrain
                    .ground_height(position)
                    .wrapping_add(RULES.ground_clearance);
                actor.row.set_position(position);
            }
            _ => {
                if matches!(mode, 3..=5) || RULES.behavior_flags & 2 == 0 {
                    move_actor(actor, ctx, effects);
                    if matches!(mode, 3..=5) && actor.row.phase & 7 == 0 {
                        if let Some(target) = routed_target(actor, ctx, descriptor) {
                            steer(actor, ctx, target.position, false);
                            let range =
                                i32::from(actor.row.array_pitch()) + i32::from(target.half_width);
                            if (i32::from(actor.row.position.x) - i32::from(target.position.x))
                                .abs()
                                < range
                                && (i32::from(actor.row.position.y) - i32::from(target.position.y))
                                    .abs()
                                    < range
                            {
                                actor.row.set_source_roll(heading_for_positions(
                                    target.position,
                                    actor.row.position,
                                ) as i16);
                            }
                        } else {
                            idle_turn(actor);
                        }
                    } else if !matches!(mode, 3..=5) {
                        idle_turn(actor);
                    }
                }
            }
        }
    }
    if matches!(mode, 3 | 4) && actor.row.phase & 7 == 0 {
        let target_slot = if descriptor.flags & 2 != 0 {
            actor.stage_relation()
        } else {
            descriptor.actor
        };
        if let Some(target) = ctx.target(target_slot) {
            let selected = if mode == 4 {
                ctx.target(target.target)
            } else {
                Some(target)
            };
            if let Some(selected) = selected.filter(|target| target.slot != 0xae02)
                && distance(actor.row.position, selected.position) <= RULES.target_range
            {
                actor.set_target(selected.slot);
                actor.row.record[77] = 10;
                actor.row.set_source_action(170);
            }
        }
    }
    if mode == 2
        && actor.row.action == 175
        && actor.stage_mode() == 2
        && actor.row.record[61] != 0
        && actor.row.phase % RULES.target_scan_period == 0
        && let Some(target) = guard_target(actor, ctx)
    {
        actor.set_target(target.slot);
        actor.row.record[77] = 10;
        actor.row.set_source_action(170);
    }
}

fn summoned(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let Some(parent) = ctx
        .target(actor.stage_parent())
        .filter(|parent| living(*parent))
    else {
        actor.row.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
        effects.expire = true;
        effects.summon_expire = actor.stage_mode() == 13;
        return;
    };
    let mut remaining = actor.word(50) as i16;
    if actor.stage_mode() == 13 {
        remaining = remaining.wrapping_sub(1);
        actor.put(50, remaining as u16);
    }
    if remaining <= 0 {
        effects.expire = true;
        effects.summon_expire = actor.stage_mode() == 13;
        return;
    }
    if let Some(target) = stage_target(actor, ctx, parent) {
        summon_follow(actor, ctx, target, effects);
        if actor.row.phase & 7 == 0
            && distance(actor.row.position, target.position) < RULES.target_range
        {
            actor.row.set_source_action(170);
        }
    } else {
        actor.set_target(parent.slot);
        summon_follow(actor, ctx, parent, effects);
        actor.set_target(0);
        actor.put(50, (actor.word(50) as i16).wrapping_sub(4) as u16);
    }
}

pub(super) fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>, effects: &mut Effects) {
    match actor.stage_mode() {
        17 => stage17(actor, ctx, effects),
        13 | 16 => {
            summoned(actor, ctx, effects);
            if !effects.expire {
                actor.row.record[71] = 0;
                jump(actor, ctx, effects);
            }
        }
        14 => {
            alliance(actor, ctx, effects);
            if actor.stage_mode() == 14 {
                actor.row.record[71] = 0;
            }
            jump(actor, ctx, effects);
        }
        1 => {
            if let Some(descriptor) = ctx.runtime.descriptor(actor.stage_index()) {
                point_route(actor, ctx, descriptor.point, effects);
            }
            jump(actor, ctx, effects);
        }
        10 => {
            actor.row.set_source_action(170);
            jump(actor, ctx, effects);
        }
        12 => follow_parent(actor, ctx),
        mode if (2..=5).contains(&mode) => {
            if let Some(descriptor) = ctx.runtime.descriptor(actor.stage_index()) {
                routed(actor, ctx, descriptor, mode, effects);
            }
            jump(actor, ctx, effects);
        }
        mode if (6..=9).contains(&mode) => {
            wander(actor, ctx, effects);
            jump(actor, ctx, effects);
        }
        _ => {}
    }
    // sub_26470 normalizes the final action only after its jump callback.
    if let Some((_, continuation)) = &mut effects.water_splash_tail {
        *continuation = super::WaterContinuation::Stage;
    } else if actor.row.action != 175 {
        actor.set_action(actor.row.action);
    }
}
