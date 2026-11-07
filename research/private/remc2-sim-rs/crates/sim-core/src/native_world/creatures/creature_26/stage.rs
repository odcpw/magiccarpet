//! Model26's sub_29380 -> sub_1D5D0(208) StageVar callback.

use super::{Actor, Context, DRAIN_ACTION, Effects, PREKILL_ACTION, RuntimeRelation, Target};
use crate::native_world::{RuntimeStageVariable, level2_next_rng16};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

fn word(actor: &Actor<'_>, at: usize) -> u16 {
    super::word(actor.row, at)
}

fn put(actor: &mut Actor<'_>, at: usize, value: u16) {
    super::put(actor.row, at, value);
}

fn required<T>(target: Target, value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| {
        format!(
            "model26 current source target {} lacks {field}",
            target.slot
        )
    })
}

fn current(actor: &Actor<'_>, ctx: &Context<'_>, slot: u16) -> Option<Target> {
    ctx.actor_target(actor, slot)
}

fn route<'a>(actor: &Actor<'_>, ctx: &'a Context<'_>) -> Result<&'a RuntimeStageVariable, String> {
    let index = usize::from(actor.row.record[76]);
    ctx.runtime
        .and_then(|runtime| runtime.stage_variables.get(index))
        .ok_or_else(|| format!("model26 action215 has no StageVar at index{index}"))
}

fn point(stage: &RuntimeStageVariable) -> Result<Position, String> {
    let packed = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value,
        _ => return Err("model26 point route has no source coordinate".to_owned()),
    };
    Ok(Position {
        x: packed as u16,
        y: (packed >> 16) as u16,
        z: 0,
    })
}

fn attack(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<(), String> {
    if ctx.rules.behavior_flags & 8 != 0 {
        return Err("model26 configured flee flag selects unsupported source action214".to_owned());
    }
    actor.row.set_source_action(DRAIN_ACTION);
    Ok(())
}

/// Generic StageVar helpers consume damage without changing word96 merely
/// because a hit arrived. Their own family/parent checks decide the reaction.
fn damage(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<u8, String> {
    let target = actor.target();
    let hit = super::consume_damage(actor, ctx)?;
    actor.set_target(target);
    Ok(if actor.row.life < 0 { 2 } else { u8::from(hit) })
}

fn move_actor(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    effects.population_decrements += u32::from(super::move_source(actor, ctx));
}

fn relax(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    let mut position = actor.row.position;
    super::settle(&mut position, ctx.rules, ctx.terrain);
    actor.row.set_position(position);
}

fn different_family(actor: &Actor<'_>, ctx: &Context<'_>, slot: u16) -> bool {
    let target = if slot == 0 {
        ctx.sentinel
    } else {
        current(actor, ctx, slot)
    };
    target.is_none_or(|target| (target.class, target.model) != (actor.row.class, actor.row.model))
}

fn hit_reaction(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<(), String> {
    let attacker = actor.attacker();
    if different_family(actor, ctx, attacker) {
        actor.set_target(attacker);
        actor.row.record[77] = 10;
        attack(actor, ctx)?;
    }
    relax(actor, ctx);
    Ok(())
}

fn random_wander(actor: &mut Actor<'_>) {
    let rng = level2_next_rng16(actor.row.entity_rng);
    actor.row.set_entity_rng(rng);
    actor.set_roll(actor.roll().wrapping_add((rng % 113 + 142) as i16) & 2047);
}

/// sub_1E1C0. Modes6..9 never read the StageVar table.
fn wander(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 if ctx.rules.behavior_flags & 2 == 0 => {
            move_actor(actor, ctx, effects);
            if actor.row.phase & 15 == 0 && actor.row.control_flags & 0x40000 == 0 {
                random_wander(actor);
            }
        }
        1 => hit_reaction(actor, ctx)?,
        2 => actor.row.set_source_action(PREKILL_ACTION),
        _ => {}
    }
    Ok(())
}

/// sub_1DDA0 uses signed coordinate promotion for its peer overlap.
fn point_step(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 => {
            move_actor(actor, ctx, effects);
            if actor.row.phase & 7 == 0 {
                let destination = point(route(actor, ctx)?)?;
                if actor.row.control_flags & 0x40000 == 0 {
                    actor.set_roll(heading_for_positions(actor.row.position, destination) as i16);
                    if actor.row.phase & 63 == 0 {
                        super::random_roam_heading(actor);
                    }
                }
                for &slot in ctx.model26 {
                    let Some(peer) = current(actor, ctx, slot) else {
                        continue;
                    };
                    if peer.source_id != actor.row.source_id
                        && (i32::from(actor.row.position.x as i16)
                            - i32::from(peer.position.x as i16))
                        .abs()
                            < i32::from(actor.row.array_pitch())
                        && (i32::from(actor.row.position.y as i16)
                            - i32::from(peer.position.y as i16))
                        .abs()
                            < i32::from(actor.row.array_pitch())
                    {
                        actor.set_roll(
                            heading_for_positions(peer.position, actor.row.position) as i16
                        );
                        break;
                    }
                }
            }
        }
        1 => hit_reaction(actor, ctx)?,
        2 => actor.row.set_source_action(PREKILL_ACTION),
        _ => {}
    }
    Ok(())
}

/// sub_1DBF0 follows its point/wander helper, then scans frozen class3 members
/// using current fields. Its X delta is promoted; its Y delta wraps to i16.
fn guard(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    stage: &RuntimeStageVariable,
    effects: &mut Effects,
) -> Result<(), String> {
    let destination = point(stage)?;
    let away = (i32::from(destination.x as i16) - i32::from(actor.row.position.x as i16)).abs()
        > 3072
        || (i32::from(destination.y as i16) - i32::from(actor.row.position.y as i16)).abs() > 3072;
    if away {
        point_step(actor, ctx, effects)?;
    } else {
        wander(actor, ctx, effects)?;
    }
    if actor.row.action != super::STAGE_ACTION
        || actor.row.record[77] != 2
        || actor.row.record[61] == 0
        || ctx.rules.scan_period == 0
        || actor.row.phase % ctx.rules.scan_period != 0
    {
        return Ok(());
    }
    let range = ctx.rules.target_range.wrapping_mul(ctx.rules.target_range);
    let mut best = None;
    let mut nearest = u32::MAX;
    for &slot in ctx.class3 {
        let Some(target) = current(actor, ctx, slot) else {
            continue;
        };
        let dx = i32::from(target.position.x) - i32::from(actor.row.position.x);
        let dy = i32::from(target.position.y.wrapping_sub(actor.row.position.y) as i16);
        let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
        if squared <= range
            && target.flags & 0x20 == 0
            && angle_distance(
                i32::from(actor.row.yaw),
                heading_for_positions(actor.row.position, target.position),
            ) < ctx.rules.target_half_angle
            && squared < nearest
        {
            nearest = squared;
            best = Some(target.slot);
        }
    }
    if let Some(slot) = best {
        actor.set_target(slot);
        actor.row.record[77] = 10;
        attack(actor, ctx)?;
    }
    Ok(())
}

fn family_slots(ctx: &Context<'_>, model: u16) -> Result<Vec<u16>, String> {
    if let Some(families) = ctx.families {
        return families
            .get(usize::from(model))
            .cloned()
            .ok_or_else(|| format!("model26 StageVar names absent source family{model}"));
    }
    // Explicit standalone fixtures provide their frozen membership in order.
    Ok(ctx
        .targets
        .iter()
        .filter(|row| row.class == 5 && u16::from(row.model) == model)
        .map(|row| row.slot)
        .collect())
}

/// sub_1E3E0 first shares a current relation within the same StageVar group,
/// then chooses the nearest member of the descriptor's frozen source family.
fn bind_leader(
    actor: &Actor<'_>,
    ctx: &Context<'_>,
    stage: &RuntimeStageVariable,
) -> Result<u16, String> {
    if stage.flags & 1 != 0 {
        for &slot in ctx.model26 {
            let Some(peer) = current(actor, ctx, slot) else {
                continue;
            };
            if required(peer, peer.relations.stage_index, "StageVar index")? == actor.row.record[76]
            {
                let relation = required(peer, peer.relations.stage_relation, "StageVar relation")?;
                if relation != 0 {
                    return Ok(relation);
                }
            }
        }
    }
    let model = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u16,
        _ => return Err("model26 family route has no source model".to_owned()),
    };
    let mut nearest = u32::MAX;
    let mut selected = 0;
    for slot in family_slots(ctx, model)? {
        let Some(target) = current(actor, ctx, slot) else {
            continue;
        };
        let squared = super::distance_squared_xy(actor.row.position, target.position);
        if squared < nearest {
            nearest = squared;
            selected = slot;
        }
    }
    Ok(selected)
}

fn leader(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    stage: &RuntimeStageVariable,
    bind: bool,
) -> Result<Option<Target>, String> {
    let slot = if stage.flags & 2 != 0 {
        let mut slot = word(actor, 78);
        if slot == 0 && bind {
            slot = bind_leader(actor, ctx, stage)?;
            put(actor, 78, slot);
        }
        slot
    } else {
        match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        }
    };
    Ok(current(actor, ctx, slot))
}

fn avoid_peers(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    for &slot in ctx.model26 {
        let Some(peer) = current(actor, ctx, slot) else {
            continue;
        };
        if peer.source_id != actor.row.source_id
            && (i32::from(actor.row.position.x) - i32::from(peer.position.x)).abs()
                < i32::from(actor.row.array_pitch())
            && (i32::from(actor.row.position.y) - i32::from(peer.position.y)).abs()
                < i32::from(actor.row.array_pitch())
        {
            actor.set_roll(heading_for_positions(peer.position, actor.row.position) as i16);
            break;
        }
    }
}

/// sub_1D8C0. Peer avoidance does not skip the following leader-separation
/// test; the mode3/4 wrapper's attack test also runs after damage reactions.
fn follow(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    mode: u8,
    effects: &mut Effects,
) -> Result<(), String> {
    match damage(actor, ctx)? {
        0 => {
            move_actor(actor, ctx, effects);
            if actor.row.phase & 7 == 0 {
                let stage = route(actor, ctx)?;
                if let Some(target) = leader(actor, ctx, stage, true)? {
                    if actor.row.control_flags & 0x40000 == 0 {
                        actor.set_roll(
                            heading_for_positions(actor.row.position, target.position) as i16
                        );
                        if actor.row.phase & 63 == 0 {
                            super::random_roam_heading(actor);
                        }
                    }
                    avoid_peers(actor, ctx);
                    let target =
                        current(actor, ctx, target.slot).expect("leader read remains current");
                    let width = i32::from(actor.row.array_pitch())
                        + i32::from(required(
                            target,
                            target.pitch_extent,
                            "leader collision width",
                        )?);
                    if (i32::from(actor.row.position.x) - i32::from(target.position.x)).abs()
                        < width
                        && (i32::from(actor.row.position.y) - i32::from(target.position.y)).abs()
                            < width
                    {
                        actor.set_roll(
                            heading_for_positions(target.position, actor.row.position) as i16
                        );
                    }
                } else if actor.row.phase & 15 == 0 && actor.row.control_flags & 0x40000 == 0 {
                    random_wander(actor);
                }
            }
        }
        1 => hit_reaction(actor, ctx)?,
        2 => actor.row.set_source_action(PREKILL_ACTION),
        _ => {}
    }
    if mode != 5 && actor.row.phase & 7 == 0 {
        let stage = route(actor, ctx)?;
        if let Some(leader) = leader(actor, ctx, stage, false)? {
            let target = if mode == 3 {
                Some(leader)
            } else {
                let slot = required(leader, leader.relations.target, "leader target")?;
                if slot == 0xae02 {
                    None
                } else {
                    current(actor, ctx, slot)
                }
            };
            if let Some(target) = target
                && super::distance(actor.row.position, target.position) <= ctx.rules.target_range
            {
                actor.set_target(target.slot);
                actor.row.record[77] = 10;
                attack(actor, ctx)?;
            }
        }
    }
    Ok(())
}

fn parent_pose(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    effects: &mut Effects,
) -> Result<(), String> {
    let parent = current(actor, ctx, word(actor, 44));
    effects.stage_hide = parent.is_none_or(|parent| parent.life < 0);
    let parent = parent.or(ctx.sentinel);
    let (mut position, yaw, roll) = if let Some(parent) = parent {
        (
            parent.position,
            required(parent, parent.source_yaw, "parent yaw")?,
            required(parent, parent.source_roll, "parent roll")?,
        )
    } else {
        (Position::default(), 0, 0)
    };
    if position.z <= 0 {
        position.z = 0;
    }
    ctx.terrain.set_source_predicted_position(position);
    actor.row.set_position(position);
    actor.row.set_yaw(yaw);
    actor.set_roll(roll);
    Ok(())
}

/// sub_1E700 is shared by summon and Alliance stages. Its fatal branch sets
/// timer1 instead of prekill; its caller still runs its own targeting tail.
fn escort(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) -> Result<(), String> {
    let action = actor.row.action;
    match damage(actor, ctx)? {
        0 => {
            move_actor(actor, ctx, effects);
            if actor.row.phase & 7 == 0
                && let Some(target) = current(actor, ctx, actor.target())
            {
                if actor.row.control_flags & 0x40000 == 0 {
                    actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                    if actor.row.phase & 63 == 0 {
                        super::random_roam_heading(actor);
                    }
                }
                avoid_peers(actor, ctx);
            }
        }
        1 => {
            move_actor(actor, ctx, effects);
            let attacker = actor.attacker();
            if attacker != word(actor, 44) && different_family(actor, ctx, attacker) {
                actor.set_target(attacker);
                attack(actor, ctx)?;
                effects.feedback = Some((word(actor, 44), 19));
            }
        }
        2 => {
            actor.row.set_source_action(action);
            put(actor, 50, 1);
        }
        _ => {}
    }
    Ok(())
}

fn summon(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    mode: u8,
    effects: &mut Effects,
) -> Result<(), String> {
    let parent_slot = word(actor, 44);
    let parent = current(actor, ctx, parent_slot)
        .filter(|parent| parent.life >= 0 && parent.flags & 0x400 == 0);
    if parent.is_none() {
        put(actor, 50, 0);
    } else if mode == 13 {
        let remaining = word(actor, 50).wrapping_sub(1);
        put(actor, 50, remaining);
    }
    if word(actor, 50) as i16 <= 0 {
        effects.stage_expiry = Some(mode == 13);
        return Ok(());
    }
    let parent = parent.expect("current summon parent checked");
    let mut target = current(actor, ctx, actor.target());
    if target.is_some_and(|target| target.life <= 0 || target.flags & 0x400 != 0) {
        actor.set_target(0);
        target = None;
    }
    if target.is_none() && actor.row.phase & 7 == 0 {
        let mut nearest = u32::MAX;
        let mut selected = None;
        for &slot in ctx.class3 {
            let Some(candidate) = current(actor, ctx, slot) else {
                continue;
            };
            if candidate.source_id != parent.source_id && matches!(candidate.model, 0 | 1) {
                let squared = super::distance_squared_xy(parent.position, candidate.position);
                if squared < nearest {
                    nearest = squared;
                    selected = Some(candidate);
                }
            }
        }
        target = selected;
        actor.set_target(target.map_or(0, |target| target.slot));
    }
    if let Some(target) = target {
        escort(actor, ctx, effects)?;
        if actor.row.phase & 7 == 0
            && let Some(target) = current(actor, ctx, target.slot)
            && super::distance(actor.row.position, target.position) < ctx.rules.target_range
        {
            actor.row.set_source_action(DRAIN_ACTION);
        }
    } else {
        actor.set_target(parent_slot);
        escort(actor, ctx, effects)?;
        actor.set_target(0);
        let remaining = word(actor, 50).wrapping_sub(4);
        put(actor, 50, remaining);
    }
    Ok(())
}

fn blink(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    effects: &mut Effects,
    clear: bool,
) -> Result<(), String> {
    let remaining = word(actor, 50) as i16;
    let duration = i32::from(word(actor, 52));
    let phase = if clear {
        0
    } else if i32::from(remaining) <= duration - 16 && remaining >= 16 {
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
    let mut child = word(actor, 56);
    for _ in 0..1000 {
        let Some(linked) = current(actor, ctx, child) else {
            break;
        };
        effects.linked_blink_flags.push((linked.handle, flags));
        child = required(linked, linked.relations.next, "linked child")?;
    }
    Ok(())
}

fn alliance(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) -> Result<(), String> {
    let remaining = word(actor, 50).wrapping_sub(1);
    put(actor, 50, remaining);
    if remaining as i16 > 0 {
        blink(actor, ctx, effects, false)?;
    }
    let parent_slot = word(actor, 44);
    let parent = (remaining as i16 > 0)
        .then(|| current(actor, ctx, parent_slot))
        .flatten()
        .filter(|parent| parent.life > 0);
    if parent.is_none() {
        actor.row.record[77] = 10;
        actor.set_target(0);
        put(actor, 44, 0);
        actor.row.record[76] = word(actor, 78) as u8;
        blink(actor, ctx, effects, true)?;
        return Ok(());
    }
    let saved = actor.target();
    actor.set_target(parent_slot);
    escort(actor, ctx, effects)?;
    let parent = current(actor, ctx, parent_slot).expect("current Alliance parent checked");
    let selected = required(parent, parent.relations.target, "parent target")?;
    let selected = if selected == 0 {
        required(parent, parent.relations.attacker, "parent attacker")?
    } else {
        selected
    };
    actor.set_target(if selected == 0 { saved } else { selected });
    if let Some(target) = current(actor, ctx, actor.target()) {
        let sibling = required(target, target.relations.parent, "target parent")? == parent_slot
            && required(target, target.relations.stage_mode, "target StageVar mode")? == 14;
        if sibling || target.life <= 0 || target.flags & 0x400 != 0 {
            actor.set_target(0);
        } else if super::distance(actor.row.position, target.position) < ctx.rules.target_range {
            actor.row.set_source_action(DRAIN_ACTION);
            effects.earlier_feedback = effects.feedback.take();
            effects.feedback = Some((parent_slot, 24));
        }
    }
    Ok(())
}

pub(super) fn step(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Result<Effects, String> {
    let mode = actor.row.record[77];
    let mut effects = Effects::default();
    match mode {
        1 => point_step(actor, ctx, &mut effects)?,
        2 => {
            let stage = route(actor, ctx)?;
            guard(actor, ctx, stage, &mut effects)?;
        }
        3..=5 => follow(actor, ctx, mode, &mut effects)?,
        6..=9 => wander(actor, ctx, &mut effects)?,
        10 => attack(actor, ctx)?,
        12 => parent_pose(actor, ctx, &mut effects)?,
        13 | 16 => summon(actor, ctx, mode, &mut effects)?,
        14 => alliance(actor, ctx, &mut effects)?,
        17 => {
            move_actor(actor, ctx, &mut effects);
            if actor.row.life < 0 {
                effects.stage_hide = true;
            } else if let Some(target) =
                current(actor, ctx, actor.target()).filter(|target| target.living())
            {
                actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
                actor
                    .row
                    .set_actual_speed(actor.row.actual_speed.wrapping_sub(8));
                if actor.row.actual_speed <= 16 {
                    actor.row.record[77] = 16;
                }
            } else {
                actor.row.record[77] = 16;
            }
        }
        // Source1D5D0 has literal returns for all other mode bytes, including
        // 0,11,15,18 and255. Source293B0 still owns the enclosing callback tail.
        _ => {}
    }
    Ok(effects)
}
