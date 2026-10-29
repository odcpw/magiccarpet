//! Model-18 `sub_25550 -> sub_1D5D0(144)` StageVar callback.

use super::{
    Creature18Context, Creature18Effects, Creature18State, RULES, SOURCE_CLASS, SOURCE_MODEL,
    Target,
};
use crate::native_world::RuntimeRelation;
use crate::{Position, heading_for_positions};

fn live_target(ctx: &Creature18Context<'_>, slot: u16) -> Option<Target> {
    ctx.target(slot)
        .filter(|target| target.life >= 0 && target.flags & 0x400 == 0)
}

fn route_target(actor: &mut Creature18State, ctx: &Creature18Context<'_>) -> Option<Target> {
    let stage = ctx
        .runtime
        .stage_variables
        .get(usize::from(actor.route_index))?;
    if stage.flags as u8 & 2 == 0 {
        let slot = match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        };
        return ctx.target(slot);
    }

    if actor.route_relation == 0 && stage.flags as u8 & 1 != 0 {
        actor.route_relation = ctx
            .model18
            .iter()
            .filter_map(|slot| ctx.target(*slot))
            .find(|peer| peer.route_index == actor.route_index && peer.route_relation != 0)
            .map_or(0, |peer| peer.route_relation);
    }
    if actor.route_relation == 0 {
        let model = match stage.relation {
            RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u8,
            _ => 0,
        };
        actor.route_relation = ctx
            .families
            .get(usize::from(model))
            .into_iter()
            .flatten()
            .filter_map(|slot| ctx.target(*slot))
            .min_by_key(|target| {
                let dx = i32::from(target.position.x.wrapping_sub(actor.position.x) as i16);
                let dy = i32::from(target.position.y.wrapping_sub(actor.position.y) as i16);
                dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
            })
            .map_or(0, |target| target.slot);
    }
    ctx.target(actor.route_relation)
}

fn steer(actor: &mut Creature18State, ctx: &Creature18Context<'_>, point: Position) {
    if actor.phase & 7 != 0 {
        return;
    }
    if actor.control_flags & 0x40000 == 0 {
        actor.roll = heading_for_positions(actor.position, point) as i16;
        if actor.phase & 63 == 0 {
            let first = super::random(actor);
            let second = super::random(actor);
            let sign = 2 * (i32::from(first % 157) / 79) - 1;
            actor.roll =
                (i32::from(actor.roll) + i32::from((second & 255) + 85) * sign) as i16 & 2047;
        }
    }
    if let Some(peer) = ctx
        .model18
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .find(|peer| {
            peer.source_id != actor.source_id
                && (i32::from(actor.position.x) - i32::from(peer.position.x)).abs()
                    < i32::from(actor.half_width)
                && (i32::from(actor.position.y) - i32::from(peer.position.y)).abs()
                    < i32::from(actor.half_width)
        })
    {
        actor.roll = heading_for_positions(peer.position, actor.position) as i16;
    }
}

fn route_hit(actor: &mut Creature18State, ctx: &Creature18Context<'_>) -> u8 {
    let hit = super::consume(actor, ctx);
    if hit == 2 {
        actor.action = 148;
    } else if hit == 1 {
        if ctx
            .target(actor.attacker)
            .is_none_or(|source| source.class != SOURCE_CLASS || source.model != SOURCE_MODEL)
        {
            actor.target = actor.attacker;
            actor.route_mode = 10;
            actor.action = 146;
        }
        super::settle(actor, ctx.terrain);
    }
    hit
}

fn idle_turn(actor: &mut Creature18State) {
    if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
        let rng = super::random(actor);
        actor.roll = (actor.roll + (rng % 113 + 142) as i16) & 2047;
    }
}

fn guard_target(actor: &Creature18State, ctx: &Creature18Context<'_>) -> Option<Target> {
    let range_squared = RULES.target_range.wrapping_mul(RULES.target_range);
    ctx.class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.flags & 0x20 == 0)
        .filter(|target| {
            super::xy_squared(actor.position, target.position) <= range_squared
                && crate::angle_distance(
                    i32::from(actor.yaw),
                    heading_for_positions(actor.position, target.position),
                ) < RULES.target_half_angle
        })
        .min_by_key(|target| super::xy_squared(actor.position, target.position))
}

fn route_behavior(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    mode: u8,
    effects: &mut Creature18Effects,
) {
    let Some(descriptor) = ctx
        .runtime
        .stage_variables
        .get(usize::from(actor.route_index))
    else {
        return;
    };
    let scalar = match descriptor.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value,
        _ => 0,
    };
    let point = Position {
        x: scalar as u16,
        y: (scalar >> 16) as u16,
        z: 0,
    };
    let near_point = (i32::from(actor.position.x as i16) - i32::from(point.x as i16)).abs() <= 3072
        && (i32::from(actor.position.y as i16) - i32::from(point.y as i16)).abs() <= 3072;
    let point_helper = mode == 1 || (mode == 2 && !near_point);
    if route_hit(actor, ctx) == 0 {
        if point_helper {
            super::move_ground(actor, ctx.terrain, effects);
            steer(actor, ctx, point);
        } else if mode <= 5 || RULES.behavior_flags & 2 == 0 {
            super::move_ground(actor, ctx.terrain, effects);
            if matches!(mode, 3..=5) && actor.phase & 7 == 0 {
                if let Some(target) = route_target(actor, ctx) {
                    steer(actor, ctx, target.position);
                    let range = i32::from(actor.half_width) + i32::from(target.half_width);
                    if (i32::from(actor.position.x) - i32::from(target.position.x)).abs() < range
                        && (i32::from(actor.position.y) - i32::from(target.position.y)).abs()
                            < range
                    {
                        actor.roll = heading_for_positions(target.position, actor.position) as i16;
                    }
                } else {
                    idle_turn(actor);
                }
            } else if !matches!(mode, 3..=5) {
                idle_turn(actor);
            }
        }
    }
    if matches!(mode, 3 | 4) && actor.phase & 7 == 0 {
        let slot = if descriptor.flags as u8 & 2 != 0 {
            actor.route_relation
        } else {
            match descriptor.relation {
                RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
                _ => 0,
            }
        };
        if let Some(target) = ctx.target(slot) {
            let selected = if mode == 4 {
                ctx.target(target.target)
            } else {
                Some(target)
            };
            if let Some(selected) = selected.filter(|target| target.slot != 0xae02)
                && super::source_distance_3d(actor.position, selected.position)
                    <= RULES.target_range
            {
                actor.target = selected.slot;
                actor.route_mode = 10;
                actor.action = 146;
            }
        }
    }
    if mode == 2
        && actor.action == 151
        && actor.route_mode == 2
        && actor.proximity != 0
        && actor.phase % RULES.source_scan_period == 0
        && let Some(target) = guard_target(actor, ctx)
    {
        actor.target = target.slot;
        actor.route_mode = 10;
        actor.action = 146;
    }
}

// sub_1E9C0 and sub_1ED30 both update Alliance blinking. The latter also
// runs during firing, so its countdown can advance independently of stage 14.
fn alliance_blink(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
    clear: bool,
) {
    let phase = if clear {
        0
    } else if actor.route_remaining <= actor.route_duration.wrapping_sub(16)
        && actor.route_remaining >= 16
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
    let flags = match phase {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    actor.control_flags = (actor.control_flags & !0x0600_0000) | flags;
    let mut child = actor.linked_child;
    for _ in 0..=ctx.targets.len() {
        let Some(row) = ctx.target(child) else { break };
        effects.linked_blink_flags.push((row.slot, flags));
        child = row.linked_child;
    }
}

pub(super) fn alliance_target(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
    target: Option<Target>,
) -> Option<Target> {
    if actor.route_mode != 14 {
        return target;
    }
    alliance_blink(actor, ctx, effects, false);
    actor.route_remaining = actor.route_remaining.wrapping_sub(1);
    if actor.route_remaining <= 0 {
        return None;
    }
    let Some(parent) = ctx.target(actor.parent) else {
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

pub(super) fn step(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
) {
    let mode = actor.route_mode;
    match mode {
        10 => actor.action = 146,
        12 => {
            if let Some(parent) = ctx.target(actor.parent) {
                if parent.life < 0 {
                    effects.route_hide = true;
                }
                actor.position = parent.position;
                actor.yaw = parent.yaw;
                actor.roll = parent.roll;
            } else {
                effects.route_hide = true;
                actor.position = Position::default();
                actor.yaw = 0;
                actor.roll = 0;
            }
        }
        13 | 16 => {
            let parent = live_target(ctx, actor.parent);
            if parent.is_none() {
                actor.route_remaining = 0;
            } else if actor.route_mode == 13 {
                actor.route_remaining = actor.route_remaining.wrapping_sub(1);
            }
            if actor.route_remaining <= 0 {
                effects.expire = true;
                effects.summon_expire = actor.route_mode == 13;
                return;
            }
            let parent = parent.expect("summon parent was checked");
            let mut target = live_target(ctx, actor.target).filter(|target| target.life > 0);
            if target.is_none() {
                actor.target = 0;
                if actor.phase & 7 == 0 {
                    target = ctx
                        .class3
                        .iter()
                        .filter_map(|slot| ctx.target(*slot))
                        .filter(|candidate| {
                            matches!(candidate.model, 0 | 1)
                                && candidate.source_id != parent.source_id
                        })
                        .min_by_key(|candidate| {
                            super::xy_squared(parent.position, candidate.position)
                        });
                    actor.target = target.map_or(0, |candidate| candidate.slot);
                }
            }
            let destination = target.map_or(parent.position, |candidate| candidate.position);
            let hit = super::consume(actor, ctx);
            if hit == 2 {
                actor.route_remaining = 1;
            } else {
                super::move_ground(actor, ctx.terrain, effects);
                if hit == 1
                    && actor.attacker != actor.parent
                    && ctx.target(actor.attacker).is_none_or(|attacker| {
                        attacker.class != SOURCE_CLASS || attacker.model != SOURCE_MODEL
                    })
                {
                    actor.target = actor.attacker;
                    actor.action = 146;
                    effects.feedback = Some((actor.parent, 19));
                } else if hit == 0 && actor.phase & 7 == 0 {
                    if actor.control_flags & 0x40000 == 0 {
                        actor.roll = heading_for_positions(actor.position, destination) as i16;
                        if actor.phase & 63 == 0 {
                            let first = super::random(actor);
                            let second = super::random(actor);
                            let sign = 2 * (i32::from(first % 157) / 79) - 1;
                            actor.roll = (i32::from(actor.roll)
                                + i32::from((second & 255) + 85) * sign)
                                as i16
                                & 2047;
                        }
                    }
                    if let Some(peer) = ctx
                        .model18
                        .iter()
                        .filter_map(|slot| ctx.target(*slot))
                        .find(|peer| {
                            peer.source_id != actor.source_id
                                && (i32::from(actor.position.x) - i32::from(peer.position.x)).abs()
                                    < i32::from(actor.half_width)
                                && (i32::from(actor.position.y) - i32::from(peer.position.y)).abs()
                                    < i32::from(actor.half_width)
                        })
                    {
                        actor.roll = heading_for_positions(peer.position, actor.position) as i16;
                    }
                }
            }
            if let Some(target) = target {
                if actor.phase & 7 == 0
                    && super::source_distance_3d(actor.position, target.position)
                        < RULES.target_range
                {
                    actor.action = 146;
                }
            } else {
                actor.target = 0;
                actor.route_remaining = actor.route_remaining.wrapping_sub(4);
            }
        }
        14 => {
            actor.route_remaining = actor.route_remaining.wrapping_sub(1);
            let parent = if actor.route_remaining > 0 {
                alliance_blink(actor, ctx, effects, false);
                ctx.target(actor.parent).filter(|parent| parent.life > 0)
            } else {
                None
            };
            let Some(parent) = parent else {
                actor.route_mode = 10;
                actor.target = 0;
                actor.parent = 0;
                actor.route_index = actor.route_relation as u8;
                alliance_blink(actor, ctx, effects, true);
                return;
            };
            let saved_target = actor.target;
            actor.target = actor.parent;
            let hit = super::consume(actor, ctx);
            if hit == 2 {
                actor.route_remaining = 1;
            } else {
                super::move_ground(actor, ctx.terrain, effects);
                if hit == 1 {
                    if actor.attacker != actor.parent
                        && ctx.target(actor.attacker).is_none_or(|attacker| {
                            attacker.class != SOURCE_CLASS || attacker.model != SOURCE_MODEL
                        })
                    {
                        actor.target = actor.attacker;
                        actor.action = 146;
                        effects.feedback = Some((actor.parent, 19));
                    }
                } else if actor.phase & 7 == 0 {
                    if actor.control_flags & 0x40000 == 0 {
                        actor.roll = heading_for_positions(actor.position, parent.position) as i16;
                        if actor.phase & 63 == 0 {
                            let first = super::random(actor);
                            let second = super::random(actor);
                            let sign = 2 * (i32::from(first % 157) / 79) - 1;
                            actor.roll = (i32::from(actor.roll)
                                + i32::from((second & 255) + 85) * sign)
                                as i16
                                & 2047;
                        }
                    }
                    if let Some(peer) = ctx
                        .model18
                        .iter()
                        .filter_map(|slot| ctx.target(*slot))
                        .find(|peer| {
                            peer.source_id != actor.source_id
                                && (i32::from(actor.position.x) - i32::from(peer.position.x)).abs()
                                    < i32::from(actor.half_width)
                                && (i32::from(actor.position.y) - i32::from(peer.position.y)).abs()
                                    < i32::from(actor.half_width)
                        })
                    {
                        actor.roll = heading_for_positions(peer.position, actor.position) as i16;
                    }
                }
            }
            let selected = if parent.target != 0 {
                parent.target
            } else {
                parent.attacker
            };
            actor.target = if selected != 0 {
                selected
            } else {
                saved_target
            };
            if let Some(target) = ctx.target(actor.target) {
                if (target.parent == actor.parent && target.route_mode == 14)
                    || target.life <= 0
                    || target.flags & 0x400 != 0
                {
                    actor.target = 0;
                } else if super::source_distance_3d(actor.position, target.position)
                    < RULES.target_range
                {
                    actor.action = 146;
                    effects.earlier_feedback = effects.feedback.take();
                    effects.feedback = Some((actor.parent, 24));
                }
            }
        }
        17 => {
            super::move_ground(actor, ctx.terrain, effects);
            if actor.life < 0 {
                effects.route_hide = true;
            } else if let Some(target) = live_target(ctx, actor.target) {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.route_mode = 16;
                }
            } else {
                actor.route_mode = 16;
            }
        }
        1..=9 => route_behavior(actor, ctx, mode, effects),
        _ => {}
    }
}
