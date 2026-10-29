//! Cymmerian `sub_28F50` StageVar entry and authored-route binding.

use super::{Actor, Context, RULES, SOURCE_MODEL};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::creatures::archer::ArcherStageRuntime;
use crate::native_world::{EntitySnapshot, RuntimeSnapshot};

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn distance_squared(a: crate::Position, b: crate::Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

/// sub_1DDA0 and sub_1DBF0 use the StageVar coordinate as a patrol point.
fn step_point_or_guard(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    runtime: &RuntimeSnapshot,
    mode: u8,
) -> Result<(), String> {
    let index = actor.row.record[76];
    let descriptor = runtime
        .descriptor(index)
        .ok_or_else(|| format!("Cymmerian StageVar {index} is absent"))?;
    let point = descriptor.point;
    let away = (i32::from(actor.row.position.x as i16) - i32::from(point.x as i16)).abs() > 3072
        || (i32::from(actor.row.position.y as i16) - i32::from(point.y as i16)).abs() > 3072;
    let seek_point = mode == 1 || away;
    match super::consume(actor, ctx) {
        0 => {
            if seek_point || RULES.behavior_flags & 2 == 0 {
                super::move_actor(actor, ctx);
                if seek_point && actor.row.phase & 7 == 0 {
                    if actor.row.control_flags & 0x40000 == 0 {
                        actor
                            .row
                            .set_source_roll(
                                crate::heading_for_positions(actor.row.position, point) as i16,
                            );
                        if actor.row.phase & 63 == 0 {
                            super::jitter(actor);
                        }
                    }
                    if let Some(peer) = ctx
                        .model25
                        .iter()
                        .filter_map(|slot| ctx.target(*slot))
                        .find(|peer| {
                            peer.source_id != actor.row.source_id
                                && (i32::from(actor.row.position.x as i16)
                                    - i32::from(peer.position.x as i16))
                                .abs()
                                    < i32::from(actor.row.array_pitch())
                                && (i32::from(actor.row.position.y as i16)
                                    - i32::from(peer.position.y as i16))
                                .abs()
                                    < i32::from(actor.row.array_pitch())
                        })
                    {
                        actor.row.set_source_roll(crate::heading_for_positions(
                            peer.position,
                            actor.row.position,
                        ) as i16);
                    }
                } else if !seek_point
                    && actor.row.phase & 15 == 0
                    && actor.row.control_flags & 0x40000 == 0
                {
                    let rng = crate::native_world::level2_next_rng16(actor.row.entity_rng);
                    actor.row.set_entity_rng(rng);
                    actor.row.set_source_roll(
                        actor
                            .row
                            .source_roll()
                            .wrapping_add((rng % 113 + 142) as i16)
                            & 2047,
                    );
                }
            }
        }
        1 => {
            let attacker = word(actor.row, 42);
            if ctx.target(attacker).is_none_or(|target| {
                (target.class, target.model) != (super::SOURCE_CLASS, SOURCE_MODEL)
            }) {
                actor.set_target(attacker);
                actor.row.record[77] = 10;
                actor.set_action(202);
            }
            let descent = if super::is_split_body(actor) {
                RULES.split_descent
            } else {
                RULES.descent
            };
            crate::ground_creature_motion::relax_to_terrain(
                &mut actor.row.position,
                ctx.terrain,
                RULES.ground_clearance,
                descent,
            );
            actor.row.set_position(actor.row.position);
        }
        2 => actor.set_action(204),
        _ => {}
    }
    let period = super::scan_period(actor);
    if mode == 2
        && actor.row.action == 207
        && actor.row.record[77] == 2
        && actor.row.record[61] != 0
        && period != 0
        && actor.row.phase % period == 0
    {
        let range = super::target_range(actor);
        let range_squared = range.wrapping_mul(range);
        let target = ctx
            .class3
            .iter()
            .filter_map(|slot| ctx.target(*slot))
            .filter(|target| target.flags & 0x20 == 0)
            .filter_map(|target| {
                let dx = i32::from(target.position.x) - i32::from(actor.row.position.x);
                let dy = i32::from(target.position.y.wrapping_sub(actor.row.position.y) as i16);
                let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                (squared <= range_squared
                    && crate::angle_distance(
                        i32::from(actor.row.yaw),
                        crate::heading_for_positions(actor.row.position, target.position),
                    ) < super::target_half_angle(actor))
                .then_some((squared, target))
            })
            .min_by_key(|(squared, _)| *squared)
            .map(|(_, target)| target);
        if let Some(target) = target {
            actor.set_target(target.slot);
            actor.row.record[77] = 10;
            actor.set_action(202);
        }
    }
    Ok(())
}

fn alliance_blink(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    effects: &mut super::Effects,
    clear: bool,
) {
    let remaining = word(actor.row, 50) as i16;
    let duration = word(actor.row, 52) as i16;
    let phase = if clear {
        0
    } else if remaining <= duration.wrapping_sub(16) && remaining >= 16 {
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
    let mut child = word(actor.row, 56);
    for _ in 0..ctx.targets.len() {
        let Some(linked) = ctx.target(child) else {
            break;
        };
        effects.linked_blink_flags.push((linked.slot, flags));
        child = linked.next;
    }
}

/// The attack callback invokes sub_1ED30 independently of the stage callback.
pub(super) fn alliance_target(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    effects: &mut super::Effects,
    target: Option<super::Target>,
) -> Option<super::Target> {
    if actor.row.record[77] != 14 {
        return target;
    }
    alliance_blink(actor, ctx, effects, false);
    let remaining = word(actor.row, 50).wrapping_sub(1);
    put(actor.row, 50, remaining);
    if (remaining as i16) <= 0 {
        return None;
    }
    let Some(parent) = ctx.target(word(actor.row, 44)) else {
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

fn step_alliance(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    parent_current: bool,
    effects: &mut super::Effects,
) {
    let remaining = word(actor.row, 50).wrapping_sub(1);
    put(actor.row, 50, remaining);
    let parent_slot = word(actor.row, 44);
    if (remaining as i16) > 0 {
        alliance_blink(actor, ctx, effects, false);
    }
    let parent = ctx
        .target(parent_slot)
        .filter(|parent| parent_current && parent.life > 0 && (remaining as i16) > 0);
    let Some(parent) = parent else {
        actor.row.record[77] = 10;
        actor.set_target(0);
        put(actor.row, 44, 0);
        actor.row.record[76] = word(actor.row, 78) as u8;
        alliance_blink(actor, ctx, effects, true);
        return;
    };
    let saved_target = actor.target();
    actor.set_target(parent_slot);
    let hit = super::consume(actor, ctx);
    if hit == 2 {
        put(actor.row, 50, 1);
    } else {
        super::move_actor(actor, ctx);
        if hit == 1 {
            let attacker = word(actor.row, 42);
            if attacker != parent_slot
                && ctx.target(attacker).is_none_or(|target| {
                    (target.class, target.model) != (super::SOURCE_CLASS, SOURCE_MODEL)
                })
            {
                actor.set_target(attacker);
                actor.set_action(202);
                effects.feedback = Some((parent_slot, 19));
            }
        } else if actor.row.phase & 7 == 0 {
            if actor.row.control_flags & 0x40000 == 0 {
                actor.row.set_source_roll(crate::heading_for_positions(
                    actor.row.position,
                    parent.position,
                ) as i16);
                if actor.row.phase & 63 == 0 {
                    super::jitter(actor);
                }
            }
            if let Some(peer) = ctx
                .model25
                .iter()
                .filter_map(|slot| ctx.target(*slot))
                .find(|peer| {
                    peer.source_id != actor.row.source_id
                        && i32::from(peer.position.x).abs_diff(i32::from(actor.row.position.x))
                            < u32::from(actor.row.array_pitch().cast_unsigned())
                        && i32::from(peer.position.y).abs_diff(i32::from(actor.row.position.y))
                            < u32::from(actor.row.array_pitch().cast_unsigned())
                })
            {
                actor
                    .row
                    .set_source_roll(
                        crate::heading_for_positions(peer.position, actor.row.position) as i16,
                    );
            }
        }
    }
    let selected = if parent.target != 0 {
        parent.target
    } else {
        parent.attacker
    };
    actor.set_target(if selected != 0 {
        selected
    } else {
        saved_target
    });
    if let Some(target) = ctx.target(actor.target()) {
        if (target.parent == parent_slot && target.stage_mode == 14)
            || target.life <= 0
            || target.flags & 0x400 != 0
        {
            actor.set_target(0);
        } else if super::distance(actor.row.position, target.position) < super::target_range(actor)
        {
            actor.set_action(202);
            effects.earlier_feedback = effects.feedback.take();
            effects.feedback = Some((parent_slot, 24));
        }
    }
}

/// Consume the reachable Army sub_28F50 -> sub_1D5D0(200) -> sub_1E580
/// callback. Some(true) means source type-10/subtype-73 and subtype-0
/// products must be materialized before this source pass returns.
pub(crate) fn step_action_207(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    runtime: &RuntimeSnapshot,
    families: &[Vec<u16>; 29],
    parent_current: bool,
    effects: &mut super::Effects,
) -> Result<Option<bool>, String> {
    let mode = actor.row.record[77];
    if matches!(mode, 1 | 2) {
        step_point_or_guard(actor, ctx, runtime, mode)?;
        return Ok(None);
    }
    if matches!(mode, 3..=5) {
        step_follow_leader_target(actor, ctx, runtime, families, mode)?;
        return Ok(None);
    }
    if matches!(mode, 6..=9) {
        // sub_1D880/sub_1D8A0 both enter sub_1E1C0. Its mailbox
        // and linked-life comparison precede movement and hit reactions.
        match super::consume(actor, ctx) {
            0 if RULES.behavior_flags & 2 == 0 => {
                super::move_actor(actor, ctx);
                if actor.row.phase & 15 == 0 && actor.row.control_flags & 0x40000 == 0 {
                    let rng = crate::native_world::level2_next_rng16(actor.row.entity_rng);
                    actor.row.set_entity_rng(rng);
                    actor.row.set_source_roll(
                        actor
                            .row
                            .source_roll()
                            .wrapping_add((rng % 113 + 142) as i16)
                            & 2047,
                    );
                }
            }
            1 => {
                let attacker = word(actor.row, 42);
                if ctx.target(attacker).is_none_or(|target| {
                    (target.class, target.model) != (super::SOURCE_CLASS, SOURCE_MODEL)
                }) {
                    actor.set_target(attacker);
                    actor.row.record[77] = 10;
                    actor.set_action(202);
                }
                let descent = if super::is_split_body(actor) {
                    RULES.split_descent
                } else {
                    RULES.descent
                };
                crate::ground_creature_motion::relax_to_terrain(
                    &mut actor.row.position,
                    ctx.terrain,
                    RULES.ground_clearance,
                    descent,
                );
                actor.row.set_position(actor.row.position);
            }
            2 => actor.set_action(204),
            _ => {}
        }
        return Ok(None);
    }
    if mode == 10 {
        // Descriptor 92/95 have behavior bit 8 clear: sub_1E040 selects
        // the ordinary attack action, without running it in this callback.
        actor.set_action(202);
        return Ok(None);
    }
    if mode == 12 {
        let parent = ctx.target(word(actor.row, 44)).filter(|_| parent_current);
        if parent.is_none_or(|parent| parent.life < 0) {
            actor.row.disable_drawing();
        }
        let parent = parent.unwrap_or_default();
        let mut position = parent.position;
        position.z = position.z.saturating_sub(512);
        actor.row.set_position(position);
        actor.row.set_yaw(parent.yaw);
        actor.row.set_source_roll(parent.roll);
        return Ok(None);
    }
    if mode == 14 {
        step_alliance(actor, ctx, parent_current, effects);
        return Ok(None);
    }
    if mode == 17 {
        super::move_actor(actor, ctx);
        if actor.row.life < 0 {
            actor.row.disable_drawing();
        } else if let Some(target) = ctx.target(actor.target()).filter(|target| target.living()) {
            actor
                .row
                .set_source_roll(
                    crate::heading_for_positions(actor.row.position, target.position) as i16,
                );
            actor
                .row
                .set_actual_speed(actor.row.actual_speed.wrapping_sub(8));
            if actor.row.actual_speed <= 16 {
                actor.row.record[77] = 16;
            }
        } else {
            actor.row.record[77] = 16;
        }
        return Ok(None);
    }
    if !matches!(mode, 13 | 16) {
        return Ok(None);
    }

    let parent_slot = word(actor.row, 44);
    let parent = ctx.target(parent_slot).filter(|parent| {
        parent_current
            && parent.life >= 0
            && parent.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
    });
    if parent.is_none() {
        put(actor.row, 50, 0);
    } else if mode == 13 {
        put(actor.row, 50, word(actor.row, 50).wrapping_sub(1));
    }
    let timer = word(actor.row, 50) as i16;
    if timer <= 0 {
        return Ok(Some(mode == 13));
    }
    let parent = parent.expect("live Army parent checked above");

    let mut target = ctx.target(actor.target()).filter(|target| {
        target.life > 0 && target.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
    });
    if target.is_none() {
        actor.set_target(0);
        if actor.row.phase & 7 == 0 {
            target = ctx
                .class3
                .iter()
                .filter_map(|slot| ctx.target(*slot))
                .filter(|candidate| {
                    candidate.slot != actor.row.index
                        && candidate.source_id != parent.source_id
                        && matches!(candidate.model, 0 | 1)
                        && candidate.life > 0
                        && candidate.flags & crate::native_world::ENTITY_DRAW_DISABLED_BIT == 0
                })
                .min_by_key(|candidate| distance_squared(parent.position, candidate.position));
            if let Some(target) = target {
                actor.set_target(target.slot);
            }
        }
    }

    if target.is_none() {
        actor.set_target(parent_slot);
    }
    let hit = super::consume(actor, ctx);
    if hit == 2 {
        put(actor.row, 50, 1);
    } else {
        super::move_actor(actor, ctx);
        if hit == 1 {
            let attacker = word(actor.row, 42);
            if attacker != parent_slot
                && ctx.target(attacker).is_none_or(|target| {
                    (target.class, target.model) != (super::SOURCE_CLASS, SOURCE_MODEL)
                })
            {
                actor.set_target(attacker);
                actor.set_action(202);
            }
        } else if actor.row.phase & 7 == 0 {
            if actor.row.control_flags & 0x40000 == 0 {
                let position = target.map_or(parent.position, |target| target.position);
                actor.row.set_source_roll(
                    crate::heading_for_positions(actor.row.position, position) as i16,
                );
                if actor.row.phase & 63 == 0 {
                    super::jitter(actor);
                }
            }
            if let Some(peer) = ctx
                .model25
                .iter()
                .filter_map(|slot| ctx.target(*slot))
                .find(|peer| {
                    peer.source_id != actor.row.source_id
                        && i32::from(peer.position.x).abs_diff(i32::from(actor.row.position.x))
                            < u32::from(actor.row.array_pitch().cast_unsigned())
                        && i32::from(peer.position.y).abs_diff(i32::from(actor.row.position.y))
                            < u32::from(actor.row.array_pitch().cast_unsigned())
                })
            {
                actor
                    .row
                    .set_source_roll(
                        crate::heading_for_positions(peer.position, actor.row.position) as i16,
                    );
            }
        }
    }
    if let Some(target) = target {
        if actor.row.phase & 7 == 0
            && super::distance(actor.row.position, target.position) < RULES.target_range
        {
            actor.set_action(202);
        }
    } else {
        // sub_1E580 temporarily points word96 at the parent while invoking
        // sub_1E700, then clears it and applies the source's four-tick penalty.
        actor.set_target(0);
        put(actor.row, 50, word(actor.row, 50).wrapping_sub(4));
    }
    Ok(None)
}

/// `sub_1D700 -> sub_1D8C0`: follow the StageVar leader and, on the
/// source cadence, attack the leader's current target when it is in range.
fn step_follow_leader_target(
    actor: &mut Actor<'_>,
    ctx: &Context<'_>,
    runtime: &RuntimeSnapshot,
    families: &[Vec<u16>; 29],
    mode: u8,
) -> Result<(), String> {
    let index = actor.row.record[76];
    let descriptor = runtime
        .descriptor(index)
        .ok_or_else(|| format!("Cymmerian StageVar {index} is absent"))?;
    let hit = super::consume(actor, ctx);
    let mut leader = None;
    if hit == 0 {
        super::move_actor(actor, ctx);
        if actor.row.phase & 7 == 0 {
            if descriptor.flags & 2 != 0 {
                let mut relation = word(actor.row, 78);
                if relation == 0 && descriptor.flags & 1 != 0 {
                    relation = ctx
                        .model25
                        .iter()
                        .filter_map(|slot| ctx.target(*slot))
                        .find(|peer| peer.stage_index == index && peer.stage_relation != 0)
                        .map_or(0, |peer| peer.stage_relation);
                }
                if relation == 0 {
                    relation = families
                        .get(usize::from(descriptor.model))
                        .into_iter()
                        .flatten()
                        .filter_map(|slot| ctx.target(*slot))
                        .min_by_key(|target| distance_squared(actor.row.position, target.position))
                        .map_or(0, |target| target.slot);
                }
                if word(actor.row, 78) == 0 {
                    put(actor.row, 78, relation);
                }
                leader = ctx.target(relation);
            } else {
                leader = ctx.target(descriptor.actor);
            }
            if let Some(target) = leader {
                if actor.row.control_flags & 0x40000 == 0 {
                    actor.row.set_source_roll(crate::heading_for_positions(
                        actor.row.position,
                        target.position,
                    ) as i16);
                    if actor.row.phase & 63 == 0 {
                        super::jitter(actor);
                    }
                }
                if let Some(peer) = ctx
                    .model25
                    .iter()
                    .filter_map(|slot| ctx.target(*slot))
                    .find(|peer| {
                        peer.source_id != actor.row.source_id
                            && i32::from(peer.position.x).abs_diff(i32::from(actor.row.position.x))
                                < u32::from(actor.row.array_pitch().cast_unsigned())
                            && i32::from(peer.position.y).abs_diff(i32::from(actor.row.position.y))
                                < u32::from(actor.row.array_pitch().cast_unsigned())
                    })
                {
                    actor.row.set_source_roll(crate::heading_for_positions(
                        peer.position,
                        actor.row.position,
                    ) as i16);
                }
                let extent = i32::from(actor.row.array_pitch()) + i32::from(target.pitch_extent);
                if i32::from(target.position.x).abs_diff(i32::from(actor.row.position.x))
                    < extent.max(0) as u32
                    && i32::from(target.position.y).abs_diff(i32::from(actor.row.position.y))
                        < extent.max(0) as u32
                {
                    actor.row.set_source_roll(crate::heading_for_positions(
                        target.position,
                        actor.row.position,
                    ) as i16);
                }
            } else if actor.row.phase & 15 == 0 && actor.row.control_flags & 0x40000 == 0 {
                let rng = crate::native_world::level2_next_rng16(actor.row.entity_rng);
                actor.row.set_entity_rng(rng);
                actor.row.set_source_roll(
                    actor
                        .row
                        .source_roll()
                        .wrapping_add((rng % 113 + 142) as i16)
                        & 2047,
                );
            }
        }
    } else if hit == 1 {
        let attacker = word(actor.row, 42);
        if ctx.target(attacker).is_none_or(|target| {
            (target.class, target.model) != (super::SOURCE_CLASS, SOURCE_MODEL)
        }) {
            actor.set_target(attacker);
            actor.row.record[77] = 10;
            actor.set_action(202);
        }
        let descent = if super::is_split_body(actor) {
            RULES.split_descent
        } else {
            RULES.descent
        };
        crate::ground_creature_motion::relax_to_terrain(
            &mut actor.row.position,
            ctx.terrain,
            RULES.ground_clearance,
            descent,
        );
        actor.row.set_position(actor.row.position);
    } else {
        actor.set_action(204);
    }

    if mode != 5 && actor.row.phase & 7 == 0 {
        // sub_1D700 re-reads the leader after the follow callback.
        let leader = if descriptor.flags & 2 != 0 {
            ctx.target(word(actor.row, 78))
        } else {
            ctx.target(descriptor.actor)
        }
        .or(leader);
        let selected = leader.and_then(|leader| {
            if mode == 3 {
                Some(leader)
            } else {
                (leader.target != 0xae02)
                    .then(|| ctx.target(leader.target))
                    .flatten()
            }
        });
        if let Some(target) = selected
            && super::distance(actor.row.position, target.position) <= super::target_range(actor)
        {
            actor.set_target(target.slot);
            actor.row.record[77] = 10;
            actor.set_action(202);
        }
    }
    Ok(())
}

fn edit_route(row: &mut EntitySnapshot, runtime: &mut dyn ArcherStageRuntime, index: u8) {
    let mut action = row.action;
    let mut route_index = row.record[76] as i8;
    let mut mode = row.record[77] as i8;
    let mut relation = u16::from_le_bytes([row.record[78], row.record[79]]);
    let mut route = super::super::stage_route::Route {
        model: SOURCE_MODEL,
        position: row.position,
        action: &mut action,
        index: &mut route_index,
        mode: &mut mode,
        relation: &mut relation,
    };
    route.bind(runtime, index);
    row.set_source_action(action);
    row.record[76] = route_index as u8;
    row.record[77] = mode as u8;
    put(row, 78, relation);
}

pub(crate) fn bind_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    let row = arena
        .entities
        .get_mut(usize::from(slot))
        .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        .ok_or_else(|| format!("Cymmerian StageVar bind lost slot{slot}"))?;
    edit_route(row, runtime, index);
    Ok(())
}

/// Source `sub_12470(a, 200)` followed by `byte46 = 3` in `sub_28F50`.
pub(crate) fn reset_to_special(actor: &mut Actor<'_>) {
    actor.row.record[76..80].fill(0);
    actor.row.set_source_action(200);
    actor.row.record[74] = 3;
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slots: &[u16],
) {
    let dead = super::mc2::targets_in_arena(arena)
        .into_iter()
        .filter(|target| target.life < 0 || target.flags & 0x400 != 0)
        .map(|target| target.slot)
        .collect::<std::collections::BTreeSet<_>>();
    for &slot in slots {
        let Some(row) = arena
            .entities
            .get_mut(usize::from(slot))
            .filter(|row| row.active && row.class == 5 && row.model == SOURCE_MODEL)
        else {
            continue;
        };
        let mut action = row.action;
        let mut index = row.record[76] as i8;
        let mut mode = row.record[77] as i8;
        let mut relation = u16::from_le_bytes([row.record[78], row.record[79]]);
        let mut route = super::super::stage_route::Route {
            model: SOURCE_MODEL,
            position: row.position,
            action: &mut action,
            index: &mut index,
            mode: &mut mode,
            relation: &mut relation,
        };
        route.prepare(runtime, |target| dead.contains(&target));
        row.set_source_action(action);
        row.record[76] = index as u8;
        row.record[77] = mode as u8;
        put(row, 78, relation);
    }
}
