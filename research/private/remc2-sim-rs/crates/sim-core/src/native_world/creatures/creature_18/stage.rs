//! Model-18 `sub_25550 -> sub_1D5D0(144)` StageVar callback.

use super::{
    Creature18Context, Creature18Effects, Creature18State, RULES, SOURCE_MODEL, Target,
    source_damage,
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
) {
    actor.roll = heading_for_positions(actor.position, point) as i16;
    super::move_ground(actor, terrain);
}

pub(super) fn step(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
) {
    match actor.route_mode {
        10 => actor.action = 146,
        12 => {
            if let Some(parent) = live_target(ctx, actor.parent) {
                if parent.life < 0 {
                    effects.route_hide = true;
                }
                actor.position = parent.position;
                actor.yaw = parent.yaw;
                actor.roll = parent.roll;
            } else {
                effects.route_hide = true;
            }
        }
        13 | 14 | 16 => {
            if let Some(parent) = live_target(ctx, actor.parent) {
                if actor.route_mode == 13 {
                    actor.route_remaining = actor.route_remaining.wrapping_sub(1);
                }
                if actor.route_mode == 14 {
                    actor.route_remaining = actor.route_remaining.wrapping_sub(1);
                    if actor.route_remaining <= 0 {
                        actor.route_mode = 10;
                        actor.target = 0;
                        actor.parent = 0;
                        actor.route_index = actor.route_relation as u8;
                        return;
                    }
                }
                let target = live_target(ctx, actor.target);
                let follow = target.map_or(parent.position, |target| target.position);
                move_toward(actor, follow, ctx.terrain);
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
            super::move_ground(actor, ctx.terrain);
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
                        move_toward(actor, target.position, ctx.terrain);
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
                    move_toward(actor, point, ctx.terrain);
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
