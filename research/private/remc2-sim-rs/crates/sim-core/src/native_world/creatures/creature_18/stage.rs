//! Model-18 `sub_25550 -> sub_1D5D0(144)` StageVar callback.

use super::{
    Creature18Context, Creature18Effects, Creature18State, RULES, SOURCE_CLASS, SOURCE_MODEL,
    Target, source_damage,
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
        return live_target(ctx, slot);
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
            .targets
            .iter()
            .filter(|target| target.class == 5 && target.model == model && target.life >= 0)
            .min_by_key(|target| {
                let dx = i32::from(target.position.x.wrapping_sub(actor.position.x) as i16);
                let dy = i32::from(target.position.y.wrapping_sub(actor.position.y) as i16);
                dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
            })
            .map_or(0, |target| target.slot);
    }
    live_target(ctx, actor.route_relation)
}

fn move_toward(
    actor: &mut Creature18State,
    point: Position,
    terrain: &crate::native_world::SessionTerrain,
    effects: &mut Creature18Effects,
) {
    actor.roll = heading_for_positions(actor.position, point) as i16;
    super::move_ground(actor, terrain, effects);
}

pub(super) fn step(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
) {
    match actor.route_mode {
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
                            crate::legacy_sqrt(super::xy_squared(
                                parent.position,
                                candidate.position,
                            ))
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
            if let Some(parent) = live_target(ctx, actor.parent) {
                actor.route_remaining = actor.route_remaining.wrapping_sub(1);
                if actor.route_remaining <= 0 {
                    actor.route_mode = 10;
                    actor.target = 0;
                    actor.parent = 0;
                    actor.route_index = actor.route_relation as u8;
                    return;
                }
                let target = live_target(ctx, actor.target);
                let follow = target.map_or(parent.position, |target| target.position);
                move_toward(actor, follow, ctx.terrain, effects);
                if target.is_none() {
                    actor.route_remaining = actor.route_remaining.wrapping_sub(4);
                }
            } else {
                actor.route_remaining = 0;
            }
            if actor.route_remaining <= 0 {
                effects.route_hide = true;
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
        1..=9 => match source_damage(actor, ctx) {
            2 => {
                actor.action = 148;
                return;
            }
            1 => {
                actor.route_mode = 10;
                actor.action = 146;
                actor.target = actor.attacker;
            }
            _ => {
                let descriptor = ctx
                    .runtime
                    .stage_variables
                    .get(usize::from(actor.route_index));
                let scalar = descriptor.map_or(0, |row| match row.relation {
                    RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value,
                    _ => 0,
                });
                let point = Position {
                    x: scalar as u16,
                    y: (scalar >> 16) as u16,
                    z: 0,
                };
                if matches!(actor.route_mode, 3 | 4 | 5) && actor.phase & 7 == 0 {
                    if let Some(target) = route_target(actor, ctx) {
                        move_toward(actor, target.position, ctx.terrain, effects);
                        if crate::legacy_sqrt(
                            crate::native_world::level2_wrapping_source_distance_3d(
                                actor.position,
                                target.position,
                            ),
                        ) <= RULES.target_range
                        {
                            actor.target = target.slot;
                            actor.route_mode = 10;
                            actor.action = 146;
                        }
                    }
                } else {
                    move_toward(actor, point, ctx.terrain, effects);
                    let radius = if actor.route_mode == 1 { 2048 } else { 3072 };
                    let close = actor
                        .position
                        .x
                        .wrapping_sub(point.x)
                        .cast_signed()
                        .unsigned_abs()
                        <= radius
                        && actor
                            .position
                            .y
                            .wrapping_sub(point.y)
                            .cast_signed()
                            .unsigned_abs()
                            <= radius;
                    if close && actor.route_mode == 1 {
                        actor.route_mode = 10;
                        actor.action = 146;
                    }
                }
            }
        },
        _ => {}
    }
}
