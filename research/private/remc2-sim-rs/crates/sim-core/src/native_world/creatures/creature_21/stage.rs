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
        let d = crate::legacy_sqrt(super::xy(parent.position, target.position));
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
                .target(attacker)
                .is_none_or(|row| row.class != 5 || row.model != SOURCE_MODEL)
        {
            actor.set_target(attacker);
            actor.set_action(170);
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
            actor.set_action(170);
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
