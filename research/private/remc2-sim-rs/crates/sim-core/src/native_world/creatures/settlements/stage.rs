//! The shared 1D5D0 helpers behind settlement actions 111 and 119.
//! Membership is frozen by the source pass; target fields are read at entry
//! to this callback and all writes return to the actor's arena carrier.

use crate::level1_mission::Level1SourceEntityView;
use crate::native_world::creatures::archer::{ArcherStageDescriptor, ArcherStageRuntime};
use crate::villager::{
    VillagerBehaviorConfig, VillagerDamageResult, VillagerState, VillagerStepEffect,
    consume_villager_mailbox, disable_villager, move_villager, next_rng, relax_to_terrain,
};
use crate::{FlightWorld, Position, angle_distance, distance_3d_wrapped, heading_for_positions};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettlementStageLinks {
    pub relation: u16,
    pub parent: u16,
    pub remaining: i16,
    pub duration: i16,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StageTarget {
    pub view: Level1SourceEntityView,
    pub yaw: i16,
    pub roll: i16,
    pub stage_index: i8,
    pub stage_mode: i8,
    pub links: SettlementStageLinks,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StageEffect {
    Expire {
        slot: u16,
        position: Position,
        summoned: bool,
    },
    OwnerFeedback {
        owner: u16,
        selector: u8,
    },
    Flash {
        slot: u16,
        bits: u32,
    },
}

pub(crate) struct StageContext<'a> {
    pub runtime: Option<&'a mut dyn ArcherStageRuntime>,
    pub targets: &'a [StageTarget],
    pub models: &'a [Vec<u16>; 29],
    pub players: &'a [u16],
    pub effects: Vec<StageEffect>,
}

impl StageContext<'_> {
    fn target(&self, slot: u16) -> Option<StageTarget> {
        (slot != 0)
            .then(|| {
                self.targets
                    .iter()
                    .find(|row| row.view.source_entity_index == slot)
                    .copied()
            })
            .flatten()
    }

    fn descriptor(&self, index: i8) -> Option<ArcherStageDescriptor> {
        self.runtime.as_ref()?.descriptor(index as u8)
    }
}

pub(crate) fn targets(
    arena: &crate::campaign::source_arena::SourceArena,
    views: &[Level1SourceEntityView],
    villagers: &[VillagerState],
) -> Vec<StageTarget> {
    views
        .iter()
        .copied()
        .map(|view| {
            let mut target = StageTarget {
                view,
                yaw: 0,
                roll: 0,
                stage_index: 0,
                stage_mode: 0,
                links: SettlementStageLinks::default(),
            };
            if let Some(raw) = arena
                .entities
                .get(usize::from(view.source_entity_index))
                .filter(|raw| raw.active && raw.class == view.class && raw.model == view.model)
            {
                let bytes = raw.record_bytes();
                let word = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
                target.yaw = raw.yaw;
                target.roll = word(36) as i16;
                target.stage_index = bytes[76] as i8;
                target.stage_mode = bytes[77] as i8;
                target.links = SettlementStageLinks {
                    parent: word(44),
                    relation: word(78),
                    remaining: word(50) as i16,
                    duration: word(52) as i16,
                };
            }
            if let Some(row) = villagers
                .iter()
                .find(|row| row.entity_index == view.source_entity_index && row.source_retained())
            {
                target.yaw = row.yaw;
                target.roll = row.roll;
                target.stage_index = row.stage_var_1;
                target.stage_mode = row.stage_var_2;
                target.links = row.stage;
            }
            if let Some(row) = arena
                .house_residents()
                .iter()
                .find(|row| row.active && row.source_entity_index == view.source_entity_index)
            {
                target.yaw = row.yaw;
                target.roll = row.desired_heading;
                target.stage_index = row.stage_var_1;
                target.stage_mode = row.stage_var_2;
                target.links = row.stage;
            }
            target
        })
        .collect()
}

fn heading(actor: &mut VillagerState, target: Position) {
    if actor.control_flags & 0x40000 != 0 {
        return;
    }
    actor.roll = heading_for_positions(actor.position, target) as i16;
    if actor.phase & 63 == 0 {
        let first = next_rng(actor.rng);
        actor.rng = next_rng(first);
        let sign = 2 * (i32::from(first % 157) / 79) - 1;
        actor.roll = actor
            .roll
            .wrapping_add((sign * (i32::from(actor.rng & 255) + 85)) as i16)
            & 2047;
    }
}

fn wander(actor: &mut VillagerState) {
    if actor.phase & 15 == 0 && actor.control_flags & 0x40000 == 0 {
        actor.rng = next_rng(actor.rng);
        actor.roll = actor.roll.wrapping_add((actor.rng % 113 + 142) as i16) & 2047;
    }
}

fn separated(actor: &mut VillagerState, context: &StageContext<'_>, signed: bool) {
    let Some(members) = context.models.get(usize::from(actor.model)) else {
        return;
    };
    for slot in members {
        let Some(peer) = context.target(*slot) else {
            continue;
        };
        if peer.view.source_id == actor.source_id {
            continue;
        }
        let delta = |a: u16, b: u16| {
            if signed {
                (i32::from(a as i16) - i32::from(b as i16)).abs()
            } else {
                (i32::from(a) - i32::from(b)).abs()
            }
        };
        if delta(actor.position.x, peer.view.position.x) < i32::from(actor.collision_half_width)
            && delta(actor.position.y, peer.view.position.y) < i32::from(actor.collision_half_width)
        {
            actor.roll = heading_for_positions(peer.view.position, actor.position) as i16;
            break;
        }
    }
}

fn engage(actor: &mut VillagerState, base: u8, config: VillagerBehaviorConfig) {
    actor.action = base + if config.behavior_flags & 8 != 0 { 6 } else { 2 };
}

fn consume<W: FlightWorld>(
    actor: &mut VillagerState,
    world: &W,
    config: VillagerBehaviorConfig,
    base: u8,
    context: &StageContext<'_>,
    effects: &mut Vec<VillagerStepEffect>,
) -> bool {
    let views = context
        .targets
        .iter()
        .map(|row| row.view)
        .collect::<Vec<_>>();
    match consume_villager_mailbox(actor, &views) {
        VillagerDamageResult::None => false,
        VillagerDamageResult::Dead => {
            actor.action = base + 4;
            true
        }
        VillagerDamageResult::Hit(attacker) => {
            if context
                .target(attacker)
                .is_none_or(|target| (target.view.class, target.view.model) != (5, actor.model))
            {
                actor.target_entity_index = (attacker != 0).then_some(attacker);
                actor.stage_var_2 = 10;
                engage(actor, base, config);
            }
            relax_to_terrain(
                &mut actor.position,
                world,
                config.ground_clearance,
                config.vertical_relaxation,
            );
            let _ = effects;
            true
        }
    }
}

fn moved<W: FlightWorld>(
    actor: &mut VillagerState,
    world: &W,
    config: VillagerBehaviorConfig,
    effects: &mut Vec<VillagerStepEffect>,
) {
    if move_villager(actor, world, config) {
        effects.push(VillagerStepEffect::PopulationCensusDecrement {
            source_entity_index: actor.entity_index,
        });
    }
}

fn stage_target(
    actor: &mut VillagerState,
    descriptor: ArcherStageDescriptor,
    context: &StageContext<'_>,
) -> Option<StageTarget> {
    if descriptor.flags & 2 == 0 {
        return context.target(descriptor.actor);
    }
    if actor.stage.relation == 0 {
        if descriptor.flags & 1 != 0 {
            actor.stage.relation = context
                .models
                .get(usize::from(actor.model))
                .into_iter()
                .flatten()
                .filter_map(|slot| context.target(*slot))
                .find(|peer| peer.stage_index == actor.stage_var_1 && peer.links.relation != 0)
                .map_or(0, |peer| peer.links.relation);
        }
        if actor.stage.relation == 0 {
            let mut distance = u32::MAX;
            if let Some(members) = context.models.get(usize::from(descriptor.model)) {
                for slot in members {
                    let Some(target) = context.target(*slot) else {
                        continue;
                    };
                    let dx =
                        i32::from(actor.position.x.wrapping_sub(target.view.position.x) as i16);
                    let dy =
                        i32::from(actor.position.y.wrapping_sub(target.view.position.y) as i16);
                    let candidate = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                    if candidate < distance {
                        distance = candidate;
                        actor.stage.relation = *slot;
                    }
                }
            }
        }
    }
    context.target(actor.stage.relation)
}

fn movement<W: FlightWorld>(
    actor: &mut VillagerState,
    world: &W,
    config: VillagerBehaviorConfig,
    base: u8,
    point: Option<Position>,
    follow: bool,
    context: &mut StageContext<'_>,
    effects: &mut Vec<VillagerStepEffect>,
) {
    if consume(actor, world, config, base, context, effects) {
        return;
    }
    if !follow && point.is_none() && config.behavior_flags & 2 != 0 {
        return;
    }
    moved(actor, world, config, effects);
    if actor.phase & 7 != 0 {
        return;
    }
    let target = if follow {
        context
            .descriptor(actor.stage_var_1)
            .and_then(|descriptor| stage_target(actor, descriptor, context))
    } else {
        None
    };
    if let Some(point) = point {
        heading(actor, point);
        separated(actor, context, true);
    } else if let Some(target) = target {
        heading(actor, target.view.position);
        separated(actor, context, false);
        let width =
            i32::from(actor.collision_half_width) + i32::from(target.view.collision_half_width);
        if (i32::from(actor.position.x) - i32::from(target.view.position.x)).abs() < width
            && (i32::from(actor.position.y) - i32::from(target.view.position.y)).abs() < width
        {
            actor.roll = heading_for_positions(target.view.position, actor.position) as i16;
        }
    } else {
        wander(actor);
    }
}

fn target_movement<W: FlightWorld>(
    actor: &mut VillagerState,
    world: &W,
    config: VillagerBehaviorConfig,
    base: u8,
    context: &mut StageContext<'_>,
    effects: &mut Vec<VillagerStepEffect>,
) {
    let views = context
        .targets
        .iter()
        .map(|row| row.view)
        .collect::<Vec<_>>();
    match consume_villager_mailbox(actor, &views) {
        VillagerDamageResult::Dead => actor.stage.remaining = 1,
        VillagerDamageResult::Hit(attacker) => {
            moved(actor, world, config, effects);
            if attacker != actor.stage.parent
                && context
                    .target(attacker)
                    .is_none_or(|target| (target.view.class, target.view.model) != (5, actor.model))
            {
                actor.target_entity_index = (attacker != 0).then_some(attacker);
                engage(actor, base, config);
                context.effects.push(StageEffect::OwnerFeedback {
                    owner: actor.stage.parent,
                    selector: 19,
                });
            }
        }
        VillagerDamageResult::None => {
            moved(actor, world, config, effects);
            if actor.phase & 7 == 0
                && let Some(target) = actor
                    .target_entity_index
                    .and_then(|slot| context.target(slot))
            {
                heading(actor, target.view.position);
                separated(actor, context, false);
            }
        }
    }
}

fn flash(actor: &mut VillagerState, context: &mut StageContext<'_>, clear: bool) {
    let selector = if clear {
        0
    } else if actor.stage.remaining <= actor.stage.duration.wrapping_sub(16)
        && actor.stage.remaining >= 16
    {
        if actor.phase & 28 == 16 {
            actor.phase & 3
        } else {
            0
        }
    } else if actor.phase & 1 != 0 {
        0
    } else {
        2
    };
    let bits = match selector {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    actor.control_flags = (actor.control_flags & !0x0600_0000) | bits;
    let mut linked = actor.linked_entity_index;
    for _ in 0..context.targets.len() {
        let Some(target) = context.target(linked) else {
            break;
        };
        context
            .effects
            .push(StageEffect::Flash { slot: linked, bits });
        linked = target.view.linked_entity_index;
    }
}

pub(crate) fn step<W: FlightWorld>(
    actor: &mut VillagerState,
    world: &W,
    config: VillagerBehaviorConfig,
    base: u8,
    context: &mut StageContext<'_>,
    effects: &mut Vec<VillagerStepEffect>,
) {
    let mode = actor.stage_var_2;
    match mode {
        1..=9 => {
            let descriptor = context.descriptor(actor.stage_var_1);
            let point = descriptor
                .filter(|descriptor| {
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
                })
                .map(|descriptor| descriptor.point);
            movement(
                actor,
                world,
                config,
                base,
                point,
                matches!(mode, 3..=5),
                context,
                effects,
            );
            if matches!(mode, 3 | 4)
                && actor.phase & 7 == 0
                && let Some(descriptor) = descriptor
            {
                let target = stage_target(actor, descriptor, context);
                let target = if mode == 4 {
                    target.and_then(|target| context.target(target.view.target_entity_index))
                } else {
                    target
                };
                if let Some(target) = target
                    && distance_3d_wrapped(actor.position, target.view.position)
                        <= u32::from(config.engagement_range)
                {
                    actor.target_entity_index = Some(target.view.source_entity_index);
                    actor.stage_var_2 = 10;
                    engage(actor, base, config);
                }
            }
            if mode == 2
                && actor.action == base + 7
                && actor.stage_var_2 == 2
                && actor.source_byte_39 != 0
                && actor.phase.is_multiple_of(config.behavior_cadence as u8)
            {
                let mut best = None;
                let mut distance = u32::MAX;
                let range = u32::from(config.engagement_range)
                    .wrapping_mul(u32::from(config.engagement_range));
                for slot in context.players {
                    let Some(target) = context
                        .target(*slot)
                        .filter(|row| row.view.control_flags & 0x20 == 0)
                    else {
                        continue;
                    };
                    // 1DBF0 promotes X subtraction, but narrows Y to a signed word.
                    let dx = i32::from(target.view.position.x) - i32::from(actor.position.x);
                    let dy =
                        i32::from(target.view.position.y.wrapping_sub(actor.position.y) as i16);
                    let d = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                    let angle = heading_for_positions(actor.position, target.view.position) as i16;
                    if d <= range
                        && d < distance
                        && angle_distance(i32::from(actor.yaw), i32::from(angle))
                            < config.separation_radius
                    {
                        best = Some(*slot);
                        distance = d;
                    }
                }
                if let Some(slot) = best {
                    actor.target_entity_index = Some(slot);
                    actor.stage_var_2 = 10;
                    engage(actor, base, config);
                }
            }
        }
        10 => engage(actor, base, config),
        12 => {
            let parent = context.target(actor.stage.parent);
            if parent.is_none_or(|parent| parent.view.life < 0) {
                disable_villager(actor, effects);
            }
            actor.position = parent.map_or(Position::default(), |parent| parent.view.position);
            actor.position.z = actor.position.z.max(0);
            actor.yaw = parent.map_or(0, |parent| parent.yaw);
            actor.roll = parent.map_or(0, |parent| parent.roll);
        }
        13 | 16 => {
            let parent = context
                .target(actor.stage.parent)
                .filter(|row| row.view.life >= 0 && row.view.control_flags & 0x400 == 0);
            if parent.is_none() {
                actor.stage.remaining = 0;
            } else if mode == 13 {
                actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
            }
            if actor.stage.remaining <= 0 {
                context.effects.push(StageEffect::Expire {
                    slot: actor.entity_index,
                    position: actor.position,
                    summoned: mode == 13,
                });
                disable_villager(actor, effects);
                return;
            }
            let mut target = actor
                .target_entity_index
                .and_then(|slot| context.target(slot));
            if target.is_some_and(|row| row.view.life <= 0 || row.view.control_flags & 0x400 != 0) {
                actor.target_entity_index = None;
                target = None;
            }
            if target.is_none()
                && actor.phase & 7 == 0
                && let Some(parent) = parent
            {
                let mut distance = u32::MAX;
                for slot in context.players {
                    let Some(candidate) = context.target(*slot).filter(|candidate| {
                        matches!(candidate.view.model, 0 | 1)
                            && candidate.view.source_id != parent.view.source_id
                    }) else {
                        continue;
                    };
                    let dx = i32::from(
                        candidate
                            .view
                            .position
                            .x
                            .wrapping_sub(parent.view.position.x) as i16,
                    );
                    let dy = i32::from(
                        candidate
                            .view
                            .position
                            .y
                            .wrapping_sub(parent.view.position.y) as i16,
                    );
                    let d = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
                    if d < distance {
                        distance = d;
                        target = Some(candidate);
                    }
                }
                actor.target_entity_index = target.map(|row| row.view.source_entity_index);
            }
            if let Some(target) = target {
                target_movement(actor, world, config, base, context, effects);
                if actor.phase & 7 == 0
                    && distance_3d_wrapped(actor.position, target.view.position)
                        < u32::from(config.engagement_range)
                {
                    actor.action = base + 2;
                }
            } else {
                actor.target_entity_index = (actor.stage.parent != 0).then_some(actor.stage.parent);
                target_movement(actor, world, config, base, context, effects);
                actor.target_entity_index = None;
                actor.stage.remaining = actor.stage.remaining.wrapping_sub(4);
            }
        }
        14 => {
            actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
            let parent = context
                .target(actor.stage.parent)
                .filter(|row| row.view.life > 0);
            if actor.stage.remaining <= 0 || parent.is_none() {
                actor.stage_var_2 = 10;
                actor.target_entity_index = None;
                actor.stage.parent = 0;
                actor.stage_var_1 = actor.stage.relation as i8;
                flash(actor, context, true);
                return;
            }
            flash(actor, context, false);
            let parent = parent.expect("live Alliance parent");
            let previous = actor.target_entity_index;
            actor.target_entity_index = Some(parent.view.source_entity_index);
            target_movement(actor, world, config, base, context, effects);
            actor.target_entity_index = (parent.view.target_entity_index != 0)
                .then_some(parent.view.target_entity_index)
                .or((parent.view.current_attacker != 0).then_some(parent.view.current_attacker))
                .or(previous);
            if let Some(target) = actor
                .target_entity_index
                .and_then(|slot| context.target(slot))
            {
                if target.view.life <= 0
                    || target.view.control_flags & 0x400 != 0
                    || (target.links.parent == actor.stage.parent && target.stage_mode == 14)
                {
                    actor.target_entity_index = None;
                } else if distance_3d_wrapped(actor.position, target.view.position)
                    < u32::from(config.engagement_range)
                {
                    actor.action = base + 2;
                    context.effects.push(StageEffect::OwnerFeedback {
                        owner: actor.stage.parent,
                        selector: 24,
                    });
                }
            }
        }
        17 => {
            moved(actor, world, config, effects);
            if actor.life < 0 {
                disable_villager(actor, effects);
                return;
            }
            if let Some(target) = actor
                .target_entity_index
                .and_then(|slot| context.target(slot))
                .filter(|row| row.view.life >= 0 && row.view.control_flags & 0x400 == 0)
            {
                actor.roll = heading_for_positions(actor.position, target.view.position) as i16;
                actor.actual_speed = actor.actual_speed.wrapping_sub(8);
                if actor.actual_speed > 16 {
                    return;
                }
            }
            actor.stage_var_2 = 16;
        }
        // Source default is an actual no-op, including modes 0, 11 and 15.
        _ => {}
    }
}
