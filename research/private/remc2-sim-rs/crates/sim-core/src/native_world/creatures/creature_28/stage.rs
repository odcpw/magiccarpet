//! Model28's source `sub_2B7B0 -> sub_1D5D0(224)` StageVar callback.

use super::{
    Actor, CallbackEffects, DamageResult, Frame, SOURCE_ATTACK_ACTION, SOURCE_PROPERTIES, distance,
    distance_xy, move_actor, next_rng, update_roam_roll,
};
use crate::Position;
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{RuntimeRelation, RuntimeSnapshot, RuntimeStageVariable};

/// Bind the authored `sub_12100` route into this source row through the shared
/// StageVar adapter. The row stays the route's authoritative state carrier.
pub(crate) fn bind_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    super::super::stage_route::bind_source(arena, runtime, slot, index)
}

/// Reconcile StageVar state against the frozen model28 membership before the
/// source callback scan, matching `UpdateEntities_57730`'s `sub_12500` pass.
pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slots: &[u16],
) {
    super::super::stage_route::prepare_source_pass(arena, runtime, slots);
}

fn route(actor: &Actor, runtime: &RuntimeSnapshot) -> Result<RuntimeStageVariable, String> {
    runtime
        .stage_variables
        .get(usize::from(actor.route_index))
        .copied()
        .ok_or_else(|| {
            format!(
                "model28 action231 has no StageVar at index{}",
                actor.route_index
            )
        })
}

fn point(stage: RuntimeStageVariable) -> Option<Position> {
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

fn live(frame: &Frame<'_>, slot: u16) -> Option<super::Target> {
    frame
        .target(slot)
        .filter(|target| target.life >= 0 && target.flags & 0x400 == 0)
}

fn live_hostile(frame: &Frame<'_>, slot: u16) -> Option<super::Target> {
    frame
        .target(slot)
        .filter(|target| target.life > 0 && target.flags & 0x400 == 0)
}

fn stage_target(
    actor: &mut Actor,
    frame: &Frame<'_>,
    stage: RuntimeStageVariable,
) -> Option<super::Target> {
    if stage.flags & 2 == 0 {
        let slot = match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        };
        return live(frame, slot);
    }

    if let Some(target) = live(frame, actor.route_relation) {
        return Some(target);
    }
    let shared = if stage.flags & 1 != 0 {
        frame
            .same_model
            .iter()
            .filter_map(|slot| frame.target(*slot))
            .find(|peer| {
                peer.slot != actor.slot
                    && peer.route_index == actor.route_index
                    && peer.route_relation != 0
                    && live(frame, peer.route_relation).is_some()
            })
            .map(|peer| peer.route_relation)
    } else {
        None
    };
    actor.route_relation = shared.unwrap_or(0);
    if actor.route_relation == 0 {
        let model = match stage.relation {
            RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u8,
            _ => return None,
        };
        let nearest = frame
            .targets
            .iter()
            .filter(|target| target.model == model && target.living())
            .min_by_key(|target| distance_xy(actor.position, target.position))
            .map_or(0, |target| target.slot);
        actor.route_relation = nearest;
    }
    live(frame, actor.route_relation)
}

fn enter_target_action(actor: &mut Actor) {
    actor.action = if SOURCE_PROPERTIES.behavior_flags & 8 != 0 {
        SOURCE_ATTACK_ACTION + 4
    } else {
        SOURCE_ATTACK_ACTION
    };
}

fn take_damage(actor: &mut Actor, frame: &Frame<'_>) -> bool {
    match super::consume(actor, frame) {
        DamageResult::Killed => {
            actor.action = super::SOURCE_DEATH_ACTION;
            true
        }
        DamageResult::Hit => {
            let other = frame.target(actor.attacker);
            let hit_parent =
                matches!(actor.route_mode, 13 | 14 | 16) && actor.attacker == actor.parent;
            if !hit_parent
                && other.is_none_or(|target| {
                    target.class != super::SOURCE_CLASS || target.model != super::SOURCE_MODEL
                })
            {
                if !matches!(actor.route_mode, 13 | 14 | 16) {
                    actor.route_mode = 10;
                }
                actor.target = actor.attacker;
                enter_target_action(actor);
            }
            false
        }
        DamageResult::None => false,
    }
}

fn move_on_route(actor: &mut Actor, frame: &Frame<'_>, target: Option<super::Target>) {
    let _ = move_actor(actor, frame.terrain);
    if actor.source_counter & 7 != 0 || actor.flags & 0x40000 != 0 {
        return;
    }
    if let Some(target) = target {
        actor.roll = crate::heading_for_positions(actor.position, target.position) as i16;
        if actor.source_counter & 63 == 0 {
            update_roam_roll(actor);
        }
        let extent = i32::from(actor.half_width.max(0)) + i32::from(target.half_width.max(0));
        if i32::from(actor.position.x.abs_diff(target.position.x)) < extent
            && i32::from(actor.position.y.abs_diff(target.position.y)) < extent
        {
            actor.roll = crate::heading_for_positions(target.position, actor.position) as i16;
        }
    } else if actor.source_counter & 15 == 0 {
        actor.roll = actor
            .roll
            .wrapping_add((next_rng(actor) % 0x71 + 142) as i16)
            & 2047;
    }
    let peer_extent = u16::try_from(actor.half_width.max(0)).unwrap_or(u16::MAX);
    if let Some(peer) = frame
        .same_model
        .iter()
        .filter_map(|slot| frame.target(*slot))
        .find(|peer| {
            peer.slot != actor.slot
                && peer.source_id != actor.source_id
                && peer.living()
                && actor.position.x.abs_diff(peer.position.x) < peer_extent
                && actor.position.y.abs_diff(peer.position.y) < peer_extent
        })
    {
        actor.roll = crate::heading_for_positions(peer.position, actor.position) as i16;
    }
}

fn model0_or1_near_parent(actor: Actor, frame: &Frame<'_>, parent: u16) -> Option<super::Target> {
    let parent = frame.target(parent)?;
    let parent_id = parent.source_id;
    frame
        .targets
        .iter()
        .copied()
        .filter(|target| {
            target.slot != actor.slot
                && target.source_id != parent_id
                && matches!(target.model, 0 | 1)
                && target.life > 0
                && target.flags & 0x400 == 0
        })
        .min_by_key(|target| distance_xy(parent.position, target.position))
}

fn transparency_mode(actor: Actor, remaining: i16) -> u8 {
    let source_counter = actor.source_counter;
    if remaining <= actor.route_duration.wrapping_sub(16)
        && remaining >= 16
        && source_counter & 0x1c == 16
    {
        source_counter & 3
    } else if source_counter & 1 != 0 {
        0
    } else {
        2
    }
}

fn apply_transparency(flags: u8, mode: u8) -> u8 {
    match mode {
        0 => flags & !6,
        1 | 3 => (flags | 2) & !4,
        2 => (flags & !6) | 4,
        _ => flags,
    }
}

fn set_child_transparency(
    actor: Actor,
    frame: &Frame<'_>,
    mode: u8,
    effects: &mut CallbackEffects,
) {
    let mut child_slot = actor.child_head;
    for _ in 0..=frame.targets.len() {
        let Some(child) = frame.target(child_slot) else {
            break;
        };
        let child_flags = (child.flags >> 24) as u8;
        effects
            .child_fade_flags
            .push((child.slot, apply_transparency(child_flags, mode)));
        child_slot = child.next;
    }
}

fn mode14_transparency(
    actor: &mut Actor,
    frame: &Frame<'_>,
    remaining: i16,
    effects: &mut CallbackEffects,
) {
    let mode = transparency_mode(*actor, remaining);
    let high = (actor.flags >> 24) as u8;
    actor.flags = (actor.flags & 0x00ff_ffff) | (u32::from(apply_transparency(high, mode)) << 24);
    set_child_transparency(*actor, frame, mode, effects);
}

fn movement_and_target(actor: &mut Actor, frame: &Frame<'_>, stage: RuntimeStageVariable) {
    if take_damage(actor, frame) {
        return;
    }
    let destination = point(stage);
    let _ = move_actor(actor, frame.terrain);
    if actor.source_counter & 7 == 0 && actor.flags & 0x40000 == 0 {
        if let Some(destination) = destination {
            actor.roll = crate::heading_for_positions(actor.position, destination) as i16;
            if actor.source_counter & 63 == 0 {
                update_roam_roll(actor);
            }
        }
        if let Some(peer) = frame
            .same_model
            .iter()
            .filter_map(|slot| frame.target(*slot))
            .find(|peer| {
                peer.slot != actor.slot
                    && peer.source_id != actor.source_id
                    && peer.living()
                    && actor.position.x.abs_diff(peer.position.x)
                        < u16::try_from(actor.half_width.max(0)).unwrap_or(u16::MAX)
                    && actor.position.y.abs_diff(peer.position.y)
                        < u16::try_from(actor.half_width.max(0)).unwrap_or(u16::MAX)
            })
        {
            actor.roll = crate::heading_for_positions(peer.position, actor.position) as i16;
        }
    }
}

fn wander(actor: &mut Actor, frame: &Frame<'_>) {
    if SOURCE_PROPERTIES.behavior_flags & 2 != 0 {
        return;
    }
    let _ = move_actor(actor, frame.terrain);
    if actor.source_counter & 15 == 0 && actor.flags & 0x40000 == 0 {
        actor.roll = actor
            .roll
            .wrapping_add((next_rng(actor) % 0x71 + 142) as i16)
            & 2047;
    }
}

fn mode13_or16(
    actor: &mut Actor,
    frame: &Frame<'_>,
    stage_mode: u8,
    effects: &mut CallbackEffects,
) {
    let parent = live(frame, actor.parent);
    if parent.is_none() {
        actor.route_remaining = 0;
    } else if stage_mode == 13 {
        actor.route_remaining = actor.route_remaining.wrapping_sub(1);
    }
    if actor.route_remaining <= 0 {
        effects.stage_hide = true;
        effects.stage_mana_expiry = true;
        effects.stage_mana_mode13 = stage_mode == 13;
        return;
    }

    if live_hostile(frame, actor.target).is_none() {
        actor.target = 0;
        if actor.source_counter & 7 == 0 {
            let target = model0_or1_near_parent(*actor, frame, actor.parent);
            actor.target = target.map_or(0, |target| target.slot);
        }
    }
    let follow = live_hostile(frame, actor.target).or(parent);
    if take_damage(actor, frame) {
        return;
    }
    if let Some(target) = follow {
        let _ = move_actor(actor, frame.terrain);
        actor.roll = crate::heading_for_positions(actor.position, target.position) as i16;
        if actor.target != 0
            && actor.source_counter & 7 == 0
            && live_hostile(frame, actor.target).is_some_and(|target| {
                distance(actor.position, target.position) < SOURCE_PROPERTIES.target_radius
            })
        {
            actor.action = SOURCE_ATTACK_ACTION;
        } else if actor.target == 0 {
            actor.route_remaining = actor.route_remaining.wrapping_sub(4);
        }
    } else {
        actor.route_remaining = 0;
    }
    if actor.route_remaining <= 0 {
        effects.stage_hide = true;
        effects.stage_mana_expiry = true;
        effects.stage_mana_mode13 = stage_mode == 13;
    }
}

pub(super) fn step(
    actor: &mut Actor,
    frame: &Frame<'_>,
    runtime: &RuntimeSnapshot,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    let mode = actor.route_mode;
    match mode {
        1 => movement_and_target(actor, frame, route(actor, runtime)?),
        2 => {
            let stage = route(actor, runtime)?;
            if let Some(destination) = point(stage) {
                let dx =
                    (i32::from(destination.x as i16) - i32::from(actor.position.x as i16)).abs();
                let dy =
                    (i32::from(destination.y as i16) - i32::from(actor.position.y as i16)).abs();
                if dx > 3072 || dy > 3072 {
                    movement_and_target(actor, frame, stage);
                } else if !take_damage(actor, frame) {
                    wander(actor, frame);
                    if actor.action == 231
                        && actor.target_scan_limit != 0
                        && SOURCE_PROPERTIES.scan_period != 0
                        && actor.source_counter % SOURCE_PROPERTIES.scan_period == 0
                        && let Some(target) = super::acquire_target(*actor, frame)
                    {
                        actor.target = target.slot;
                        actor.route_mode = 10;
                        enter_target_action(actor);
                    }
                }
            } else {
                let _ = take_damage(actor, frame);
            }
        }
        3 => {
            let stage = route(actor, runtime)?;
            if take_damage(actor, frame) {
                return Ok(());
            }
            let target = stage_target(actor, frame, stage);
            move_on_route(actor, frame, target);
            if actor.source_counter & 7 == 0
                && let Some(target) = target
            {
                if distance(actor.position, target.position) <= SOURCE_PROPERTIES.target_radius {
                    actor.target = target.slot;
                    actor.route_mode = 10;
                    enter_target_action(actor);
                }
            }
        }
        4 => {
            let stage = route(actor, runtime)?;
            if take_damage(actor, frame) {
                return Ok(());
            }
            let target = stage_target(actor, frame, stage);
            move_on_route(actor, frame, target);
            if actor.source_counter & 7 == 0
                && let Some(target) = target
                && target.route_target != 0xae02
                && let Some(next) = live(frame, target.route_target)
                && distance(actor.position, next.position) <= SOURCE_PROPERTIES.target_radius
            {
                actor.target = next.slot;
                actor.route_mode = 10;
                enter_target_action(actor);
            }
        }
        5 => {
            let stage = route(actor, runtime)?;
            if !take_damage(actor, frame) {
                let target = stage_target(actor, frame, stage);
                move_on_route(actor, frame, target);
            }
        }
        6..=9 => {
            if take_damage(actor, frame) {
                return Ok(());
            }
            wander(actor, frame);
        }
        10 => {
            enter_target_action(actor);
        }
        12 => {
            if let Some(parent) = frame.target(actor.parent).filter(|parent| parent.life >= 0) {
                actor.position = parent.position;
                actor.yaw = parent.yaw;
                actor.roll = parent.yaw;
            } else {
                effects.stage_hide = true;
            }
        }
        13 | 16 => mode13_or16(actor, frame, mode, effects),
        14 => {
            actor.route_remaining = actor.route_remaining.wrapping_sub(1);
            let remaining = actor.route_remaining;
            mode14_transparency(actor, frame, remaining, effects);
            let parent = live(frame, actor.parent).filter(|parent| parent.life > 0);
            if actor.route_remaining <= 0 || parent.is_none() {
                actor.route_index = actor.route_relation as u8;
                actor.route_mode = 10;
                actor.target = 0;
                actor.parent = 0;
            } else if !take_damage(actor, frame)
                && let Some(parent) = parent
            {
                let target = live(frame, actor.target)
                    .or_else(|| live(frame, parent.route_target))
                    .unwrap_or(parent);
                let _ = move_actor(actor, frame.terrain);
                actor.roll = crate::heading_for_positions(actor.position, target.position) as i16;
                if actor.source_counter & 7 == 0
                    && distance(actor.position, target.position) < SOURCE_PROPERTIES.target_radius
                {
                    actor.target = target.slot;
                    actor.action = SOURCE_ATTACK_ACTION;
                }
            }
        }
        17 => {
            let _ = move_actor(actor, frame.terrain);
            if actor.life < 0 {
                effects.stage_hide = true;
            } else if let Some(target) = live(frame, actor.target) {
                actor.roll = crate::heading_for_positions(actor.position, target.position) as i16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.route_mode = 16;
                }
            } else {
                actor.route_mode = 16;
            }
        }
        // sub_1D5D0 has no source callback for modes0,11, or15.
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_world::{RuntimeStageVariable, SessionTerrain};

    fn fixture() -> (RuntimeSnapshot, SessionTerrain) {
        let session = crate::native_world::tests::level_two_campaign_session();
        (
            session.runtime().clone(),
            session.canonical_terrain().clone(),
        )
    }

    fn actor(mode: u8) -> Actor {
        Actor {
            slot: 40,
            source_id: 40,
            action: 231,
            phase: 0,
            flags: 0x0800_0000 | 0x800,
            position: Position {
                x: 20_000,
                y: 20_000,
                z: 5_000,
            },
            rng: 1,
            yaw: 0,
            pitch: 0,
            half_width: 384,
            roll: 0,
            speed: 92,
            minimum_speed: 120,
            maximum_speed: 64,
            life: 8_000,
            max_life: 8_000,
            timer: 0,
            source_animation: 0,
            descriptor: 93,
            subspell_index: 2_000,
            target: 11,
            child_head: 0,
            parent: 10,
            route_index: 1,
            route_mode: mode,
            route_relation: 3,
            route_remaining: 32,
            route_duration: 32,
            attacker: 0,
            killer: 0,
            pending_damage: 0,
            pending_attacker: 0,
            target_scan_limit: 64,
            source_counter: 8,
            visual: super::super::SOURCE_IDLE_VISUAL,
            animation_frame: 0,
            animation_limit: 4,
        }
    }

    fn target(slot: u16, model: u8, position: Position) -> super::super::Target {
        super::super::Target {
            slot,
            source_id: slot,
            class: 5,
            model,
            action: 231,
            phase: 0,
            flags: 0,
            life: 8_000,
            position,
            yaw: 0,
            half_width: 384,
            next: 0,
            attacker: 0,
            killer: 0,
            route_index: 0,
            route_relation: 0,
            route_target: 0,
        }
    }

    #[test]
    fn stagevar231_routes_a_normal_actor_toward_its_authored_point() {
        let (mut runtime, terrain) = fixture();
        let destination = Position {
            x: 30_000,
            y: 28_000,
            z: 0,
        };
        runtime.stage_variables[1] = RuntimeStageVariable {
            kind: 1,
            flags: 0,
            word_2: 0,
            relation: RuntimeRelation::AxisScalar(
                u32::from(destination.x) | (u32::from(destination.y) << 16),
            ),
        };
        let targets = [];
        let frame = Frame {
            terrain: &terrain,
            targets: &targets,
            same_model: &[],
        };
        let mut actor = actor(1);
        let mut effects = CallbackEffects::default();

        step(&mut actor, &frame, &runtime, &mut effects).unwrap();

        assert_eq!(
            i32::from(actor.roll as u16),
            crate::heading_for_positions(actor.position, destination),
            "mode1 must turn toward the authored point on its eight-turn cadence"
        );
        assert!(!effects.stage_hide);
    }

    #[test]
    fn mode14_expiry_rebinds_the_next_route_and_clears_parent_and_target() {
        let (mut runtime, terrain) = fixture();
        runtime.stage_variables[1] = RuntimeStageVariable {
            kind: 14,
            flags: 0,
            word_2: 0,
            relation: RuntimeRelation::AxisScalar(0),
        };
        let targets = [target(10, 5, Position::default())];
        let frame = Frame {
            terrain: &terrain,
            targets: &targets,
            same_model: &[],
        };
        let mut actor = actor(14);
        actor.route_remaining = 1;
        actor.parent = 10;
        actor.target = 11;
        let mut effects = CallbackEffects::default();

        step(&mut actor, &frame, &runtime, &mut effects).unwrap();

        assert_eq!((actor.route_index, actor.route_mode), (3, 10));
        assert_eq!((actor.parent, actor.target), (0, 0));
        assert!(!effects.stage_hide);
    }

    #[test]
    fn mode13_parent_loss_expires_with_both_source_mana_sphere_producers() {
        let (mut runtime, terrain) = fixture();
        runtime.stage_variables[1] = RuntimeStageVariable {
            kind: 13,
            flags: 0,
            word_2: 0,
            relation: RuntimeRelation::AxisScalar(0),
        };
        let targets = [];
        let frame = Frame {
            terrain: &terrain,
            targets: &targets,
            same_model: &[],
        };
        let mut actor = actor(13);
        actor.parent = 99;
        actor.route_remaining = 24;
        let mut effects = CallbackEffects::default();

        step(&mut actor, &frame, &runtime, &mut effects).unwrap();

        assert_eq!(actor.route_remaining, 0);
        assert!(effects.stage_hide);
        assert!(effects.stage_mana_expiry);
        assert!(effects.stage_mana_mode13);
    }

    #[test]
    fn mode13_live_parent_and_no_hostile_target_continue_without_expiry() {
        let (mut runtime, terrain) = fixture();
        runtime.stage_variables[1] = RuntimeStageVariable {
            kind: 13,
            flags: 0,
            word_2: 0,
            relation: RuntimeRelation::AxisScalar(0),
        };
        let parent = target(
            10,
            5,
            Position {
                x: 22_000,
                y: 20_000,
                z: 5_000,
            },
        );
        let targets = [parent];
        let frame = Frame {
            terrain: &terrain,
            targets: &targets,
            same_model: &[],
        };
        let mut actor = actor(13);
        actor.parent = 10;
        actor.target = 0;
        actor.route_remaining = 24;
        let mut effects = CallbackEffects::default();

        step(&mut actor, &frame, &runtime, &mut effects).unwrap();

        assert_eq!(actor.route_remaining, 19);
        assert!(!effects.stage_hide);
        assert!(!effects.stage_mana_expiry);
    }

    #[test]
    fn attack_contact_uses_live_subspell_damage_and_source_identity() {
        let (runtime, terrain) = fixture();
        let mut actor = actor(1);
        actor.source_id = 901;
        actor.subspell_index = 37;
        actor.action = SOURCE_ATTACK_ACTION;
        actor.phase = 4;
        actor.timer = 8;
        actor.animation_limit = 16;
        actor.target = 11;
        let targets = [target(11, 5, actor.position)];
        let frame = Frame {
            terrain: &terrain,
            targets: &targets,
            same_model: &[],
        };

        let callback = super::super::step(&mut actor, &frame, &runtime, &[]).unwrap();
        let contact = callback
            .notices
            .melee
            .expect("source melee phase emits contact");

        assert_eq!(contact.target_slot, 11);
        assert_eq!(contact.damage, 37);
        assert_eq!(contact.attacker_source_id, 901);
    }
}
