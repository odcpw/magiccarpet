//! Source class-5/model-17 behavior (pinned remc2 `sub_24860..sub_24DF0`).
//!
//! This module owns one callback-local actor value. The MC2 adapter is the
//! sole reader/writer of the source arena row and applies returned intents.

use crate::combat::actors::{consume_damage, inherit_killer, inherit_linked_life};
use crate::level_definition::CREATURE17_CLASS_PROPERTIES as RULES;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, RuntimeSnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;

#[cfg(test)]
mod tests;

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 17;
pub(crate) const DESCRIPTOR: u16 = 85;
pub(crate) const STRIKE_DESCRIPTOR: u16 = 87;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Creature17State {
    pub slot: u16,
    pub generation: u32,
    pub source_id: u16,
    pub player: u16,
    pub rng: u16,
    pub active: bool,
    pub action: u8,
    pub control_flags: u32,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub max_life: i32,
    pub life: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub constructor_subspell: u16,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub attacker: u16,
    pub killer: u16,
    pub target: u16,
    pub linked_peer: u16,
    pub linked_child: u16,
    pub previous: u16,
    pub next: u16,
    pub parent: u16,
    pub phase: u8,
    pub proximity: u8,
    pub attack_phase: u8,
    pub descriptor: u16,
    pub timer: i32,
    pub visual: u16,
    pub animation_frame: u8,
    pub animation_limit: u8,
    pub half_width: i16,
    pub half_height: i16,
    pub vertical_extent: i16,
    pub route_index: u8,
    pub route_mode: u8,
    pub route_relation: u16,
    pub route_remaining: i16,
    pub route_duration: i16,
}

/// `sub_4C460` after the source allocator has supplied a fixed slot and seed.
pub(crate) fn construct(
    slot: u16,
    generation: u32,
    seed: u16,
    position: Position,
    phase_counter: &mut u8,
) -> Creature17State {
    let phase = *phase_counter;
    *phase_counter = phase_counter.wrapping_add(1);
    let rng = level2_next_rng16(seed);
    let angle = (rng & 2047).wrapping_sub(1) as i16;
    Creature17State {
        slot,
        generation,
        source_id: slot,
        player: 0,
        rng,
        active: true,
        action: 137,
        control_flags: 8,
        position,
        yaw: angle,
        pitch: angle,
        roll: angle,
        fov: 0,
        speed: RULES.minimum_speed,
        minimum_speed: RULES.minimum_speed,
        maximum_speed: RULES.maximum_speed,
        max_life: RULES.max_life,
        life: RULES.max_life,
        mana: RULES.initial_mana,
        max_mana: 0,
        constructor_subspell: RULES.constructor_subspell,
        pending_damage: 0,
        pending_attacker: 0,
        attacker: 0,
        killer: 0,
        target: 0,
        linked_peer: 0,
        linked_child: 0,
        previous: 0,
        next: 0,
        parent: 0,
        phase,
        proximity: 64,
        attack_phase: 0,
        descriptor: DESCRIPTOR,
        timer: 0,
        visual: 285,
        animation_frame: 0,
        animation_limit: 0,
        half_width: RULES.collision_half_width,
        half_height: RULES.collision_half_height,
        vertical_extent: 0,
        route_index: 0,
        route_mode: 0,
        route_relation: 0,
        route_remaining: 0,
        route_duration: 0,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Target {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub life: i32,
    pub flags: u32,
    pub position: Position,
    pub yaw: i16,
    pub speed: i16,
    pub maximum_speed: i16,
    pub previous: u16,
    pub next: u16,
    pub linked_peer: u16,
    pub linked_child: u16,
    pub parent: u16,
    pub route_index: u8,
    pub route_mode: u8,
    pub route_relation: u16,
    pub target: u16,
    pub attacker: u16,
    pub killer: u16,
}

impl Target {
    fn alive(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }

    fn player(self) -> bool {
        self.class == 3 && matches!(self.model, 0 | 1)
    }
}

pub(crate) struct Creature17Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub runtime: &'a RuntimeSnapshot,
    pub targets: &'a [Target],
    pub class3: &'a [u16],
    pub model17: &'a [u16],
}

impl Creature17Context<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| self.targets.iter().find(|row| row.slot == slot).copied())
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectileIntent {
    pub target: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Creature17Effects {
    /// Source action138's unconditional PrepareEventSound at callback entry.
    pub sound: Option<u8>,
    /// Source `sub_1CED0` contact mailbox; the adapter applies it once.
    pub melee: Option<(u16, i32, u16)>,
    /// Source `sub_1D0E0` class-9/model-20 projectile producer.
    pub projectile: Option<ProjectileIntent>,
    /// Fixed arena slot allocated by the leaf adapter for that producer.
    pub projectile_slot: Option<u16>,
    /// Qualified player returned by the source pre-kill chain walk.
    pub kill_credit: Option<u16>,
    pub linked_actions: Vec<(u16, u8)>,
    pub linked_updates: Vec<LinkedUpdate>,
    pub population_decrements: u32,
    pub kill: bool,
    pub route_hide: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LinkedUpdate {
    pub slot: u16,
    pub action: u8,
    pub target: u16,
    pub linked_peer: u16,
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn xy_squared(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn consume(actor: &mut Creature17State, ctx: &Creature17Context<'_>) -> u8 {
    let mut hit = u8::from(
        consume_damage(
            &mut actor.life,
            &mut actor.pending_damage,
            &mut actor.pending_attacker,
            &mut actor.attacker,
        )
        .is_some(),
    );
    let mut child_slot = actor.linked_child;
    for _ in 0..=ctx.targets.len() {
        let Some(child) = ctx.target(child_slot) else {
            break;
        };
        if inherit_linked_life(
            &mut actor.life,
            &mut actor.attacker,
            child.life,
            child.attacker,
        ) {
            hit = 1;
            break;
        }
        child_slot = child.linked_child;
    }
    if actor.life < 0 {
        actor.killer = actor.attacker;
        2
    } else {
        hit
    }
}

fn nearest_candidate(actor: &Creature17State, ctx: &Creature17Context<'_>) -> Option<Target> {
    let max_distance = RULES.target_range.saturating_mul(RULES.target_range);
    let mut nearest = None;
    let mut shortest = u32::MAX;
    for target in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.slot != actor.slot && target.flags & 0x20 == 0)
    {
        let squared = xy_squared(actor.position, target.position);
        if squared <= max_distance
            && squared < shortest
            && angle_distance(
                i32::from(actor.yaw),
                heading_for_positions(actor.position, target.position),
            ) < RULES.target_half_angle
        {
            nearest = Some(target);
            shortest = squared;
        }
    }
    nearest
}

fn nearest_unlinked_peer(
    actor: &Creature17State,
    ctx: &Creature17Context<'_>,
    toward_actor: bool,
) -> Option<Target> {
    let max_distance = RULES.target_range.saturating_mul(RULES.target_range);
    let mut nearest = None;
    let mut shortest = u32::MAX;
    for peer in ctx
        .model17
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|peer| peer.slot != actor.slot && peer.linked_peer == 0)
    {
        let squared = xy_squared(actor.position, peer.position);
        let heading = if toward_actor {
            heading_for_positions(peer.position, actor.position)
        } else {
            heading_for_positions(actor.position, peer.position)
        };
        if squared <= max_distance
            && squared < shortest
            && angle_distance(i32::from(actor.yaw), heading) < RULES.target_half_angle
        {
            nearest = Some(peer);
            shortest = squared;
        }
    }
    nearest
}

fn jitter(actor: &mut Creature17State) {
    let first = level2_next_rng16(actor.rng);
    actor.rng = level2_next_rng16(first);
    let sign = 2 * (i32::from(first % 157) / 79) - 1;
    let delta = sign * (i32::from(actor.rng & 255) + 85);
    actor.roll = actor.roll.wrapping_add(delta as i16) & 2047;
}

fn settle(actor: &mut Creature17State, terrain: &SessionTerrain) {
    crate::ground_creature_motion::relax_to_terrain(
        &mut actor.position,
        terrain,
        RULES.ground_clearance,
        RULES.descent,
    );
}

fn move_actor(
    actor: &mut Creature17State,
    terrain: &SessionTerrain,
    effects: &mut Creature17Effects,
) {
    if actor.control_flags & 0x800 != 0 {
        actor.control_flags &= !0x800;
        return;
    }
    let motion = crate::ground_creature_motion::step_ground_creature_motion(
        actor.position,
        actor.yaw,
        actor.roll,
        actor.speed,
        terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: actor.half_width,
            half_height: actor.half_height,
            ground_clearance: RULES.ground_clearance,
            descent: RULES.descent,
            turn_limit: RULES.turn_limit,
            terrain_mask: RULES.terrain_mask,
            maximum_tile_slope: RULES.maximum_slope,
        },
    );
    actor.position = motion.position;
    actor.yaw = motion.yaw;
    actor.control_flags = if motion.movement_blocked {
        actor.control_flags | 0x40000
    } else {
        actor.control_flags & !0x40000
    };
    if motion.killed && RULES.behavior_flags & 1 != 0 {
        actor.life = -1;
        effects.population_decrements = effects.population_decrements.wrapping_add(1);
    }
}

fn follow_heading(actor: &mut Creature17State, target: Position) {
    actor.roll = heading_for_positions(actor.position, target) as i16;
}

fn choose_idle_peer(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    toward_actor: bool,
) -> bool {
    if RULES.behavior_flags & 4 != 0 {
        return false;
    }
    let Some(peer) = nearest_unlinked_peer(actor, ctx, toward_actor) else {
        return false;
    };
    actor.linked_peer = peer.slot;
    actor.action = 139;
    true
}

fn acquire_target(actor: &mut Creature17State, ctx: &Creature17Context<'_>) -> bool {
    if actor.proximity == 0 {
        return false;
    }
    let Some(target) = nearest_candidate(actor, ctx) else {
        return false;
    };
    actor.target = target.slot;
    actor.action = 138;
    true
}

fn reset_from_bad_target(actor: &mut Creature17State) {
    actor.descriptor = DESCRIPTOR;
    actor.target = 0;
    actor.action = 137;
    actor.speed = actor.minimum_speed;
}

fn pursue(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    effects.sound = Some(58);
    let hit = consume(actor, ctx);
    if hit == 2 {
        actor.action = 140;
        return;
    }
    if hit == 1 {
        actor.target = actor.attacker;
        settle(actor, ctx.terrain);
        return;
    }
    move_actor(actor, ctx.terrain, effects);
    let Some(target) = ctx
        .target(actor.target)
        .filter(|target| target.alive() && target.player())
    else {
        reset_from_bad_target(actor);
        return;
    };
    if actor.phase & 3 == 0 {
        follow_heading(actor, target.position);
        if let Some(peer) = ctx
            .model17
            .iter()
            .filter_map(|slot| ctx.target(*slot))
            .find(|peer| {
                peer.slot != actor.slot
                    && peer.source_id != actor.source_id
                    && actor.position.x.abs_diff(peer.position.x) < actor.half_width.max(0) as u16
                    && actor.position.y.abs_diff(peer.position.y) < actor.half_width.max(0) as u16
            })
        {
            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
        }
    }
    if actor.phase % RULES.source_scan_period != 0 {
        return;
    }
    let range = distance(actor.position, target.position);
    if range >= RULES.target_range {
        actor.action = 137;
    } else if actor.attack_phase == 0 {
        if range >= RULES.projectile_min_range {
            effects.projectile = Some(ProjectileIntent {
                target: target.slot,
            });
        } else {
            actor.roll = heading_for_positions(actor.position, target.position) as i16;
            actor.yaw = actor.roll;
            actor.descriptor = STRIKE_DESCRIPTOR;
            actor.timer = 0;
            actor.attack_phase = 2;
            actor.speed = actor.minimum_speed.saturating_mul(3);
            return;
        }
    }

    if matches!(actor.attack_phase, 2 | 3) {
        let time = actor.timer;
        let vertical = if time <= 4 {
            i32::from(RULES.strike_vertical_step) >> time.clamp(0, 15)
        } else if time >= 8 {
            -i32::from(RULES.strike_vertical_step)
        } else {
            -(i32::from(RULES.strike_vertical_step) >> (8 - time))
        };
        actor.timer = actor.timer.wrapping_add(1);
        if actor.speed > actor.maximum_speed {
            actor.speed = actor.speed.saturating_sub(RULES.strike_speed_decay);
        }
        let ground = ctx.terrain.ground_height(actor.position);
        if i32::from(actor.position.z).wrapping_add(vertical) <= i32::from(ground) {
            actor.attack_phase = 4;
            actor.timer = RULES.strike_duration;
        } else {
            actor.position.z = actor.position.z.wrapping_add(vertical as i16);
            if actor.attack_phase != 3
                && distance(actor.position, target.position) < RULES.melee_range
            {
                effects.melee = Some((target.slot, i32::from(RULES.melee_damage), actor.source_id));
                actor.attack_phase = 3;
            }
        }
    } else if actor.attack_phase == 4 {
        let remaining = actor.timer;
        actor.timer = actor.timer.wrapping_sub(1);
        if remaining == RULES.strike_duration {
            actor.descriptor = DESCRIPTOR;
            actor.speed = actor.maximum_speed;
        } else if actor.timer == 0 {
            actor.attack_phase = 0;
            actor.speed = actor.minimum_speed;
        }
    }
}

fn step_linked(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    if actor.linked_peer == 0 {
        actor.linked_peer = 0;
        actor.action = 137;
        return;
    }
    let peer = ctx
        .target(actor.linked_peer)
        .filter(|peer| peer.alive() && peer.class == SOURCE_CLASS && peer.model == SOURCE_MODEL);
    let hit = consume(actor, ctx);
    if hit != 0 {
        if let Some(peer) = peer {
            effects.linked_updates.push(LinkedUpdate {
                slot: peer.slot,
                action: 138,
                target: actor.attacker,
                linked_peer: 0,
            });
        }
        actor.linked_peer = 0;
        actor.target = actor.attacker;
        actor.action = if hit == 2 { 140 } else { 138 };
        settle(actor, ctx.terrain);
        return;
    }
    move_actor(actor, ctx.terrain, effects);
    let Some(peer) = peer else {
        actor.linked_peer = 0;
        actor.action = 137;
        return;
    };
    if actor.phase % RULES.source_scan_period != 0 {
        return;
    }
    match peer.action {
        136 | 137 => {
            follow_heading(actor, peer.position);
            if let Some(close_peer) = ctx
                .model17
                .iter()
                .filter_map(|slot| ctx.target(*slot))
                .find(|other| {
                    other.slot != actor.slot
                        && other.source_id != actor.source_id
                        && actor.position.x.abs_diff(other.position.x) < 256
                        && actor.position.y.abs_diff(other.position.y) < 256
                })
            {
                actor.roll = heading_for_positions(close_peer.position, actor.position) as i16;
            }
            actor.speed = peer.maximum_speed.saturating_add(peer.speed);
        }
        138 => {
            actor.target = peer.target;
            actor.linked_peer = 0;
            actor.action = 138;
        }
        139 => {
            actor.linked_peer = peer.linked_peer;
            if let Some(next_peer) = ctx.target(actor.linked_peer).filter(|next| next.alive()) {
                follow_heading(actor, next_peer.position);
                actor.speed = peer.maximum_speed.saturating_add(peer.speed);
            } else {
                actor.linked_peer = 0;
                actor.action = 137;
            }
        }
        _ => {
            actor.linked_peer = 0;
            actor.action = 137;
        }
    }
}

fn prekill(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
    effects: &mut Creature17Effects,
) {
    let mut child_slot = actor.linked_child;
    for _ in 0..=ctx.targets.len() {
        let Some(child) = ctx.target(child_slot) else {
            break;
        };
        effects.linked_actions.push((child.slot, 141));
        inherit_killer(&mut actor.killer, child.killer);
        child_slot = child.linked_child;
    }
    actor.action = 141;
}

/// Runs the family callback only. The scheduler owns the enabled guard,
/// source byte3E tail and following-pass reclaim.
pub(crate) fn step(
    actor: &mut Creature17State,
    ctx: &Creature17Context<'_>,
) -> Option<Creature17Effects> {
    let mut effects = Creature17Effects::default();
    let callback_action = actor.action;
    match callback_action {
        136 => {
            match consume(actor, ctx) {
                2 => actor.action = 140,
                1 => {
                    actor.target = actor.attacker;
                    actor.action = 138;
                }
                _ if actor.phase % RULES.source_scan_period == 0 => {
                    choose_idle_peer(actor, ctx, true);
                }
                _ => {}
            }
            settle(actor, ctx.terrain);
        }
        137 => {
            let hit = consume(actor, ctx);
            if hit == 2 {
                actor.action = 140;
            } else if hit == 1 {
                actor.target = actor.attacker;
                actor.action = 138;
                settle(actor, ctx.terrain);
            } else {
                move_actor(actor, ctx.terrain, &mut effects);
                if actor.phase % RULES.source_scan_period == 0 {
                    jitter(actor);
                    if actor.proximity != 0 && !acquire_target(actor, ctx) {
                        choose_idle_peer(actor, ctx, false);
                    }
                }
            }
        }
        138 => pursue(actor, ctx, &mut effects),
        139 => step_linked(actor, ctx, &mut effects),
        140 => prekill(actor, ctx, &mut effects),
        141 => effects.kill = actor.phase & 7 == 0,
        143 => {
            stage::step(actor, ctx, &mut effects);
            if actor.action == 138 {
                actor.attack_phase = 0;
            }
        }
        _ => return None,
    }
    if matches!(callback_action, 136 | 137) && actor.action == 138 {
        if !ctx
            .target(actor.target)
            .is_some_and(|target| target.class == 3 && matches!(target.model, 0 | 1))
        {
            actor.target = 0;
        }
        actor.attack_phase = 0;
    }
    Some(effects)
}
