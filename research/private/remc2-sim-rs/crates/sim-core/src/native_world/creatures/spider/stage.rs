//! StageVar routes for the same Spider arena row.

use super::{
    RULES, SOURCE_MODEL, SpiderContext, SpiderEffects, SpiderState, StageDescriptor, StageRuntime,
    Target, consume, distance, heading_for_positions, jitter, level2_next_rng16, move_actor,
    settle, xy,
};
use crate::Position;

pub(crate) fn bind_stage(actor: &mut SpiderState, index: u8, runtime: &mut dyn StageRuntime) {
    if index != 0
        && let Some(route) = runtime.bind(index)
    {
        actor.stage.index = index;
        actor.stage.mode = route.kind;
        actor.stage.relation = if route.kind == 6 { route.scalar } else { 0 };
        actor.action = 167;
    } else {
        actor.stage.index = 0;
        actor.stage.mode = 0;
        actor.stage.relation = 0;
        actor.action = 161;
    }
}

fn wrapped_near(a: Position, b: Position, radius: u16) -> bool {
    a.x.wrapping_sub(b.x).cast_signed().unsigned_abs() <= radius
        && a.y.wrapping_sub(b.y).cast_signed().unsigned_abs() <= radius
}
fn signed_near(a: Position, b: Position, radius: u32) -> bool {
    i32::from(a.x as i16).abs_diff(i32::from(b.x as i16)) <= radius
        && i32::from(a.y as i16).abs_diff(i32::from(b.y as i16)) <= radius
}

pub(crate) fn stage_prepass(actor: &mut SpiderState, ctx: &mut SpiderContext<'_>) {
    let action = actor.action & 7;
    if matches!(action, 4 | 5) {
        return;
    }
    if matches!(actor.stage.mode, 13 | 14 | 16 | 17) {
        if !matches!(action, 2 | 6) {
            actor.action = 167;
        }
        return;
    }
    if matches!(actor.stage.mode, 10 | 15) {
        if ((actor.stage.mode == 10 && !matches!(action, 2 | 6))
            || (actor.stage.mode == 15 && action != 0))
            && let Some(runtime) = ctx.stage_runtime.as_mut()
        {
            bind_stage(actor, actor.stage.index, *runtime);
        }
        return;
    }
    let Some(route) = ctx
        .stage_runtime
        .as_ref()
        .and_then(|r| r.descriptor(actor.stage.index))
    else {
        return;
    };
    let ended = match route.kind {
        1 => wrapped_near(actor.position, route.point, 2048),
        3 | 4 | 5 | 8 | 9 => {
            if route.flags & 4 != 0 {
                if route.kind == 3 {
                    actor.stage.relation = 0;
                }
                !matches!(action, 2 | 6) || route.kind != 3
            } else {
                if route.flags & 2 != 0
                    && matches!(route.kind, 3..=5)
                    && ctx
                        .target(actor.stage.relation)
                        .is_some_and(|t| !t.living())
                {
                    actor.stage.relation = 0;
                }
                route.kind == 9
                    && route.flags & 2 == 0
                    && (route.point.x != 0 || route.point.y != 0)
                    && wrapped_near(actor.position, route.point, 3072)
            }
        }
        6 => {
            actor.stage.relation = actor.stage.relation.wrapping_sub(1);
            actor.stage.relation == 0
        }
        7 => route.flags & 0x18 != 0,
        _ => false,
    };
    if ended && let Some(runtime) = ctx.stage_runtime.as_mut() {
        if route.kind == 7 {
            runtime.rearm_model_relations();
        }
        bind_stage(actor, route.next, *runtime);
    }
}

fn follow(
    actor: &mut SpiderState,
    ctx: &SpiderContext<'_>,
    point: Position,
    signed: bool,
    width: Option<i16>,
) {
    if actor.phase & 7 != 0 {
        return;
    }
    if actor.control_flags & 0x40000 == 0 {
        actor.roll = heading_for_positions(actor.position, point) as i16;
        if actor.phase & 63 == 0 {
            jitter(actor);
        }
    }
    if let Some(peer) = ctx.model20.iter().filter_map(|s| ctx.target(*s)).find(|t| {
        t.source_id != actor.source_id
            && if signed {
                signed_near(
                    actor.position,
                    t.position,
                    actor.half_width.saturating_sub(1) as u32,
                )
            } else {
                actor.position.x.abs_diff(t.position.x) < actor.half_width as u16
                    && actor.position.y.abs_diff(t.position.y) < actor.half_width as u16
            }
    }) {
        actor.roll = heading_for_positions(peer.position, actor.position) as i16;
        return;
    }
    if let Some(width) = width {
        let separation = i32::from(actor.half_width) + i32::from(width);
        if i32::from(actor.position.x.abs_diff(point.x)) < separation
            && i32::from(actor.position.y.abs_diff(point.y)) < separation
        {
            actor.roll = heading_for_positions(point, actor.position) as i16;
        }
    }
}
fn idle(actor: &mut SpiderState) {
    if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
        actor.rng = level2_next_rng16(actor.rng);
        actor.roll = actor.roll.wrapping_add((actor.rng % 113 + 142) as i16) & 2047;
    }
}
fn selected(
    actor: &mut SpiderState,
    ctx: &SpiderContext<'_>,
    route: StageDescriptor,
) -> Option<Target> {
    let slot = if route.flags & 2 == 0 {
        route.actor
    } else {
        if actor.stage.relation == 0 {
            if route.flags & 1 != 0 {
                actor.stage.relation = ctx
                    .model20
                    .iter()
                    .filter_map(|s| ctx.target(*s))
                    .find(|t| t.stage.index == actor.stage.index && t.stage.relation != 0)
                    .map_or(0, |t| t.stage.relation);
            }
            if actor.stage.relation == 0 {
                let mut best = u32::MAX;
                for target in ctx
                    .class5
                    .iter()
                    .filter_map(|s| ctx.target(*s))
                    .filter(|t| t.model == route.model)
                {
                    let d = xy(actor.position, target.position);
                    if d < best {
                        best = d;
                        actor.stage.relation = target.slot;
                    }
                }
            }
        }
        actor.stage.relation
    };
    ctx.target(slot)
}
fn blink(
    actor: &mut SpiderState,
    ctx: &SpiderContext<'_>,
    effects: &mut SpiderEffects,
    clear: bool,
) {
    let phase = if clear {
        0
    } else if actor.stage.remaining <= actor.stage.duration.wrapping_sub(16)
        && actor.stage.remaining >= 16
    {
        if actor.phase & 0x1c == 16 {
            actor.phase & 3
        } else {
            0
        }
    } else if actor.phase & 1 != 0 {
        0
    } else {
        2
    };
    let bits = match phase {
        1 | 3 => 0x02000000,
        2 => 0x04000000,
        _ => 0,
    };
    actor.control_flags = (actor.control_flags & !0x06000000) | bits;
    let mut next = actor.next;
    for _ in 0..=ctx.targets.len() {
        let Some(mut child) = ctx.target(next) else {
            break;
        };
        next = child.next;
        child.flags = (child.flags & !0x06000000) | bits;
        effects.linked_updates.push(child);
    }
}

/// sub_1ED30 is called by acquisition, attack AND flee. Its countdown is
/// separate from the StageVar14 callback and can therefore advance twice.
pub(super) fn alliance_target(
    actor: &mut SpiderState,
    ctx: &SpiderContext<'_>,
    target: Target,
    effects: &mut SpiderEffects,
) -> Option<Target> {
    if actor.stage.mode != 14 {
        return Some(target);
    }
    blink(actor, ctx, effects, false);
    actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
    if actor.stage.remaining <= 0 {
        return None;
    }
    if let Some(parent) = ctx.target(actor.stage.parent) {
        if parent.slot == target.slot {
            return None;
        }
        let selected = if parent.target != 0 {
            parent.target
        } else {
            parent.attacker
        };
        if selected != 0 && selected != target.slot {
            return None;
        }
    }
    Some(target)
}

fn summon_follow(actor: &mut SpiderState, ctx: &SpiderContext<'_>, effects: &mut SpiderEffects) {
    let hit = consume(actor, ctx, true);
    if hit == 2 {
        actor.stage.remaining = 1;
        return;
    }
    effects.population_decrements += u32::from(move_actor(actor, ctx));
    if hit == 1 {
        if actor.attacker != actor.stage.parent
            && ctx
                .target(actor.attacker)
                .is_none_or(|t| t.class != 5 || t.model != SOURCE_MODEL)
        {
            actor.target = actor.attacker;
            actor.action = 162;
            effects.feedback = Some((actor.stage.parent, 19));
        }
    } else if let Some(target) = ctx.target(actor.target) {
        follow(actor, ctx, target.position, false, None);
    }
}
fn summoned(actor: &mut SpiderState, ctx: &SpiderContext<'_>, effects: &mut SpiderEffects) {
    let parent = ctx.target(actor.stage.parent);
    if actor.stage.mode == 14 {
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
        if actor.stage.remaining > 0 {
            blink(actor, ctx, effects, false);
        }
        if actor.stage.remaining <= 0 || parent.is_none_or(|t| t.life <= 0) {
            actor.stage.mode = 10;
            actor.target = 0;
            actor.stage.parent = 0;
            actor.stage.index = actor.stage.relation as u8;
            blink(actor, ctx, effects, true);
            return;
        }
        let parent = parent.expect("checked Alliance parent");
        let previous = actor.target;
        actor.target = actor.stage.parent;
        summon_follow(actor, ctx, effects);
        let inherited = if parent.target != 0 {
            parent.target
        } else {
            parent.attacker
        };
        actor.target = if inherited != 0 { inherited } else { previous };
        if let Some(target) = ctx.target(actor.target) {
            if (target.stage.parent == actor.stage.parent && target.stage.mode == 14)
                || target.life <= 0
                || !target.living()
            {
                actor.target = 0;
            } else if distance(actor.position, target.position) < RULES.target_range {
                actor.action = 162;
                effects.feedback = Some((actor.stage.parent, 24));
            }
        }
        return;
    }
    if parent.is_none_or(|t| !t.living()) {
        actor.stage.remaining = 0;
    } else if actor.stage.mode == 13 {
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
    }
    if actor.stage.remaining <= 0 {
        effects.expire = true;
        effects.summon_expire = actor.stage.mode == 13;
        return;
    }
    let parent = parent.expect("checked summoned parent");
    let mut target = ctx.target(actor.target);
    if target.is_some_and(|t| t.life <= 0 || !t.living()) {
        actor.target = 0;
        target = None;
    }
    if target.is_none() && actor.phase & 7 == 0 {
        let mut best = u32::MAX;
        for candidate in ctx
            .class3
            .iter()
            .filter_map(|s| ctx.target(*s))
            .filter(|t| matches!(t.model, 0 | 1) && t.source_id != parent.source_id)
        {
            let d = xy(parent.position, candidate.position);
            if d < best {
                best = d;
                target = Some(candidate);
            }
        }
        actor.target = target.map_or(0, |t| t.slot);
    }
    if let Some(target) = target {
        summon_follow(actor, ctx, effects);
        if actor.phase & 7 == 0 && distance(actor.position, target.position) < RULES.target_range {
            actor.action = 162;
        }
    } else {
        actor.target = actor.stage.parent;
        summon_follow(actor, ctx, effects);
        actor.target = 0;
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(4);
    }
}

pub(super) fn step(
    actor: &mut SpiderState,
    ctx: &mut SpiderContext<'_>,
    effects: &mut SpiderEffects,
) {
    let mode = actor.stage.mode;
    match mode {
        10 => actor.action = 162,
        12 => {
            if let Some(parent) = ctx.target(actor.stage.parent) {
                if parent.life < 0 {
                    actor.control_flags |= super::ENTITY_DRAW_DISABLED_BIT;
                }
                actor.position = parent.position;
                actor.position.z = actor.position.z.max(0);
                actor.yaw = parent.yaw;
                actor.roll = parent.roll;
            } else {
                actor.control_flags |= super::ENTITY_DRAW_DISABLED_BIT;
                actor.position = Position::default();
                actor.yaw = 0;
                actor.roll = 0;
            }
        }
        17 => {
            effects.population_decrements += u32::from(move_actor(actor, ctx));
            if actor.life < 0 {
                actor.control_flags |= super::ENTITY_DRAW_DISABLED_BIT;
            } else if let Some(target) = ctx.target(actor.target).filter(|t| t.living()) {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.stage.mode = 16;
                }
            } else {
                actor.stage.mode = 16;
            }
        }
        13 | 14 | 16 => summoned(actor, ctx, effects),
        1..=9 => {
            let route = ctx
                .stage_runtime
                .as_ref()
                .and_then(|r| r.descriptor(actor.stage.index));
            let point = route.is_some_and(|r| {
                mode == 1 || (mode == 2 && !signed_near(actor.position, r.point, 3072))
            });
            match consume(actor, ctx, !point) {
                2 => actor.action = 164,
                1 => {
                    if ctx
                        .target(actor.attacker)
                        .is_none_or(|t| t.class != 5 || t.model != SOURCE_MODEL)
                    {
                        actor.target = actor.attacker;
                        actor.stage.mode = 10;
                        actor.action = 162;
                    }
                    settle(actor, ctx.terrain);
                }
                _ => {
                    let wander = matches!(mode, 6..=9) || (mode == 2 && !point);
                    // Descriptor bit2 suppresses wandering movement and RNG;
                    // Spider's source descriptor leaves it clear.
                    if !wander || RULES.behavior_flags & 2 == 0 {
                        effects.population_decrements += u32::from(move_actor(actor, ctx));
                        if let Some(route) = route {
                            if point {
                                follow(actor, ctx, route.point, true, None);
                            } else if matches!(mode, 3..=5) && actor.phase & 7 == 0 {
                                if let Some(target) = selected(actor, ctx, route) {
                                    follow(
                                        actor,
                                        ctx,
                                        target.position,
                                        false,
                                        Some(target.half_width),
                                    );
                                } else {
                                    idle(actor);
                                }
                            } else {
                                idle(actor);
                            }
                        }
                    }
                }
            }
            // 1D7C0 and 1D700 perform this scan after their movement helper,
            // including when that helper handled a hit or death.
            if matches!(mode, 3 | 4)
                && actor.phase & 7 == 0
                && let Some(route) = route
            {
                let slot = if route.flags & 2 != 0 {
                    actor.stage.relation
                } else {
                    route.actor
                };
                if let Some(target) = ctx.target(slot) {
                    let target = if mode == 4 {
                        ctx.target(target.target)
                    } else {
                        Some(target)
                    };
                    if let Some(target) = target.filter(|t| t.slot != 0xae02)
                        && distance(actor.position, target.position) <= RULES.target_range
                    {
                        actor.target = target.slot;
                        actor.stage.mode = 10;
                        actor.action = 162;
                    }
                }
            }
            if mode == 2
                && actor.action == 167
                && actor.stage.mode == 2
                && actor.proximity != 0
                && actor.phase % RULES.scan_period == 0
            {
                // sub_1DBF0 uses promoted X subtraction and signed-short Y.
                let mut best = u32::MAX;
                let mut selected = None;
                for t in ctx
                    .class3
                    .iter()
                    .filter_map(|s| ctx.target(*s))
                    .filter(|t| t.flags & 0x20 == 0)
                {
                    let x = i32::from(t.position.x) - i32::from(actor.position.x);
                    let y = i32::from(t.position.y.wrapping_sub(actor.position.y) as i16);
                    let d = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32;
                    if d <= RULES.target_range * RULES.target_range
                        && d < best
                        && super::angle_distance(
                            i32::from(actor.yaw),
                            heading_for_positions(actor.position, t.position),
                        ) < RULES.target_half_angle
                    {
                        best = d;
                        selected = Some(t);
                    }
                }
                if let Some(target) = selected {
                    actor.target = target.slot;
                    actor.stage.mode = 10;
                    actor.action = 162;
                }
            }
        }
        _ => {}
    }
}
