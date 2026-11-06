//! Model-17 `sub_24DF0 -> sub_1D5D0(136)` StageVar callback.

use super::{
    Creature17Context, Creature17Effects, Creature17State, RULES, Target, consume, jitter,
    move_actor, settle,
};
use crate::native_world::RuntimeRelation;
use crate::{Position, heading_for_positions};

fn live(ctx: &Creature17Context<'_>, slot: u16) -> Option<Target> {
    ctx.target(slot)
        .filter(|target| target.alive() && target.flags & 0x400 == 0)
}

fn stage_target(actor: &mut Creature17State, ctx: &Creature17Context<'_>) -> Option<Target> {
    let stage = ctx
        .runtime
        .stage_variables
        .get(usize::from(actor.route_index))?;
    if stage.flags & 2 == 0 {
        let slot = match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        };
        return ctx.target(slot);
    }
    if actor.route_relation == 0 {
        if stage.flags & 1 != 0 {
            actor.route_relation = ctx.model17.iter().filter_map(|slot| ctx.target(*slot))
                .find(|peer| peer.route_index == actor.route_index && peer.route_relation != 0)
                .map_or(0, |peer| peer.route_relation);
        }
        let model = match stage.relation {
            RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u8,
            _ => 0,
        };
        if actor.route_relation == 0 {
            actor.route_relation = ctx.models.and_then(|models| models.get(usize::from(model)))
                .into_iter().flatten().filter_map(|slot| ctx.target(*slot))
                .min_by_key(|target| super::xy_squared(actor.position, target.position))
                .map_or(0, |target| target.slot);
        }
    }
    ctx.target(actor.route_relation)
}

fn point(actor: &Creature17State, ctx: &Creature17Context<'_>) -> Option<Position> {
    let stage = ctx
        .runtime
        .stage_variables
        .get(usize::from(actor.route_index))?;
    let packed = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value,
        _ => return None,
    };
    Some(Position {
        x: packed as u16,
        y: (packed >> 16) as u16,
        z: 0,
    })
}

fn damage(actor: &mut Creature17State, ctx: &Creature17Context<'_>) -> u8 {
    match consume(actor, ctx) {
        2 => {
            actor.action = 140;
            2
        }
        1 => {
            if ctx.target(actor.attacker).is_none_or(|attacker| {
                (attacker.class, attacker.model) != (super::SOURCE_CLASS, super::SOURCE_MODEL)
            }) {
                actor.target = actor.attacker;
                actor.route_mode = 10;
                actor.action = 138;
            }
            settle(actor, ctx.terrain);
            1
        }
        _ => 0,
    }
}

fn roam_to(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
    destination: Position,
    signed_axes: bool,
) {
    move_actor(actor, ctx.terrain, effects);
    if actor.phase & 7 == 0 {
        if actor.control_flags & 0x40000 == 0 {
            actor.roll = heading_for_positions(actor.position, destination) as i16;
            if actor.phase & 63 == 0 {
                jitter(actor);
            }
        }
        if let Some(peer) = ctx
            .model17
            .iter()
            .filter_map(|slot| ctx.target(*slot))
            .find(|peer| {
                let axis = |value: u16| if signed_axes { i32::from(value as i16) } else { i32::from(value) };
                peer.source_id != actor.source_id
                    && (axis(actor.position.x) - axis(peer.position.x)).abs() < i32::from(actor.half_width)
                    && (axis(actor.position.y) - axis(peer.position.y)).abs() < i32::from(actor.half_width)
            })
        {
            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
        }
    }
}

fn nearest_guard(actor: &Creature17State, ctx: &Creature17Context<'_>) -> Option<Target> {
    if actor.proximity == 0 || actor.phase % RULES.source_scan_period != 0 { return None; }
    let limit = RULES.target_range.wrapping_mul(RULES.target_range);
    let mut best = None;
    let mut distance = u32::MAX;
    for candidate in ctx.class3.iter().filter_map(|slot| ctx.target(*slot)) {
        let dx = i32::from(candidate.position.x) - i32::from(actor.position.x);
        let dy = i32::from(candidate.position.y.wrapping_sub(actor.position.y) as i16);
        let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
        if candidate.flags & 0x20 == 0 && squared <= limit && squared < distance
            && crate::angle_distance(i32::from(actor.yaw), heading_for_positions(actor.position, candidate.position))
                < RULES.target_half_angle
        {
            best = Some(candidate);
            distance = squared;
        }
    }
    best
}

fn bound_target(actor: &Creature17State, ctx: &Creature17Context<'_>) -> Option<Target> {
    let stage = ctx.runtime.stage_variables.get(usize::from(actor.route_index))?;
    let slot = if stage.flags & 2 != 0 { actor.route_relation } else {
        match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        }
    };
    ctx.target(slot)
}

fn wander(actor: &mut Creature17State, ctx: &Creature17Context<'_>, effects: &mut Creature17Effects) {
    move_actor(actor, ctx.terrain, effects);
    if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
        actor.rng = crate::native_world::level2_next_rng16(actor.rng);
        actor.roll = actor.roll.wrapping_add((actor.rng % 113 + 142) as i16) & 2047;
    }
}

fn follow_parent(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    let parent = ctx.source_address(actor.parent);
    if actor.parent == 0 || parent.is_none_or(|parent| parent.life < 0) {
        effects.route_hide = true;
    }
    if let Some(parent) = parent {
        actor.position = parent.position;
        actor.position.z = actor.position.z.max(0);
        actor.yaw = parent.yaw;
        actor.roll = parent.roll;
    }
}

fn alliance_blink(actor: &mut Creature17State, ctx: &Creature17Context<'_>, effects: &mut Creature17Effects, clear: bool) {
    let phase = if clear { 0 } else if actor.route_remaining <= actor.route_duration.wrapping_sub(16)
        && actor.route_remaining >= 16 {
        if actor.phase & 0x1c == 16 { actor.phase & 3 } else { 0 }
    } else if actor.phase & 1 != 0 { 0 } else { 2 };
    let bits = match phase { 1 | 3 => 0x0200_0000, 2 => 0x0400_0000, _ => 0 };
    actor.control_flags = (actor.control_flags & !0x0600_0000) | bits;
    let mut child = actor.linked_child;
    for _ in 0..=ctx.targets.len() {
        let Some(row) = ctx.target(child) else { break };
        effects.linked_blink_flags.push((row.slot, bits));
        child = row.linked_child;
    }
}

pub(super) fn alliance_target(actor: &mut Creature17State, ctx: &Creature17Context<'_>, effects: &mut Creature17Effects, target: Option<Target>) -> Option<Target> {
    if actor.route_mode != 14 { return target; }
    alliance_blink(actor, ctx, effects, false);
    actor.route_remaining = actor.route_remaining.wrapping_sub(1);
    if actor.route_remaining <= 0 { return None; }
    let Some(parent) = ctx.target(actor.parent) else { return target };
    let target = target?;
    let selected = if parent.target != 0 { parent.target } else { parent.attacker };
    (target.slot != parent.slot && (selected == 0 || selected == target.slot)).then_some(target)
}

/// sub_1E580 -> sub_1E700. Fixed-address parent/target reads retain their
/// source life/flag tests; only mode13 consumes the ordinary countdown.
fn follow_summoner(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    let parent = live(ctx, actor.parent);
    if parent.is_none() {
        actor.route_remaining = 0;
    } else if actor.route_mode == 13 {
        actor.route_remaining = actor.route_remaining.wrapping_sub(1);
    }
    if actor.route_remaining <= 0 {
        effects.stage_expiry = Some(actor.route_mode == 13);
        return;
    }
    let parent = parent.expect("live source parent checked above");
    let mut target = ctx
        .target(actor.target)
        .filter(|target| target.life > 0 && target.flags & 0x400 == 0);
    if target.is_none() {
        actor.target = 0;
        if actor.phase & 7 == 0 {
            // sub_16FC0 scans frozen membership/current packets, testing
            // model and source identity without another life/hide filter.
            target = ctx
                .class3
                .iter()
                .filter_map(|slot| ctx.target(*slot))
                .filter(|candidate| {
                    matches!(candidate.model, 0 | 1) && candidate.source_id != parent.source_id
                })
                .min_by_key(|candidate| super::xy_squared(parent.position, candidate.position));
            actor.target = target.map_or(0, |target| target.slot);
        }
    }
    if target.is_none() {
        actor.target = actor.parent;
    }
    match consume(actor, ctx) {
        2 => actor.route_remaining = 1,
        hit => {
            move_actor(actor, ctx.terrain, effects);
            if hit == 1 {
                if actor.attacker != actor.parent
                    && ctx.target(actor.attacker).is_none_or(|attacker| {
                        (attacker.class, attacker.model)
                            != (super::SOURCE_CLASS, super::SOURCE_MODEL)
                    })
                {
                    actor.target = actor.attacker;
                    actor.action = 138;
                    effects.feedback.push((actor.parent, 19));
                }
            } else if actor.phase & 7 == 0 {
                let destination = target.unwrap_or(parent);
                if actor.control_flags & 0x40000 == 0 {
                    actor.roll = heading_for_positions(actor.position, destination.position) as i16;
                    if actor.phase & 63 == 0 {
                        jitter(actor);
                    }
                }
                if let Some(peer) = ctx
                    .model17
                    .iter()
                    .filter_map(|slot| ctx.target(*slot))
                    .find(|peer| {
                        peer.source_id != actor.source_id
                            && actor.position.x.abs_diff(peer.position.x)
                                < actor.half_width.max(0) as u16
                            && actor.position.y.abs_diff(peer.position.y)
                                < actor.half_width.max(0) as u16
                    })
                {
                    actor.roll = heading_for_positions(peer.position, actor.position) as i16;
                }
            }
        }
    }
    if let Some(target) = target {
        if actor.phase & 7 == 0
            && super::distance(actor.position, target.position) < RULES.target_range
        {
            actor.action = 138;
        }
    } else {
        actor.target = 0;
        actor.route_remaining = actor.route_remaining.wrapping_sub(4);
    }
}

pub(super) fn step(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    match actor.route_mode {
        1 => {
            if damage(actor, ctx) != 0 {
                return;
            }
            if let Some(destination) = point(actor, ctx) {
                roam_to(actor, ctx, effects, destination, true);
            }
        }
        2 => {
            let hit = damage(actor, ctx);
            if hit == 0 && let Some(destination) = point(actor, ctx) {
                let dx = (i32::from(actor.position.x as i16) - i32::from(destination.x as i16)).abs();
                let dy = (i32::from(actor.position.y as i16) - i32::from(destination.y as i16)).abs();
                if dx > 3072 || dy > 3072 {
                    roam_to(actor, ctx, effects, destination, true);
                } else {
                    wander(actor, ctx, effects);
                }
            }
            if actor.action == 143 && actor.route_mode == 2
                && let Some(target) = nearest_guard(actor, ctx)
            {
                actor.target = target.slot;
                actor.route_mode = 10;
                actor.action = 138;
            }
        }
        3..=5 => {
            let mode = actor.route_mode;
            let hit = damage(actor, ctx);
            if hit == 0 {
                move_actor(actor, ctx.terrain, effects);
                if actor.phase & 7 == 0 {
                    if let Some(target) = stage_target(actor, ctx) {
                        if actor.control_flags & 0x40000 == 0 {
                            actor.roll = heading_for_positions(actor.position, target.position) as i16;
                            if actor.phase & 63 == 0 { jitter(actor); }
                        }
                        if let Some(peer) = ctx.model17.iter().filter_map(|slot| ctx.target(*slot)).find(|peer| {
                            peer.source_id != actor.source_id
                                && i32::from(actor.position.x.abs_diff(peer.position.x)) < i32::from(actor.half_width)
                                && i32::from(actor.position.y.abs_diff(peer.position.y)) < i32::from(actor.half_width)
                        }) {
                            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
                        }
                        let width = i32::from(actor.half_width) + i32::from(target.half_width);
                        if i32::from(actor.position.x.abs_diff(target.position.x)) < width
                            && i32::from(actor.position.y.abs_diff(target.position.y)) < width
                        {
                            actor.roll = heading_for_positions(target.position, actor.position) as i16;
                        }
                    } else if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
                        actor.rng = crate::native_world::level2_next_rng16(actor.rng);
                        actor.roll = actor.roll.wrapping_add((actor.rng % 113 + 142) as i16) & 2047;
                    }
                }
            }
            // sub_1D7C0/1D700 run this tail even after a hit or lethal result.
            if mode != 5 && actor.phase & 7 == 0
                && let Some(followed) = bound_target(actor, ctx)
            {
                let target = if mode == 3 { Some(followed) } else { ctx.target(followed.target) };
                if let Some(target) = target && super::distance(actor.position, target.position) <= RULES.target_range {
                    actor.target = target.slot;
                    actor.route_mode = 10;
                    actor.action = 138;
                }
            }
        }
        6..=9 => {
            if damage(actor, ctx) == 0 { wander(actor, ctx, effects); }
        }
        10 => {
            actor.action = 138;
        }
        12 => follow_parent(actor, ctx, effects),
        13 | 16 => follow_summoner(actor, ctx, effects),
        14 => {
            actor.route_remaining = actor.route_remaining.wrapping_sub(1);
            if actor.route_remaining > 0 { alliance_blink(actor, ctx, effects, false); }
            let parent = if actor.route_remaining > 0 { ctx.target(actor.parent).filter(|row| row.life > 0) } else { None };
            if let Some(parent) = parent {
                let previous = actor.target;
                actor.target = actor.parent;
                match consume(actor, ctx) {
                    2 => actor.route_remaining = 1,
                    hit => {
                        if hit == 0 {
                            roam_to(actor, ctx, effects, parent.position, false);
                        } else {
                            move_actor(actor, ctx.terrain, effects);
                            if actor.attacker != actor.parent && ctx.target(actor.attacker)
                                .is_none_or(|row| (row.class, row.model) != (super::SOURCE_CLASS, super::SOURCE_MODEL)) {
                                actor.target = actor.attacker;
                                actor.action = 138;
                                effects.feedback.push((actor.parent, 19));
                            }
                        }
                    }
                }
                let inherited = if parent.target != 0 { parent.target } else { parent.attacker };
                actor.target = if inherited != 0 { inherited } else { previous };
                if let Some(target) = ctx.target(actor.target) {
                    if (target.parent == actor.parent && target.route_mode == 14) || target.life <= 0 || target.flags & 0x400 != 0 {
                        actor.target = 0;
                    } else if super::distance(actor.position, target.position) < RULES.target_range {
                        actor.action = 138;
                        effects.feedback.push((actor.parent, 24));
                    }
                }
            } else {
                actor.route_mode = 10;
                actor.route_index = actor.route_relation as u8;
                actor.parent = 0;
                actor.target = 0;
                alliance_blink(actor, ctx, effects, true);
            }
        }
        17 => {
            move_actor(actor, ctx.terrain, effects);
            if actor.life < 0 {
                effects.route_hide = true;
            } else if let Some(target) = live(ctx, actor.target) {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.route_mode = 16;
                }
            } else {
                actor.route_mode = 16;
            }
        }
        _ => {}
    }
}
