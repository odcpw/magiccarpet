//! Model-17 `sub_24DF0 -> sub_1D5D0(136)` StageVar callback.

use super::{
    Creature17Context, Creature17Effects, Creature17State, RULES, Target, consume, jitter,
    move_actor,
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
        return live(ctx, slot);
    }
    if actor.route_relation == 0 {
        let model = match stage.relation {
            RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u8,
            _ => 0,
        };
        actor.route_relation = ctx
            .targets
            .iter()
            .filter(|target| target.class == 5 && target.model == model && target.alive())
            .min_by_key(|target| super::xy_squared(actor.position, target.position))
            .map_or(0, |target| target.slot);
    }
    live(ctx, actor.route_relation)
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

fn damage(actor: &mut Creature17State, ctx: &Creature17Context<'_>) -> bool {
    match consume(actor, ctx) {
        2 => {
            actor.action = 140;
            true
        }
        1 => {
            actor.target = actor.attacker;
            actor.route_mode = 10;
            actor.action = 138;
            true
        }
        _ => false,
    }
}

fn roam_to(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
    destination: Position,
) {
    move_actor(actor, ctx.terrain, effects);
    if actor.phase & 7 == 0 {
        actor.roll = heading_for_positions(actor.position, destination) as i16;
        if actor.phase & 63 == 0 {
            jitter(actor);
        }
        if let Some(peer) = ctx
            .model17
            .iter()
            .filter_map(|slot| ctx.target(*slot))
            .find(|peer| {
                peer.slot != actor.slot
                    && peer.source_id != actor.source_id
                    && peer.alive()
                    && actor.position.x.abs_diff(peer.position.x) < actor.half_width.max(0) as u16
                    && actor.position.y.abs_diff(peer.position.y) < actor.half_width.max(0) as u16
            })
        {
            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
        }
    }
}

fn follow_parent(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    if let Some(parent) = live(ctx, actor.parent) {
        actor.position = parent.position;
        actor.yaw = parent.yaw;
        actor.roll = parent.yaw;
    } else {
        effects.route_hide = true;
    }
}

pub(super) fn step(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    match actor.route_mode {
        1 => {
            if damage(actor, ctx) {
                return;
            }
            if let Some(destination) = point(actor, ctx) {
                roam_to(actor, ctx, effects, destination);
            }
        }
        2 => {
            if damage(actor, ctx) {
                return;
            }
            if let Some(destination) = point(actor, ctx) {
                let dx = actor
                    .position
                    .x
                    .wrapping_sub(destination.x)
                    .cast_signed()
                    .unsigned_abs();
                let dy = actor
                    .position
                    .y
                    .wrapping_sub(destination.y)
                    .cast_signed()
                    .unsigned_abs();
                if dx > 3072 || dy > 3072 {
                    roam_to(actor, ctx, effects, destination);
                } else {
                    move_actor(actor, ctx.terrain, effects);
                    if actor.phase & 15 == 0 {
                        if let Some(target) = stage_target(actor, ctx) {
                            actor.target = target.slot;
                            actor.route_mode = 10;
                            actor.action = 138;
                        }
                    }
                }
            }
        }
        3..=5 => {
            if damage(actor, ctx) {
                return;
            }
            move_actor(actor, ctx.terrain, effects);
            if actor.phase & 7 == 0
                && let Some(target) = stage_target(actor, ctx)
            {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                if crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
                    actor.position,
                    target.position,
                )) <= RULES.target_range
                {
                    actor.target = target.slot;
                    actor.route_mode = 10;
                    actor.action = 138;
                }
            }
        }
        6..=9 => {
            if damage(actor, ctx) {
                return;
            }
            move_actor(actor, ctx.terrain, effects);
            if actor.phase & 15 == 0 {
                if let Some(target) = stage_target(actor, ctx) {
                    actor.roll = heading_for_positions(actor.position, target.position) as i16;
                } else if let Some(destination) = point(actor, ctx) {
                    actor.roll = heading_for_positions(actor.position, destination) as i16;
                }
            }
        }
        10 => {
            if live(ctx, actor.target).is_none()
                && let Some(target) = stage_target(actor, ctx)
            {
                actor.target = target.slot;
            }
            if live(ctx, actor.target).is_some() {
                actor.action = 138;
            } else {
                actor.target = 0;
                actor.action = 137;
            }
        }
        12 => follow_parent(actor, ctx, effects),
        13 | 16 => {
            if actor.parent == 0 || live(ctx, actor.parent).is_none() {
                actor.route_remaining = 0;
            } else {
                actor.route_remaining = actor.route_remaining.wrapping_sub(1);
                let follow = live(ctx, actor.target).map_or_else(
                    || live(ctx, actor.parent).map(|parent| parent.position),
                    |target| Some(target.position),
                );
                if let Some(position) = follow {
                    actor.roll = heading_for_positions(actor.position, position) as i16;
                    move_actor(actor, ctx.terrain, effects);
                }
            }
            if actor.route_remaining <= 0 {
                effects.route_hide = true;
            }
        }
        14 => {
            actor.route_remaining = actor.route_remaining.wrapping_sub(1);
            if actor.route_remaining <= 0 {
                actor.route_mode = 10;
                actor.route_index = actor.route_relation as u8;
                actor.route_relation = 0;
                actor.parent = 0;
                actor.target = 0;
                actor.action = 138;
            } else if let Some(target) = stage_target(actor, ctx) {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                move_actor(actor, ctx.terrain, effects);
            } else {
                effects.route_hide = true;
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
