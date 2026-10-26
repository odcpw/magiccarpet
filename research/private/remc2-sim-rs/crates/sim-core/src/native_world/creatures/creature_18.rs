//! Source class-5/model-18 behavior (pinned remc2 `sub_24E20..sub_25550`).
//!
//! The adapter owns the source row and allocations. This module mutates a
//! callback-local actor and returns ordered producer intents.

use crate::combat::actors::{consume_damage, inherit_killer, inherit_linked_life};
use crate::level_definition::CREATURE18_CLASS_PROPERTIES as RULES;
use crate::level1_mission::projectile_guidance_angles;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, RuntimeSnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;

#[cfg(test)]
mod tests;

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 18;
pub(crate) const DESCRIPTOR: u16 = 86;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Creature18State {
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

/// `sub_4C590` after the source allocator has supplied a fixed slot and seed.
pub(crate) fn construct(
    slot: u16,
    generation: u32,
    seed: u16,
    position: Position,
    phase_counter: &mut u8,
) -> Creature18State {
    let phase = *phase_counter;
    *phase_counter = phase_counter.wrapping_add(1);
    let rng = level2_next_rng16(seed);
    let angle = (rng & 2047).wrapping_sub(1) as i16;
    Creature18State {
        slot,
        generation,
        source_id: slot,
        player: 0,
        rng,
        active: true,
        action: 147,
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
        timer: 100,
        visual: 286,
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
    pub roll: i16,
    pub previous: u16,
    pub next: u16,
    pub target: u16,
    pub attacker: u16,
    pub killer: u16,
    pub linked_peer: u16,
    pub linked_child: u16,
    pub parent: u16,
    pub route_index: u8,
    pub route_mode: u8,
    pub route_relation: u16,
}

impl Target {
    fn alive(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }

    fn player(self) -> bool {
        self.class == 3 && matches!(self.model, 0 | 1)
    }
}

pub(crate) struct Creature18Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub runtime: &'a RuntimeSnapshot,
    pub targets: &'a [Target],
    pub class3: &'a [u16],
    pub model18: &'a [u16],
}

impl Creature18Context<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| self.targets.iter().find(|row| row.slot == slot).copied())
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectileIntent {
    pub target_class: u8,
    pub target_model: u8,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub source_id: u16,
    pub source_target: u16,
    pub subspell: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Creature18Effects {
    // Producer intents stay inside the family leaf. The shared scheduler sees
    // only successfully initialized fixed slots in the typed Fireball arena.
    projectiles: Vec<ProjectileIntent>,
    pub projectile_slots: Vec<u16>,
    pub linked_actions: Vec<(u16, u8)>,
    pub kill_credit: Option<u16>,
    pub kill: bool,
    pub route_hide: bool,
}

fn source_distance_3d(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn xy_squared(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn consume(actor: &mut Creature18State, ctx: &Creature18Context<'_>) -> u8 {
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

fn nearest_player(actor: &Creature18State, ctx: &Creature18Context<'_>) -> Option<Target> {
    let max_distance = RULES.target_range.saturating_mul(RULES.target_range);
    ctx.class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.alive() && target.player() && target.flags & 0x20 == 0)
        .filter(|target| {
            xy_squared(actor.position, target.position) <= max_distance
                && angle_distance(
                    i32::from(actor.yaw),
                    heading_for_positions(actor.position, target.position),
                ) < RULES.target_half_angle
        })
        .min_by_key(|target| xy_squared(actor.position, target.position))
}

fn random(actor: &mut Creature18State) -> u16 {
    actor.rng = level2_next_rng16(actor.rng);
    actor.rng
}

fn request_action(actor: &mut Creature18State, family: u8, phase: u8) {
    match (family, phase) {
        (0, 0) => actor.timer = i32::from(random(actor) % 400) + 400,
        (0, 1) => actor.timer = i32::from(random(actor) % 60) + 60,
        (1, _) => actor.timer = i32::from(random(actor) % 400) + 400,
        (2, 0) => actor.timer = i32::from(random(actor) % 200) + 200,
        (2, 1) => actor.timer = 10,
        (2, 2) => actor.timer = 12,
        (2, 3) => actor.timer = 14,
        _ => {}
    }
    if family <= 2 {
        actor.action = match family {
            0 => 144,
            1 => 145,
            _ => 146,
        };
        actor.attack_phase = phase;
    }
}

fn turn_toward(actor: &mut Creature18State, target: Position, limit: i16) {
    actor.roll = heading_for_positions(actor.position, target) as i16;
    let mut delta = actor.roll - actor.yaw;
    while delta > 1024 {
        delta -= 2048;
    }
    while delta < -1024 {
        delta += 2048;
    }
    actor.yaw = actor.yaw.wrapping_add(delta.clamp(-limit, limit)) & 2047;
}

fn settle(actor: &mut Creature18State, terrain: &SessionTerrain) {
    let ground = terrain.ground_height(actor.position);
    actor.position.z = ground.wrapping_add(RULES.ground_clearance);
}

fn move_ground(actor: &mut Creature18State, terrain: &SessionTerrain) {
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
    if motion.killed {
        actor.life = -1;
    }
}

fn source_damage(actor: &mut Creature18State, ctx: &Creature18Context<'_>) -> u8 {
    settle(actor, ctx.terrain);
    let result = consume(actor, ctx);
    if result == 1 {
        actor.target = actor.attacker;
    }
    result
}

fn fan(actor: &Creature18State, target: Target, effects: &mut Creature18Effects) {
    let (base_yaw, pitch) = projectile_guidance_angles(actor.position, target.position);
    for offset in RULES.fan_yaw_offsets {
        let mut position = actor.position;
        position.z = position.z.wrapping_add(RULES.fan_lift);
        effects.projectiles.push(ProjectileIntent {
            target_class: target.class,
            target_model: target.model,
            position,
            yaw: base_yaw.wrapping_add(offset) & 2047,
            pitch,
            source_id: actor.source_id,
            source_target: actor.target,
            subspell: RULES.projectile_damage,
        });
    }
}

fn prekill(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
) {
    let mut child_slot = actor.linked_child;
    for _ in 0..=ctx.targets.len() {
        let Some(child) = ctx.target(child_slot) else {
            break;
        };
        effects.linked_actions.push((child.slot, 149));
        inherit_killer(&mut actor.killer, child.killer);
        child_slot = child.linked_child;
    }
    actor.action = 149;
}

fn step_attack(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
) {
    let hit = source_damage(actor, ctx);
    if hit == 2 {
        actor.action = 148;
        return;
    }
    match actor.attack_phase {
        0 => {
            let Some(target) = ctx
                .target(actor.target)
                .filter(|target| target.alive() && target.player())
            else {
                actor.target = 0;
                request_action(actor, 0, 0);
                return;
            };
            turn_toward(actor, target.position, RULES.turn_response);
            if hit == 1 {
                actor.timer = actor.timer.wrapping_sub(47);
                if actor.timer < 0 {
                    request_action(actor, 2, 1);
                }
            } else {
                if random(actor) % 41 == 0 {
                    request_action(actor, 2, 1);
                } else {
                    actor.timer = actor.timer.wrapping_sub(1);
                    if actor.timer < 0 {
                        request_action(actor, 2, 2);
                    }
                }
            }
        }
        1 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                request_action(actor, 2, 2);
                return;
            }
            if actor.phase % RULES.source_scan_period == 0 {
                let Some(target) = ctx
                    .target(actor.target)
                    .filter(|target| target.alive() && target.player())
                else {
                    actor.target = 0;
                    request_action(actor, 2, 2);
                    return;
                };
                turn_toward(actor, target.position, RULES.turn_response);
                fan(actor, target, effects);
            }
        }
        2 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                request_action(actor, 2, 3);
            }
        }
        3 => {
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer < 0 {
                request_action(actor, 1, 0);
            } else if actor.timer >= 8 {
                actor.yaw = actor.yaw.wrapping_add(170) & 2047;
                actor.roll = actor.yaw;
            }
        }
        _ => request_action(actor, 0, 0),
    }
}

fn step_cooldown(actor: &mut Creature18State, ctx: &Creature18Context<'_>) {
    match source_damage(actor, ctx) {
        2 => actor.action = 148,
        1 => request_action(actor, 0, 1),
        _ => {
            actor.target = 0;
            move_ground(actor, ctx.terrain);
            actor.timer = actor.timer.wrapping_sub(1);
            if actor.timer <= 0 {
                request_action(actor, 0, 0);
            }
        }
    }
}

/// Runs one source callback. The scheduler owns the enabled guard, byte3E
/// increment, same-pass tail, and following-pass reclaim.
pub(crate) fn step(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
) -> Option<Creature18Effects> {
    let mut effects = Creature18Effects::default();
    match actor.action {
        144 => match source_damage(actor, ctx) {
            2 => actor.action = 148,
            1 => request_action(actor, 2, 0),
            _ if actor.attack_phase == 1 => {
                let Some(target) = ctx
                    .target(actor.target)
                    .filter(|target| target.alive() && target.player())
                else {
                    actor.target = 0;
                    request_action(actor, 0, 0);
                    return Some(effects);
                };
                if source_distance_3d(actor.position, target.position) < RULES.target_range {
                    turn_toward(actor, target.position, RULES.turn_response);
                    if random(actor) % 49 == 0 {
                        request_action(actor, 2, 0);
                    }
                } else {
                    actor.target = 0;
                    request_action(actor, 0, 0);
                }
            }
            _ => {
                if actor.timer > 0 {
                    actor.timer = actor.timer.wrapping_sub(1);
                    if actor.proximity != 0
                        && random(actor) & 1 == 0
                        && let Some(target) = nearest_player(actor, ctx)
                    {
                        actor.target = target.slot;
                        request_action(actor, 0, 1);
                    }
                    if actor.timer == 0 && actor.action == 144 {
                        request_action(actor, 1, 0);
                    }
                } else {
                    request_action(actor, 1, 0);
                }
            }
        },
        145 => step_cooldown(actor, ctx),
        146 => step_attack(actor, ctx, &mut effects),
        147 => request_action(actor, 0, 0),
        148 => prekill(actor, ctx, &mut effects),
        149 => effects.kill = actor.phase & 7 == 0,
        151 => {
            stage::step(actor, ctx, &mut effects);
            settle(actor, ctx.terrain);
            if actor.action == 146 {
                request_action(actor, 2, 0);
            }
        }
        _ => return None,
    }
    Some(effects)
}
