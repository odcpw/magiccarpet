//! Model21 StageVar callbacks, including the real source boss minion route.

use super::{
    Actor, Context, Effects, RULES, SOURCE_MODEL, Target, distance, jump, living, move_actor,
};
use crate::heading_for_positions;
use crate::native_world::creatures::stage_route::Route;

fn prepare_route(actor: &mut Actor<'_>, ctx: &mut Context<'_>, base: u8) {
    let targets = ctx.targets;
    let mut action = actor.row.action;
    let mut index = actor.row.record[76] as i8;
    let mut mode = actor.row.record[77] as i8;
    let mut relation = actor.stage_relation();
    let route_model = base / 8;
    let mut route = Route {
        model: route_model,
        position: actor.row.position,
        action: &mut action,
        index: &mut index,
        mode: &mut mode,
        relation: &mut relation,
    };
    route.prepare(ctx.runtime, |slot| {
        targets
            .iter()
            .find(|target| slot != 0 && target.slot == slot)
            .is_none_or(|target| target.life < 0 || target.control_flags & 0x400 != 0)
    });
    actor.set_action(action);
    actor.row.record[76] = index as u8;
    actor.row.record[77] = mode as u8;
    actor.put(78, relation);
}

pub(super) fn base_route(actor: &mut Actor<'_>, ctx: &mut Context<'_>, base: u8) {
    prepare_route(actor, ctx, base);
}

fn stage_target(actor: &mut Actor<'_>, ctx: &Context<'_>, parent: Target) -> Option<Target> {
    let current = actor.target();
    if let Some(target) = ctx.target(current).filter(|target| living(*target)) {
        return Some(target);
    }
    let mut best = None;
    let mut shortest = u32::MAX;
    for target in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| living(*target) && target.control_flags & 0x20 == 0)
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
    actor
        .row
        .set_source_roll(heading_for_positions(actor.row.position, target.position) as i16);
    move_actor(actor, ctx, effects);
    jump(actor, ctx, effects);
}

/// Source sub_1E320; after reaching the target it falls through to the
/// persistent model21 StageVar16 follow/attack state.
fn stage17(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    move_actor(actor, ctx, effects);
    let Some(target) = ctx.target(actor.target()).filter(|target| living(*target)) else {
        actor.row.set_actual_speed(RULES.attack_ground_speed);
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

fn stage16(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    match super::consume(actor, ctx, true) {
        2 => {
            actor.set_action(172);
            return;
        }
        1 => {
            actor.set_target(actor.attacker());
            actor.set_action(170);
            return;
        }
        _ => {}
    }
    let Some(parent) = ctx
        .target(actor.stage_parent())
        .filter(|parent| living(*parent))
    else {
        actor.row.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
        effects.expire = true;
        return;
    };
    let remaining = actor.word(50) as i16;
    if remaining <= 0 {
        effects.expire = true;
        effects.summon_expire = true;
        return;
    }
    if let Some(target) = stage_target(actor, ctx, parent) {
        summon_follow(actor, ctx, target, effects);
        if actor.row.phase & 7 == 0
            && distance(actor.row.position, target.position) < RULES.target_range
        {
            actor.set_action(170);
        }
    } else {
        actor.set_target(parent.slot);
        summon_follow(actor, ctx, parent, effects);
        actor.set_target(0);
        actor.put(50, remaining.wrapping_sub(4) as u16);
    }
}

pub(super) fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>, effects: &mut Effects) {
    match actor.stage_mode() {
        17 => stage17(actor, ctx, effects),
        16 => stage16(actor, ctx, effects),
        13 | 14 => {
            prepare_route(actor, ctx, 168);
            if actor.row.action == 175 {
                jump(actor, ctx, effects);
            }
        }
        mode if (1..=10).contains(&mode) => {
            prepare_route(actor, ctx, 168);
            if actor.row.action == 175 {
                jump(actor, ctx, effects);
            }
        }
        _ => {}
    }
}
