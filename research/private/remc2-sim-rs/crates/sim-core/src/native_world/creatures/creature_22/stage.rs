//! The model22 action183 delegation to the shared source StageVar language.

use super::{
    Actor, Runtime, SOURCE_MODEL, SourceEntityHandle, StageDescriptor, Target, chain, distance,
    generic_move,
};
use crate::native_world::{ENTITY_DRAW_DISABLED_BIT, level2_next_rng16};
use crate::{FlightWorld, Position, heading_for_positions};

const TARGET_RANGE: u32 = 5120;

pub(crate) fn bind_stage(handle: SourceEntityHandle, index: u8, runtime: &mut dyn Runtime) {
    let descriptor = (index != 0)
        .then(|| runtime.bind_stage_descriptor(index))
        .flatten();
    if let Some(mut actor) = runtime.actor(handle) {
        if let Some(descriptor) = descriptor {
            actor.stage.index = index;
            actor.stage.mode = descriptor.kind;
            actor.stage.relation = if descriptor.kind == 6 {
                descriptor.scalar
            } else {
                0
            };
            actor.action = 183;
        } else {
            actor.stage.index = 0;
            actor.stage.mode = 0;
            actor.stage.relation = 0;
            actor.action = 176;
        }
        runtime.write_actor(actor);
    }
}

fn near(a: Position, b: Position, limit: u16) -> bool {
    a.x.wrapping_sub(b.x).cast_signed().unsigned_abs() <= limit
        && a.y.wrapping_sub(b.y).cast_signed().unsigned_abs() <= limit
}

pub(crate) fn stage_prepass(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    let action = actor.action & 7;
    if matches!(action, 4 | 5) {
        return;
    }
    if matches!(actor.stage.mode, 13 | 14 | 16 | 17) {
        if !matches!(action, 2 | 6) {
            actor.action = 183;
        }
        runtime.write_actor(actor);
        return;
    }
    if (actor.stage.mode == 10 && !matches!(action, 2 | 6))
        || (actor.stage.mode == 15 && action != 0)
    {
        bind_stage(handle, actor.stage.index, runtime);
        return;
    }
    let Some(descriptor) = runtime.stage_descriptor(actor.stage.index) else {
        return;
    };
    let ended = match descriptor.kind {
        1 => near(actor.position, descriptor.point, 2048),
        3 | 4 | 5 | 8 | 9 => {
            if descriptor.flags & 4 != 0 {
                if descriptor.kind == 3 {
                    actor.stage.relation = 0;
                }
                !matches!(action, 2 | 6) || descriptor.kind != 3
            } else {
                if descriptor.flags & 2 != 0
                    && matches!(descriptor.kind, 3..=5)
                    && runtime.target(actor.stage.relation).is_some_and(|row| {
                        row.life < 0 || row.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
                    })
                {
                    actor.stage.relation = 0;
                }
                descriptor.kind == 9
                    && descriptor.flags & 2 == 0
                    && (descriptor.point.x != 0 || descriptor.point.y != 0)
                    && near(actor.position, descriptor.point, 3072)
            }
        }
        6 => {
            actor.stage.relation = actor.stage.relation.wrapping_sub(1);
            actor.stage.relation == 0
        }
        7 => descriptor.flags & 0x18 != 0,
        _ => false,
    };
    runtime.write_actor(actor);
    if ended {
        if descriptor.kind == 7 {
            runtime.rearm_model_relations();
        }
        bind_stage(handle, descriptor.next, runtime);
    }
}

fn consume_mailbox(actor: &mut Actor, runtime: &dyn Runtime, linked_hit: bool) -> u8 {
    let mut result = 0;
    if actor.hit_source != 0 {
        actor.life = actor.life.wrapping_sub(actor.damage);
        actor.attacker = actor.hit_source;
        actor.hit_source = 0;
        result = 1;
    } else {
        actor.attacker = 0;
    }
    for handle in chain(actor.handle, runtime).into_iter().skip(1) {
        if let Some(child) = runtime.actor(handle)
            && child.life < actor.life
        {
            actor.life = child.life;
            actor.attacker = child.attacker;
            if linked_hit {
                result = 1;
            }
            break;
        }
    }
    if actor.life < 0 {
        actor.climb_hold = actor.attacker as i16;
        result = 2;
    }
    result
}

fn jitter(actor: &mut Actor) {
    let first = level2_next_rng16(actor.entity_rng);
    actor.entity_rng = level2_next_rng16(first);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    actor.roll = (i32::from(actor.roll) + (i32::from(actor.entity_rng & 255) + 85) * direction)
        as i16
        & 2047;
}

fn idle_jitter(actor: &mut Actor) {
    if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
        actor.entity_rng = level2_next_rng16(actor.entity_rng);
        actor.roll = actor
            .roll
            .wrapping_add((actor.entity_rng % 113 + 142) as i16)
            & 2047;
    }
}

fn avoid_peers(actor: &mut Actor, runtime: &dyn Runtime) -> bool {
    for slot in runtime.frozen_model_slots(SOURCE_MODEL) {
        if let Some(peer) = runtime.target(*slot)
            && peer.source_id != actor.source_id
            && (i32::from(actor.position.x as i16) - i32::from(peer.position.x as i16)).abs()
                < i32::from(actor.half_width)
            && (i32::from(actor.position.y as i16) - i32::from(peer.position.y as i16)).abs()
                < i32::from(actor.half_width)
        {
            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
            return true;
        }
    }
    false
}

fn follow(
    actor: &mut Actor,
    point: Position,
    actor_target: bool,
    target_width: i16,
    runtime: &dyn Runtime,
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
    if avoid_peers(actor, runtime) {
        return;
    }
    if actor_target
        && (i32::from(actor.position.x) - i32::from(point.x)).abs()
            < i32::from(actor.half_width) + i32::from(target_width)
        && (i32::from(actor.position.y) - i32::from(point.y)).abs()
            < i32::from(actor.half_width) + i32::from(target_width)
    {
        actor.roll = heading_for_positions(point, actor.position) as i16;
    }
}

fn selected(
    actor: &mut Actor,
    descriptor: StageDescriptor,
    runtime: &dyn Runtime,
) -> Option<Target> {
    if descriptor.flags & 2 == 0 {
        return runtime.target(descriptor.actor);
    }
    if actor.stage.relation == 0 {
        if descriptor.flags & 1 != 0 {
            actor.stage.relation = runtime
                .frozen_model_slots(SOURCE_MODEL)
                .iter()
                .filter_map(|slot| runtime.actor_at(*slot))
                .find(|peer| peer.stage.index == actor.stage.index && peer.stage.relation != 0)
                .map_or(0, |peer| peer.stage.relation);
        }
        if actor.stage.relation == 0 {
            actor.stage.relation = runtime
                .frozen_model_slots(descriptor.model)
                .iter()
                .filter_map(|slot| runtime.target(*slot))
                .min_by_key(|target| super::distance_squared_xy(actor.position, target.position))
                .map_or(0, |target| target.slot);
        }
    }
    runtime.target(actor.stage.relation)
}

fn hit_action(actor: &mut Actor, runtime: &dyn Runtime) {
    if runtime
        .target(actor.attacker)
        .is_none_or(|target| target.class != 5 || target.model != SOURCE_MODEL)
    {
        actor.target_or_growth = actor.attacker;
        actor.stage.mode = 10;
        actor.action = 178;
    }
    actor.position.z = actor
        .position
        .z
        .max(runtime.terrain().ground_height(actor.position));
}

fn ordinary(
    actor: &mut Actor,
    runtime: &mut dyn Runtime,
    descriptor: Option<StageDescriptor>,
    point: bool,
) {
    match consume_mailbox(actor, runtime, !point) {
        2 => actor.action = 180,
        1 => hit_action(actor, runtime),
        _ => {
            if generic_move(actor, runtime.terrain()) {
                runtime.decrement_population();
            }
            if point {
                if let Some(descriptor) = descriptor {
                    follow(actor, descriptor.point, false, 0, runtime);
                }
            } else if matches!(actor.stage.mode, 3..=5) {
                if actor.phase & 7 == 0 {
                    if let Some(target) =
                        descriptor.and_then(|descriptor| selected(actor, descriptor, runtime))
                    {
                        follow(actor, target.position, true, target.half_width, runtime);
                    } else {
                        idle_jitter(actor);
                    }
                }
            } else {
                idle_jitter(actor);
            }
        }
    }
}

fn summoned_follow(actor: &mut Actor, runtime: &mut dyn Runtime) {
    match consume_mailbox(actor, runtime, true) {
        2 => {
            actor.stage.remaining = 1;
            return;
        }
        hit => {
            if generic_move(actor, runtime.terrain()) {
                runtime.decrement_population();
            }
            if hit == 1 {
                if actor.attacker != actor.stage.parent
                    && runtime
                        .target(actor.attacker)
                        .is_none_or(|row| row.class != 5 || row.model != SOURCE_MODEL)
                {
                    actor.target_or_growth = actor.attacker;
                    actor.action = 178;
                    runtime.feedback(actor.stage.parent, 19);
                }
            } else if let Some(target) = runtime.target(actor.target_or_growth) {
                follow(actor, target.position, false, 0, runtime);
            }
        }
    }
}

fn blink(actor: &mut Actor, runtime: &mut dyn Runtime, clear: bool) {
    let remaining = actor.stage.remaining;
    let selection = if clear {
        0
    } else if remaining <= actor.stage.duration.wrapping_sub(16) && remaining >= 16 {
        if actor.phase & 0x1c == 16 {
            actor.phase & 3
        } else {
            0
        }
    } else if actor.phase & 1 == 0 {
        2
    } else {
        0
    };
    let bits = match selection {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    actor.control_flags = (actor.control_flags & !0x0600_0000) | bits;
    for handle in chain(actor.handle, runtime).into_iter().skip(1) {
        if let Some(mut child) = runtime.actor(handle) {
            child.control_flags = (child.control_flags & !0x0600_0000) | bits;
            runtime.write_actor(child);
        }
    }
}

fn summoned(actor: &mut Actor, runtime: &mut dyn Runtime) {
    let parent = runtime.target(actor.stage.parent);
    if actor.stage.mode == 14 {
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
        if actor.stage.remaining <= 0 || parent.is_none_or(|parent| parent.life <= 0) {
            actor.stage.mode = 10;
            actor.target_or_growth = 0;
            actor.stage.parent = 0;
            actor.stage.index = actor.stage.relation as u8;
            blink(actor, runtime, true);
            return;
        }
        blink(actor, runtime, false);
        let parent = parent.expect("living Alliance parent");
        let previous = actor.target_or_growth;
        actor.target_or_growth = actor.stage.parent;
        summoned_follow(actor, runtime);
        let inherited = if parent.target != 0 {
            parent.target
        } else {
            parent.attacker
        };
        actor.target_or_growth = if inherited != 0 { inherited } else { previous };
        if let Some(target) = runtime.target(actor.target_or_growth) {
            if (target.parent == actor.stage.parent && target.stage_mode == 14)
                || target.life <= 0
                || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
            {
                actor.target_or_growth = 0;
            } else if distance(actor.position, target.position) < TARGET_RANGE {
                actor.action = 178;
                runtime.feedback(actor.stage.parent, 24);
            }
        }
        return;
    }
    if parent.is_none_or(|parent| {
        parent.life < 0 || parent.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
    }) {
        actor.stage.remaining = 0;
    } else if actor.stage.mode == 13 {
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
    }
    if actor.stage.remaining <= 0 {
        // Commit before allocations: NewEvent may replace the executing row.
        runtime.write_actor(*actor);
        if actor.stage.mode == 13 {
            runtime.emit_effect(actor.position, 73, actor.handle.source_slot);
        }
        runtime.emit_effect(actor.position, 0, actor.handle.source_slot);
        runtime.hide_current_slot(actor.handle.source_slot);
        if let Some(current) = runtime.actor(actor.handle) {
            *actor = current;
        }
        return;
    }
    if runtime
        .target(actor.target_or_growth)
        .is_some_and(|target| {
            target.life <= 0 || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
        })
    {
        actor.target_or_growth = 0;
    }
    if runtime.target(actor.target_or_growth).is_none() && actor.phase & 7 == 0 {
        let parent = parent.expect("valid summon parent");
        actor.target_or_growth = runtime
            .frozen_class3_slots()
            .iter()
            .filter_map(|slot| runtime.target(*slot))
            .filter(|row| matches!(row.model, 0 | 1) && row.source_id != parent.source_id)
            .min_by_key(|row| {
                let x = i32::from(parent.position.x.wrapping_sub(row.position.x).cast_signed());
                let y = i32::from(parent.position.y.wrapping_sub(row.position.y).cast_signed());
                x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32
            })
            .map_or(0, |row| row.slot);
    }
    if let Some(target) = runtime.target(actor.target_or_growth) {
        summoned_follow(actor, runtime);
        if actor.phase & 7 == 0 && distance(actor.position, target.position) < TARGET_RANGE {
            actor.action = 178;
        }
    } else {
        actor.target_or_growth = actor.stage.parent;
        summoned_follow(actor, runtime);
        actor.target_or_growth = 0;
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(4);
    }
}

pub(super) fn step(handle: SourceEntityHandle, runtime: &mut dyn Runtime) {
    let Some(mut actor) = runtime.actor(handle) else {
        return;
    };
    let mode = actor.stage.mode;
    match mode {
        1..=9 => {
            let descriptor = runtime.stage_descriptor(actor.stage.index);
            let point = descriptor.is_some_and(|descriptor| {
                mode == 1
                    || (mode == 2
                        && ((i32::from(actor.position.x as i16)
                            - i32::from(descriptor.point.x as i16))
                        .abs()
                            > 3072
                            || (i32::from(actor.position.y as i16)
                                - i32::from(descriptor.point.y as i16))
                            .abs()
                                > 3072))
            });
            ordinary(&mut actor, runtime, descriptor, point);
            if matches!(mode, 3 | 4)
                && actor.phase & 7 == 0
                && let Some(descriptor) = descriptor
            {
                let slot = if descriptor.flags & 2 != 0 {
                    actor.stage.relation
                } else {
                    descriptor.actor
                };
                if let Some(target) = runtime.target(slot) {
                    let target = if mode == 4 {
                        runtime.target(target.target)
                    } else {
                        Some(target)
                    };
                    if let Some(target) = target.filter(|target| target.slot != 0xae02)
                        && distance(actor.position, target.position) <= TARGET_RANGE
                    {
                        actor.target_or_growth = target.slot;
                        actor.stage.mode = 10;
                        actor.action = 178;
                    }
                }
            }
            if mode == 2
                && actor.action == 183
                && actor.stage.mode == 2
                && actor.cadence_counter != 0
                && actor.phase % super::PROPERTIES.scan_period == 0
            {
                let mut nearest = u32::MAX;
                let mut selected = None;
                for slot in runtime.frozen_class3_slots() {
                    if let Some(target) = runtime.target(*slot) {
                        let x = i32::from(target.position.x) - i32::from(actor.position.x);
                        let y = i32::from(
                            target
                                .position
                                .y
                                .wrapping_sub(actor.position.y)
                                .cast_signed(),
                        );
                        let d = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32;
                        let heading = heading_for_positions(actor.position, target.position);
                        if d <= TARGET_RANGE * TARGET_RANGE
                            && d < nearest
                            && target.control_flags & 0x20 == 0
                            && crate::angle_distance(i32::from(actor.yaw), i32::from(heading)) < 512
                        {
                            nearest = d;
                            selected = Some(target.slot);
                        }
                    }
                }
                if let Some(slot) = selected {
                    actor.target_or_growth = slot;
                    actor.stage.mode = 10;
                    actor.action = 178;
                }
            }
        }
        10 => actor.action = 178,
        12 => {
            if let Some(parent) = runtime.target(actor.stage.parent) {
                if parent.life < 0 {
                    actor.hide();
                }
                actor.position = Position {
                    z: parent.position.z.max(0),
                    ..parent.position
                };
                actor.yaw = parent.yaw;
                actor.roll = parent.roll;
            } else {
                actor.hide();
                actor.position = Position::default();
                actor.yaw = 0;
                actor.roll = 0;
            }
        }
        13 | 14 | 16 => summoned(&mut actor, runtime),
        17 => {
            if generic_move(&mut actor, runtime.terrain()) {
                runtime.decrement_population();
            }
            if actor.life < 0 {
                actor.hide();
            } else if let Some(target) = runtime.target(actor.target_or_growth).filter(|target| {
                target.life >= 0 && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            }) {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.stage.mode = 16;
                }
            } else {
                actor.stage.mode = 16;
            }
        }
        // 1D5D0 has no callback for these exact StageVar modes.
        _ => {}
    }
    runtime.write_actor(actor);
}
