//! Model28's source `sub_2B7B0 -> sub_1D5D0(224)` StageVar callback.

use super::{
    Actor, CallbackEffects, DamageResult, Frame, SOURCE_ATTACK_ACTION, SOURCE_PROPERTIES, Target,
};
use crate::campaign::source_arena::SourceArena;
use crate::native_world::{RuntimeRelation, RuntimeSnapshot, RuntimeStageVariable};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) fn bind_authored(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) -> Result<(), String> {
    super::super::stage_route::bind_source(arena, runtime, slot, index)
}

pub(crate) fn prepare_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    targets: crate::native_world::source_target::Context<'_>,
    slots: &[u16],
) -> Result<(), String> {
    super::super::stage_route::prepare_source_pass(arena, runtime, targets, slots)
}

fn required<T>(target: Target, value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| {
        format!(
            "model28 current source target {} lacks {field}",
            target.slot
        )
    })
}

fn current(actor: &Actor, frame: &Frame<'_>, slot: u16) -> Option<Target> {
    frame.actor_target(actor, slot)
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

fn point(stage: RuntimeStageVariable) -> Result<Position, String> {
    let packed = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value,
        _ => return Err("model28 point route has no source coordinate".into()),
    };
    Ok(Position {
        x: packed as u16,
        y: (packed >> 16) as u16,
        z: 0,
    })
}

fn attack(actor: &mut Actor) -> Result<(), String> {
    if SOURCE_PROPERTIES.behavior_flags & 8 != 0 {
        return Err("model28 configured flee flag selects unsupported source action230".into());
    }
    actor.action = SOURCE_ATTACK_ACTION;
    Ok(())
}

/// The generic StageVar helpers make their own family/parent hit decision;
/// source2B9A0's unconditional target assignment belongs only to action226.
fn damage(actor: &mut Actor, frame: &Frame<'_>) -> Result<DamageResult, String> {
    let target = actor.target;
    let result = super::consume(actor, frame)?;
    actor.target = target;
    Ok(result)
}

fn movement(actor: &mut Actor, frame: &Frame<'_>, effects: &mut CallbackEffects) {
    let _ = super::move_for_callback(actor, frame.terrain, effects);
}

fn relax(actor: &mut Actor, frame: &Frame<'_>) {
    let ground = frame.terrain.ground_height(actor.position);
    if actor.position.z > ground {
        actor.position.z = actor.position.z.wrapping_add(SOURCE_PROPERTIES.descent);
    }
    let floor = ground.wrapping_add(SOURCE_PROPERTIES.ground_clearance);
    if actor.position.z <= floor {
        actor.position.z = floor;
    }
}

fn different_family(actor: &Actor, frame: &Frame<'_>, slot: u16) -> bool {
    let target = if slot == 0 {
        frame.sentinel
    } else {
        current(actor, frame, slot)
    };
    target.is_none_or(|target| {
        (target.class, target.model) != (super::SOURCE_CLASS, super::SOURCE_MODEL)
    })
}

fn hit_reaction(actor: &mut Actor, frame: &Frame<'_>) -> Result<(), String> {
    if different_family(actor, frame, actor.attacker) {
        actor.target = actor.attacker;
        actor.route_mode = 10;
        attack(actor)?;
    }
    relax(actor, frame);
    Ok(())
}

fn random_wander(actor: &mut Actor) {
    let rng = super::next_rng(actor);
    actor.roll = actor.roll.wrapping_add((rng % 113 + 142) as i16) & 2047;
}

/// Source1E1C0, used by modes6..9 without any StageVar descriptor read.
fn wander(
    actor: &mut Actor,
    frame: &Frame<'_>,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    match damage(actor, frame)? {
        DamageResult::None if SOURCE_PROPERTIES.behavior_flags & 2 == 0 => {
            movement(actor, frame, effects);
            if actor.source_counter & 15 == 0 && actor.flags & 0x40000 == 0 {
                random_wander(actor);
            }
        }
        DamageResult::Hit => hit_reaction(actor, frame)?,
        DamageResult::Killed => actor.action = super::SOURCE_DEATH_ACTION,
        _ => {}
    }
    Ok(())
}

/// Source1DDA0's peer overlap promotes each coordinate from signed i16.
fn point_step(
    actor: &mut Actor,
    frame: &Frame<'_>,
    runtime: &RuntimeSnapshot,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    match damage(actor, frame)? {
        DamageResult::None => {
            movement(actor, frame, effects);
            if actor.source_counter & 7 == 0 {
                let destination = point(route(actor, runtime)?)?;
                if actor.flags & 0x40000 == 0 {
                    actor.roll = heading_for_positions(actor.position, destination) as i16;
                    if actor.source_counter & 63 == 0 {
                        super::update_roam_roll(actor);
                    }
                }
                for &slot in frame.same_model {
                    let Some(peer) = current(actor, frame, slot) else {
                        continue;
                    };
                    if peer.source_id != actor.source_id
                        && (i32::from(actor.position.x as i16) - i32::from(peer.position.x as i16))
                            .abs()
                            < i32::from(actor.half_width)
                        && (i32::from(actor.position.y as i16) - i32::from(peer.position.y as i16))
                            .abs()
                            < i32::from(actor.half_width)
                    {
                        actor.roll = heading_for_positions(peer.position, actor.position) as i16;
                        break;
                    }
                }
            }
        }
        DamageResult::Hit => hit_reaction(actor, frame)?,
        DamageResult::Killed => actor.action = super::SOURCE_DEATH_ACTION,
    }
    Ok(())
}

fn guard(
    actor: &mut Actor,
    frame: &Frame<'_>,
    runtime: &RuntimeSnapshot,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    let destination = point(route(actor, runtime)?)?;
    let away = (i32::from(destination.x as i16) - i32::from(actor.position.x as i16)).abs() > 3072
        || (i32::from(destination.y as i16) - i32::from(actor.position.y as i16)).abs() > 3072;
    if away {
        point_step(actor, frame, runtime, effects)?;
    } else {
        wander(actor, frame, effects)?;
    }
    if actor.action != 231
        || actor.route_mode != 2
        || actor.target_scan_limit == 0
        || SOURCE_PROPERTIES.scan_period == 0
        || actor.source_counter % SOURCE_PROPERTIES.scan_period != 0
    {
        return Ok(());
    }
    let range = SOURCE_PROPERTIES
        .target_radius
        .wrapping_mul(SOURCE_PROPERTIES.target_radius);
    let mut selected = None;
    let mut nearest = u32::MAX;
    for &slot in frame.class3 {
        let Some(target) = current(actor, frame, slot) else {
            continue;
        };
        // DBF0 promotes X to int, but stores Y's difference in signed word.
        let dx = i32::from(target.position.x) - i32::from(actor.position.x);
        let dy = i32::from(target.position.y.wrapping_sub(actor.position.y) as i16);
        let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
        if squared <= range
            && target.flags & 0x20 == 0
            && squared < nearest
            && angle_distance(
                i32::from(actor.yaw),
                heading_for_positions(actor.position, target.position),
            ) < SOURCE_PROPERTIES.target_half_angle
        {
            nearest = squared;
            selected = Some(target.slot);
        }
    }
    if let Some(slot) = selected {
        actor.target = slot;
        actor.route_mode = 10;
        attack(actor)?;
    }
    Ok(())
}

fn family_slots(frame: &Frame<'_>, model: u16) -> Result<Vec<u16>, String> {
    if let Some(families) = frame.families {
        return families
            .get(usize::from(model))
            .cloned()
            .ok_or_else(|| format!("model28 StageVar names absent source family{model}"));
    }
    if model == u16::from(super::SOURCE_MODEL) {
        return Ok(frame.same_model.to_vec());
    }
    Ok(frame
        .targets
        .iter()
        .filter(|target| target.class == 5 && u16::from(target.model) == model)
        .map(|target| target.slot)
        .collect())
}

fn bind_leader(
    actor: &Actor,
    frame: &Frame<'_>,
    stage: RuntimeStageVariable,
) -> Result<u16, String> {
    if stage.flags & 1 != 0 {
        for &slot in frame.same_model {
            let Some(peer) = current(actor, frame, slot) else {
                continue;
            };
            if required(peer, peer.relations.stage_index, "StageVar index")? == actor.route_index {
                let relation = required(peer, peer.relations.stage_relation, "StageVar relation")?;
                if relation != 0 {
                    return Ok(relation);
                }
            }
        }
    }
    let model = match stage.relation {
        RuntimeRelation::AxisScalar(value) | RuntimeRelation::Scalar(value) => value as u16,
        _ => return Err("model28 family route has no source model".into()),
    };
    let mut nearest = u32::MAX;
    let mut selected = 0;
    for slot in family_slots(frame, model)? {
        let Some(target) = current(actor, frame, slot) else {
            continue;
        };
        let squared = super::distance_xy(actor.position, target.position);
        if squared < nearest {
            nearest = squared;
            selected = slot;
        }
    }
    Ok(selected)
}

fn leader(
    actor: &mut Actor,
    frame: &Frame<'_>,
    stage: RuntimeStageVariable,
    bind: bool,
) -> Result<Option<Target>, String> {
    let slot = if stage.flags & 2 != 0 {
        if actor.route_relation == 0 && bind {
            actor.route_relation = bind_leader(actor, frame, stage)?;
        }
        actor.route_relation
    } else {
        match stage.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        }
    };
    Ok(current(actor, frame, slot))
}

fn avoid_peers(actor: &mut Actor, frame: &Frame<'_>) {
    for &slot in frame.same_model {
        let Some(peer) = current(actor, frame, slot) else {
            continue;
        };
        if peer.source_id != actor.source_id
            && (i32::from(actor.position.x) - i32::from(peer.position.x)).abs()
                < i32::from(actor.half_width)
            && (i32::from(actor.position.y) - i32::from(peer.position.y)).abs()
                < i32::from(actor.half_width)
        {
            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
            break;
        }
    }
}

/// D8C0's peer avoidance runs before the independent leader-separation check.
/// D700/D7C0 then run their own attack tail even after damage changes action.
fn follow(
    actor: &mut Actor,
    frame: &Frame<'_>,
    runtime: &RuntimeSnapshot,
    mode: u8,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    match damage(actor, frame)? {
        DamageResult::None => {
            movement(actor, frame, effects);
            if actor.source_counter & 7 == 0 {
                let stage = route(actor, runtime)?;
                if let Some(target) = leader(actor, frame, stage, true)? {
                    if actor.flags & 0x40000 == 0 {
                        actor.roll = heading_for_positions(actor.position, target.position) as i16;
                        if actor.source_counter & 63 == 0 {
                            super::update_roam_roll(actor);
                        }
                    }
                    avoid_peers(actor, frame);
                    let target = current(actor, frame, target.slot)
                        .expect("current leader remains addressed");
                    let width = i32::from(actor.half_width)
                        + i32::from(required(
                            target,
                            target.pitch_extent,
                            "leader collision width",
                        )?);
                    if (i32::from(actor.position.x) - i32::from(target.position.x)).abs() < width
                        && (i32::from(actor.position.y) - i32::from(target.position.y)).abs()
                            < width
                    {
                        actor.roll = heading_for_positions(target.position, actor.position) as i16;
                    }
                } else if actor.source_counter & 15 == 0 && actor.flags & 0x40000 == 0 {
                    random_wander(actor);
                }
            }
        }
        DamageResult::Hit => hit_reaction(actor, frame)?,
        DamageResult::Killed => actor.action = super::SOURCE_DEATH_ACTION,
    }
    if mode != 5 && actor.source_counter & 7 == 0 {
        if let Some(leader) = leader(actor, frame, route(actor, runtime)?, false)? {
            let target = if mode == 3 {
                Some(leader)
            } else {
                let slot = required(leader, leader.relations.target, "leader target")?;
                if slot == 0xae02 {
                    None
                } else {
                    current(actor, frame, slot)
                }
            };
            if let Some(target) = target
                && super::distance(actor.position, target.position)
                    <= SOURCE_PROPERTIES.target_radius
            {
                actor.target = target.slot;
                actor.route_mode = 10;
                attack(actor)?;
            }
        }
    }
    Ok(())
}

fn parent_pose(
    actor: &mut Actor,
    frame: &Frame<'_>,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    let parent = current(actor, frame, actor.parent);
    effects.stage_hide = parent.is_none_or(|parent| parent.life < 0);
    let parent = parent.or(frame.sentinel);
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
    frame.terrain.set_source_predicted_position(position);
    actor.position = position;
    actor.yaw = yaw;
    actor.roll = roll;
    Ok(())
}

fn escort(
    actor: &mut Actor,
    frame: &Frame<'_>,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    let action = actor.action;
    match damage(actor, frame)? {
        DamageResult::None => {
            movement(actor, frame, effects);
            if actor.source_counter & 7 == 0
                && let Some(target) = current(actor, frame, actor.target)
            {
                if actor.flags & 0x40000 == 0 {
                    actor.roll = heading_for_positions(actor.position, target.position) as i16;
                    if actor.source_counter & 63 == 0 {
                        super::update_roam_roll(actor);
                    }
                }
                avoid_peers(actor, frame);
            }
        }
        DamageResult::Hit => {
            movement(actor, frame, effects);
            if actor.attacker != actor.parent && different_family(actor, frame, actor.attacker) {
                actor.target = actor.attacker;
                attack(actor)?;
                effects.notices.feedback.push((actor.parent, 19));
            }
        }
        DamageResult::Killed => {
            actor.action = action;
            actor.route_remaining = 1;
        }
    }
    Ok(())
}

fn summon(
    actor: &mut Actor,
    frame: &Frame<'_>,
    mode: u8,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    let parent_slot = actor.parent;
    let parent = current(actor, frame, parent_slot)
        .filter(|parent| parent.life >= 0 && parent.flags & 0x400 == 0);
    if parent.is_none() {
        actor.route_remaining = 0;
    } else if mode == 13 {
        actor.route_remaining = actor.route_remaining.wrapping_sub(1);
    }
    if actor.route_remaining <= 0 {
        effects.stage_mana_expiry = true;
        effects.stage_mana_mode13 = mode == 13;
        effects.stage_hide = true;
        return Ok(());
    }
    let parent = parent.expect("current summon parent checked");
    let mut target = current(actor, frame, actor.target);
    if target.is_some_and(|target| target.life <= 0 || target.flags & 0x400 != 0) {
        actor.target = 0;
        target = None;
    }
    if target.is_none() && actor.source_counter & 7 == 0 {
        let mut selected = None;
        let mut nearest = u32::MAX;
        for &slot in frame.class3 {
            let Some(candidate) = current(actor, frame, slot) else {
                continue;
            };
            if candidate.source_id != parent.source_id && matches!(candidate.model, 0 | 1) {
                let squared = super::distance_xy(parent.position, candidate.position);
                if squared < nearest {
                    nearest = squared;
                    selected = Some(candidate);
                }
            }
        }
        target = selected;
        actor.target = target.map_or(0, |target| target.slot);
    }
    if let Some(target) = target {
        escort(actor, frame, effects)?;
        if actor.source_counter & 7 == 0
            && let Some(target) = current(actor, frame, target.slot)
            && super::distance(actor.position, target.position) < SOURCE_PROPERTIES.target_radius
        {
            actor.action = SOURCE_ATTACK_ACTION;
        }
    } else {
        actor.target = parent_slot;
        escort(actor, frame, effects)?;
        actor.target = 0;
        actor.route_remaining = actor.route_remaining.wrapping_sub(4);
    }
    Ok(())
}

fn blink(
    actor: &mut Actor,
    frame: &Frame<'_>,
    effects: &mut CallbackEffects,
    clear: bool,
) -> Result<(), String> {
    let remaining = actor.route_remaining;
    let phase = if clear {
        0
    } else if i32::from(remaining) <= i32::from(actor.route_duration) - 16 && remaining >= 16 {
        if actor.source_counter & 0x1c == 16 {
            actor.source_counter & 3
        } else {
            0
        }
    } else if actor.source_counter & 1 != 0 {
        0
    } else {
        2
    };
    let bits = match phase {
        1 | 3 => 2,
        2 => 4,
        _ => 0,
    };
    actor.flags = (actor.flags & !0x0600_0000) | (u32::from(bits) << 24);
    let mut slot = actor.child_head;
    for _ in 0..1000 {
        let Some(child) = current(actor, frame, slot) else {
            break;
        };
        let flags = ((child.flags >> 24) as u8 & !6) | bits;
        effects.child_fade_flags.push((child.slot, flags));
        slot = required(child, child.relations.next, "linked child")?;
    }
    Ok(())
}

fn alliance(
    actor: &mut Actor,
    frame: &Frame<'_>,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    actor.route_remaining = actor.route_remaining.wrapping_sub(1);
    if actor.route_remaining > 0 {
        blink(actor, frame, effects, false)?;
    }
    let parent_slot = actor.parent;
    let parent = (actor.route_remaining > 0)
        .then(|| current(actor, frame, parent_slot))
        .flatten()
        .filter(|parent| parent.life > 0);
    if parent.is_none() {
        actor.route_mode = 10;
        actor.target = 0;
        actor.parent = 0;
        actor.route_index = actor.route_relation as u8;
        blink(actor, frame, effects, true)?;
        return Ok(());
    }
    let previous = actor.target;
    actor.target = parent_slot;
    escort(actor, frame, effects)?;
    let parent = current(actor, frame, parent_slot).expect("current Alliance parent checked");
    let selected = required(parent, parent.relations.target, "parent target")?;
    let selected = if selected == 0 {
        required(parent, parent.relations.attacker, "parent attacker")?
    } else {
        selected
    };
    actor.target = if selected == 0 { previous } else { selected };
    if let Some(target) = current(actor, frame, actor.target) {
        let sibling = required(target, target.relations.parent, "target parent")? == parent_slot
            && required(target, target.relations.stage_mode, "target StageVar mode")? == 14;
        if sibling || target.life <= 0 || target.flags & 0x400 != 0 {
            actor.target = 0;
        } else if super::distance(actor.position, target.position) < SOURCE_PROPERTIES.target_radius
        {
            actor.action = SOURCE_ATTACK_ACTION;
            effects.notices.feedback.push((parent_slot, 24));
        }
    }
    Ok(())
}

pub(super) fn step(
    actor: &mut Actor,
    frame: &Frame<'_>,
    runtime: &RuntimeSnapshot,
    effects: &mut CallbackEffects,
) -> Result<(), String> {
    match actor.route_mode {
        1 => point_step(actor, frame, runtime, effects)?,
        2 => guard(actor, frame, runtime, effects)?,
        mode @ 3..=5 => follow(actor, frame, runtime, mode, effects)?,
        6..=9 => wander(actor, frame, effects)?,
        10 => attack(actor)?,
        12 => parent_pose(actor, frame, effects)?,
        mode @ (13 | 16) => summon(actor, frame, mode, effects)?,
        14 => alliance(actor, frame, effects)?,
        17 => {
            movement(actor, frame, effects);
            if actor.life < 0 {
                effects.stage_hide = true;
            } else if let Some(target) =
                current(actor, frame, actor.target).filter(|target| target.living())
            {
                actor.roll = heading_for_positions(actor.position, target.position) as i16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.route_mode = 16;
                }
            } else {
                actor.route_mode = 16;
            }
        }
        // All other mode bytes are literal source1D5D0 returns. The enclosing
        // sub2B7B0 still resets state0 if a reached helper selected action226.
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
            parent: 0,
            route_mode: 0,
            relations: crate::native_world::source_target::Relations {
                target: Some(0),
                attacker: Some(0),
                parent: Some(0),
                stage_mode: Some(0),
                stage_index: Some(0),
                stage_relation: Some(0),
                next: Some(0),
            },
            source_yaw: Some(0),
            source_roll: Some(0),
            pitch_extent: Some(384),
        }
    }

    fn frame<'a>(
        terrain: &'a SessionTerrain,
        targets: &'a [Target],
        same_model: &'a [u16],
    ) -> Frame<'a> {
        Frame {
            terrain,
            targets,
            current_target: None,
            sentinel: None,
            class3: &[],
            families: None,
            attack_animation_count: None,
            same_model,
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
        let frame = frame(&terrain, &targets, &[]);
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
        let frame = frame(&terrain, &targets, &[]);
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
        let frame = frame(&terrain, &targets, &[]);
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
        let frame = frame(&terrain, &targets, &[]);
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
    fn configured_stage_reads_descriptors_only_on_source_reached_paths() {
        let (runtime, terrain) = fixture();
        let ctx = frame(&terrain, &[], &[]);
        for mode in [0, 11, 15, 18, 255] {
            let mut row = actor(mode);
            row.route_index = 255;
            let before = row;
            let mut effects = CallbackEffects::default();
            step(&mut row, &ctx, &runtime, &mut effects).unwrap();
            assert_eq!(
                row, before,
                "source default mode{mode} does not read the table"
            );
        }
        let mut attack = actor(10);
        attack.route_index = 255;
        attack.target = 123;
        step(&mut attack, &ctx, &runtime, &mut CallbackEffects::default()).unwrap();
        assert_eq!((attack.action, attack.target), (SOURCE_ATTACK_ACTION, 123));
        for mode in [1, 5, 6, 7, 8, 9] {
            let mut row = actor(mode);
            row.route_index = 255;
            row.source_counter = 1;
            step(&mut row, &ctx, &runtime, &mut CallbackEffects::default()).unwrap();
            assert_eq!(
                row.action, 231,
                "no descriptor access before the helper cadence"
            );
        }
        for mode in [6, 7, 8, 9] {
            let mut row = actor(mode);
            row.route_index = 255;
            row.source_counter = 16;
            step(&mut row, &ctx, &runtime, &mut CallbackEffects::default()).unwrap();
            let rng = 1_u16.wrapping_mul(9377).wrapping_add(9439);
            assert_eq!((row.rng, row.roll), (rng, (rng % 113 + 142) as i16));
        }
    }

    #[test]
    fn configured_follow_fatal_helper_still_runs_the_wrapper_attack_tail() {
        let (mut runtime, terrain) = fixture();
        runtime.stage_variables[1] = RuntimeStageVariable {
            kind: 3,
            flags: 0,
            word_2: 0,
            relation: RuntimeRelation::LiveEntity(10),
        };
        let position = actor(3).position;
        let mut leader = target(10, 5, position);
        leader.life = -1;
        leader.flags = 0x400;
        leader.relations.target = Some(11);
        let targets = [leader, target(11, 5, position)];
        let ctx = frame(&terrain, &targets, &[]);
        for mode in [3, 4] {
            let mut row = actor(mode);
            row.pending_damage = 9000;
            row.pending_attacker = 11;
            let mut effects = CallbackEffects::default();
            step(&mut row, &ctx, &runtime, &mut effects).unwrap();
            assert_eq!(row.life, -1000);
            assert_eq!(
                (row.action, row.route_mode, row.target),
                (SOURCE_ATTACK_ACTION, 10, if mode == 3 { 10 } else { 11 }),
                "the wrapper follows its current pointer even after the fatal helper"
            );
            assert!(
                !effects.population_decrement,
                "mailbox death does not run movement"
            );
        }
    }

    #[test]
    fn configured_follow_separates_from_leader_after_peer_and_blocked_heading() {
        let (mut runtime, terrain) = fixture();
        runtime.stage_variables[1] = RuntimeStageVariable {
            kind: 5,
            flags: 0,
            word_2: 0,
            relation: RuntimeRelation::LiveEntity(10),
        };
        let mut row = actor(5);
        row.flags |= 0x40000;
        let leader = target(
            10,
            5,
            Position {
                x: 20_100,
                ..row.position
            },
        );
        let peer = target(
            42,
            super::super::SOURCE_MODEL,
            Position {
                x: 19_900,
                ..row.position
            },
        );
        let targets = [leader, peer];
        let ctx = frame(&terrain, &targets, &[42]);
        step(&mut row, &ctx, &runtime, &mut CallbackEffects::default()).unwrap();
        assert_eq!(
            row.roll,
            heading_for_positions(leader.position, row.position) as i16
        );
        assert_ne!(
            row.roll,
            heading_for_positions(peer.position, row.position) as i16,
            "peer overlap must not return before leader separation"
        );
    }

    #[test]
    fn configured_parent_pose_uses_current_dead_parent_and_actual_sentinel() {
        let (runtime, terrain) = fixture();
        let stale = target(10, 5, Position::default());
        let mut parent = target(
            10,
            5,
            Position {
                x: 31_000,
                y: 32_000,
                z: 900,
            },
        );
        parent.life = -1;
        parent.source_yaw = Some(123);
        parent.source_roll = Some(456);
        let lookup = |slot| (slot == 10).then_some(parent);
        let stale_targets = [stale];
        let ctx = Frame {
            current_target: Some(&lookup),
            ..frame(&terrain, &stale_targets, &[])
        };
        let mut row = actor(12);
        let mut effects = CallbackEffects::default();
        step(&mut row, &ctx, &runtime, &mut effects).unwrap();
        assert!(effects.stage_hide);
        assert_eq!(
            (row.position, row.yaw, row.roll),
            (parent.position, 123, 456)
        );
        assert_eq!(terrain.source_predicted_position(), parent.position);

        let mut sentinel = parent;
        sentinel.slot = 0;
        sentinel.position.z = -4;
        let ctx = Frame {
            sentinel: Some(sentinel),
            ..frame(&terrain, &[], &[])
        };
        let mut row = actor(12);
        row.parent = 0;
        let mut effects = CallbackEffects::default();
        step(&mut row, &ctx, &runtime, &mut effects).unwrap();
        assert!(effects.stage_hide);
        assert_eq!(
            (row.position, row.yaw, row.roll),
            (
                Position {
                    z: 0,
                    ..sentinel.position
                },
                123,
                456
            )
        );
        assert_eq!(terrain.source_predicted_position(), row.position);
    }

    #[test]
    fn configured_summon_fatal_lifetime_and_no_target_tail_keep_source_order() {
        let (runtime, terrain) = fixture();
        let parent = target(10, 5, actor(16).position);
        let targets = [parent];
        let ctx = frame(&terrain, &targets, &[]);
        let mut row = actor(16);
        row.source_counter = 1;
        row.target = 0;
        row.pending_damage = 9000;
        row.pending_attacker = 11;
        let mut effects = CallbackEffects::default();
        step(&mut row, &ctx, &runtime, &mut effects).unwrap();
        assert_eq!(
            (row.life, row.action, row.target, row.route_remaining),
            (-1000, 231, 0, -3)
        );
        assert!(
            !effects.stage_mana_expiry,
            "expiry was tested before helper timer1 and its minus4 tail"
        );
        step(&mut row, &ctx, &runtime, &mut effects).unwrap();
        assert!(effects.stage_mana_expiry && !effects.stage_mana_mode13);
    }

    #[test]
    fn configured_alliance_duration_word_is_unsigned_before_promotion() {
        let (runtime, terrain) = fixture();
        let parent = target(10, 5, actor(14).position);
        let targets = [parent];
        let ctx = frame(&terrain, &targets, &[]);
        let mut row = actor(14);
        row.target = 0;
        row.source_counter = 0;
        row.route_remaining = 33;
        row.route_duration = u16::MAX;
        step(&mut row, &ctx, &runtime, &mut CallbackEffects::default()).unwrap();
        assert_eq!((row.route_remaining, row.flags & 0x0600_0000), (32, 0));
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
        actor.source_animation = 16; // Source word2C is the actual header count.
        actor.animation_limit = 2; // Particle mode metadata is not that count.
        actor.route_duration = actor.yaw as u16;
        actor.target = 11;
        let targets = [target(11, 5, actor.position)];
        let frame = frame(&terrain, &targets, &[]);

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
