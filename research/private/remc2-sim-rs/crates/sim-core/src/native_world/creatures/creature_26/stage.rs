//! Model26's `sub_29380 -> sub_1D5D0(208)` StageVar callback.

use super::{
    Actor, Context, DRAIN_ACTION, Effects, PREKILL_ACTION, ROAM_ACTION, RuntimeRelation, distance,
    distance_squared_xy, heading_for_positions, move_source, random_roam_heading,
};
use crate::Position;
use crate::native_world::RuntimeStageVariable;

fn live(ctx: &Context<'_>, slot: u16) -> Option<super::Target> {
    ctx.any_target(slot)
        .filter(|target| target.living() && target.flags & 0x400 == 0)
}

fn route<'a>(actor: &Actor<'_>, ctx: &'a Context<'_>) -> Result<&'a RuntimeStageVariable, String> {
    let index = usize::from(actor.row.record[76]);
    ctx.runtime
        .and_then(|runtime| runtime.stage_variables.get(index))
        .ok_or_else(|| format!("model26 action215 has no StageVar at index{index}"))
}

fn point(stage: &RuntimeStageVariable) -> Option<Position> {
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

fn stage_target(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    stage: &RuntimeStageVariable,
) -> Option<super::Target> {
    if stage.flags & 2 == 0 {
        let slot = match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        };
        return live(ctx, slot);
    }

    let current = u16::from_le_bytes([actor.row.record[78], actor.row.record[79]]);
    if current == 0 {
        let model = match stage.relation {
            RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u8,
            _ => return None,
        };
        let nearest = ctx
            .targets
            .iter()
            .filter(|target| target.class == 5 && target.model == model && target.living())
            .min_by_key(|target| distance_squared_xy(actor.row.position, target.position))
            .map_or(0, |target| target.slot);
        actor.row.record[78..80].copy_from_slice(&nearest.to_le_bytes());
    }
    live(
        ctx,
        u16::from_le_bytes([actor.row.record[78], actor.row.record[79]]),
    )
}

fn move_toward(actor: &mut Actor<'_>, ctx: &Context<'_>, destination: Position) {
    move_source(actor, ctx);
    if actor.row.life < 0 {
        return;
    }
    if actor.row.phase & 7 != 0 || actor.row.control_flags & 0x40000 != 0 {
        return;
    }
    actor.set_roll(heading_for_positions(actor.row.position, destination) as i16);
    if actor.row.phase & 63 == 0 {
        random_roam_heading(actor);
    }
    if let Some(peer) = ctx
        .model26
        .iter()
        .filter_map(|slot| ctx.model26_target(*slot))
        .find(|peer| {
            peer.slot != actor.row.index
                && peer.source_id != actor.row.source_id
                && peer.living()
                && actor.row.position.x.abs_diff(peer.position.x)
                    < u16::try_from(super::RULES.collision_half_width.max(0)).unwrap_or(u16::MAX)
                && actor.row.position.y.abs_diff(peer.position.y)
                    < u16::try_from(super::RULES.collision_half_width.max(0)).unwrap_or(u16::MAX)
        })
    {
        actor.set_roll(heading_for_positions(peer.position, actor.row.position) as i16);
    }
}

fn begin_drain(actor: &mut Actor<'_>, target: u16) {
    actor.set_target(target);
    actor.row.set_source_action(DRAIN_ACTION);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DamageOutcome {
    hit: bool,
    fatal: bool,
}

fn take_damage(actor: &mut Actor<'_>, ctx: &Context<'_>) -> DamageOutcome {
    let hit = super::consume_damage(actor, ctx);
    DamageOutcome {
        hit,
        fatal: actor.row.life < 0 || actor.row.action == PREKILL_ACTION,
    }
}

fn finish(actor: &mut Actor<'_>, mode: u8, mut effects: Effects) -> Effects {
    if actor.row.life < 0 {
        if mode == 17 {
            effects.stage_hide = true;
        } else {
            if actor.row.action == super::STAGE_ACTION {
                actor.row.set_source_action(PREKILL_ACTION);
            }
            effects.population_decrements = 1;
        }
    }
    effects
}

fn follow_parent(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let parent = u16::from_le_bytes([actor.row.record[44], actor.row.record[45]]);
    if let Some(parent) = live(ctx, parent) {
        actor.row.set_position(parent.position);
        actor.row.set_yaw(parent.yaw);
        actor.set_roll(parent.roll);
    } else {
        effects.stage_hide = true;
    }
}

pub(super) fn step(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<Effects, String> {
    let stage = route(actor, ctx)?;
    let mode = actor.row.record[77];
    let mut effects = Effects::default();
    match mode {
        1 => {
            let damage = take_damage(actor, ctx);
            if !damage.fatal {
                if damage.hit {
                    if ctx.any_target(actor.attacker()).is_some_and(|target| {
                        target.class != actor.row.class || target.model != actor.row.model
                    }) {
                        actor.set_target(actor.attacker());
                        actor.row.record[77] = 10;
                        actor.row.set_source_action(DRAIN_ACTION);
                    }
                    let mut position = actor.row.position;
                    super::settle(&mut position, ctx.rules, ctx.terrain);
                    actor.row.set_position(position);
                } else if let Some(destination) = point(stage) {
                    move_toward(actor, ctx, destination);
                }
            }
        }
        2 => {
            if take_damage(actor, ctx).fatal {
                return Ok(finish(actor, mode, effects));
            }
            let destination = point(stage);
            if let Some(destination) = destination {
                let dx = actor.row.position.x.abs_diff(destination.x);
                let dy = actor.row.position.y.abs_diff(destination.y);
                if dx > 3072 || dy > 3072 {
                    move_toward(actor, ctx, destination);
                } else {
                    move_source(actor, ctx);
                    if actor.row.life >= 0
                        && actor.row.phase & 15 == 0
                        && let Some(target) = stage_target(actor, ctx, stage)
                    {
                        begin_drain(actor, target.slot);
                    }
                }
            }
        }
        3..=5 => {
            if take_damage(actor, ctx).fatal {
                return Ok(finish(actor, mode, effects));
            }
            move_source(actor, ctx);
            if actor.row.life >= 0
                && actor.row.phase & 7 == 0
                && let Some(target) = stage_target(actor, ctx, stage)
            {
                actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                if distance(actor.row.position, target.position) <= super::RULES.target_range {
                    begin_drain(actor, target.slot);
                }
            }
        }
        6..=9 => {
            if take_damage(actor, ctx).fatal {
                return Ok(finish(actor, mode, effects));
            }
            move_source(actor, ctx);
            if actor.row.life >= 0 && actor.row.phase & 15 == 0 {
                if let Some(target) = stage_target(actor, ctx, stage) {
                    actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                } else if let Some(destination) = point(stage) {
                    actor.set_roll(heading_for_positions(actor.row.position, destination) as i16);
                }
            }
        }
        10 => {
            let target = live(ctx, actor.target()).or_else(|| stage_target(actor, ctx, stage));
            if let Some(target) = target {
                begin_drain(actor, target.slot);
            } else {
                actor.set_target(0);
                actor.row.set_source_action(ROAM_ACTION);
            }
        }
        12 => follow_parent(actor, ctx, &mut effects),
        13 | 16 => {
            if take_damage(actor, ctx).fatal {
                return Ok(finish(actor, mode, effects));
            }
            let mut remaining = i16::from_le_bytes([actor.row.record[50], actor.row.record[51]]);
            let parent_slot = u16::from_le_bytes([actor.row.record[44], actor.row.record[45]]);
            let parent = live(ctx, parent_slot);
            if parent.is_none() {
                remaining = 0;
                actor.row.record[50..52].copy_from_slice(&remaining.to_le_bytes());
            } else if actor.row.record[77] == 13 {
                remaining = remaining.wrapping_sub(1);
                actor.row.record[50..52].copy_from_slice(&remaining.to_le_bytes());
            }
            if remaining <= 0 {
                effects.stage_hide = true;
                effects.stage_mana_expiry = actor.row.record[77] == 13;
            } else if let Some(target) = live(ctx, actor.target()).or(parent) {
                actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                move_source(actor, ctx);
            } else {
                effects.stage_hide = true;
            }
        }
        14 => {
            if take_damage(actor, ctx).fatal {
                return Ok(finish(actor, mode, effects));
            }
            let remaining =
                i16::from_le_bytes([actor.row.record[50], actor.row.record[51]]).wrapping_sub(1);
            actor.row.record[50..52].copy_from_slice(&remaining.to_le_bytes());
            let parent = u16::from_le_bytes([actor.row.record[44], actor.row.record[45]]);
            if remaining <= 0 || live(ctx, parent).is_none() {
                actor.row.record[76] = actor.row.record[78];
                actor.row.record[77] = 10;
                actor.row.record[44..46].fill(0);
                actor.set_target(0);
            } else if let Some(target) = live(ctx, actor.target()) {
                actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                move_source(actor, ctx);
            }
        }
        17 => {
            move_source(actor, ctx);
            if actor.row.life < 0 {
                effects.stage_hide = true;
            } else if let Some(target) = live(ctx, actor.target()) {
                actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                let speed = actor.row.actual_speed.wrapping_sub(8);
                actor.row.set_actual_speed(speed);
                if speed <= 16 {
                    actor.row.record[77] = 16;
                }
            } else {
                actor.row.record[77] = 16;
            }
        }
        // sub_1D5D0's default branch leaves the actor unchanged for modes with
        // no registered source callback (including11 and15).
        _ => {}
    }
    Ok(finish(actor, mode, effects))
}
