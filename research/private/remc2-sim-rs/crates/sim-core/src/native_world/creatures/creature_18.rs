//! Source class-5/model-18 behavior (pinned remc2 `sub_24E20..sub_25550`).
//!
//! The adapter owns the source row and allocations. This module mutates a
//! callback-local actor and returns ordered producer intents.

use crate::combat::actors::{consume_damage, inherit_killer, inherit_linked_life};
use crate::level_definition::CREATURE18_CLASS_PROPERTIES as RULES;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, RuntimeSnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;
use mc2::{DESCRIPTOR, PARTICLE};

#[cfg(test)]
mod tests;

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 18;

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
        // SetEvent144 derives mana from the constructor maximum before any
        // later authored health override.
        mana: RULES.max_life >> 1,
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
        visual: PARTICLE,
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
    pub half_width: i16,
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
        self.class == 3
    }
}

pub(crate) struct Creature18Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub runtime: &'a RuntimeSnapshot,
    pub targets: &'a [Target],
    pub class3: &'a [u16],
    pub model18: &'a [u16],
    pub families: &'a [Vec<u16>],
}

impl Creature18Context<'_> {
    fn source_address(&self, slot: u16) -> Option<Target> {
        self.targets.iter().find(|row| row.slot == slot).copied()
    }

    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| self.source_address(slot))
            .flatten()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Creature18Effects {
    // Producer intents stay inside the family leaf. The shared scheduler sees
    // only successfully initialized fixed slots in the typed Fireball arena.
    fan_target: Option<u16>,
    pub projectile_slots: Vec<u16>,
    pub linked_actions: Vec<(u16, u8)>,
    pub linked_blink_flags: Vec<(u16, u32)>,
    pub kill_credit: Option<u16>,
    pub earlier_feedback: Option<(u16, u8)>,
    pub feedback: Option<(u16, u8)>,
    pub population_decrements: u32,
    pub kill: bool,
    pub route_hide: bool,
    pub expire: bool,
    pub summon_expire: bool,
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
        // The class-3 list was frozen at pass start. sub_24E20 tests the
        // current 0x20 flag, but does not recheck life or drawing here; a
        // later firing callback performs its own target-lifetime check.
        .filter(|target| target.flags & 0x20 == 0)
        .filter(|target| {
            xy_squared(actor.position, target.position) <= max_distance
                && angle_distance(
                    i32::from(actor.yaw),
                    heading_for_positions(actor.position, target.position),
                ) < RULES.target_half_angle
        })
        .min_by_key(|target| xy_squared(actor.position, target.position))
        // The source checks the winning slot's current class only after the
        // scan, so a transformed nearer member must not select a farther one.
        .filter(|target| target.player())
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

fn move_ground(
    actor: &mut Creature18State,
    terrain: &SessionTerrain,
    effects: &mut Creature18Effects,
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
    if motion.movement_blocked {
        actor.control_flags |= 0x40000;
    } else {
        actor.control_flags &= !0x40000;
    }
    if motion.killed {
        actor.life = -1;
        effects.population_decrements = effects.population_decrements.wrapping_add(1);
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
            // sub_250B0 turns toward the source slot even before its later
            // phase-one lifetime check; slot zero is the source sentinel.
            let target = ctx.source_address(actor.target).unwrap_or_default();
            turn_toward(actor, target.position, RULES.turn_limit);
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
                let target = ctx.target(actor.target);
                let target = stage::alliance_target(actor, ctx, effects, target);
                let Some(target) = target.filter(|target| target.alive()) else {
                    request_action(actor, 2, 2);
                    return;
                };
                // sub_250B0 uses sub_58350(..., 5, 0x400) while firing;
                // its third argument is ignored and the fourth is the cap.
                turn_toward(actor, target.position, 1024);
                effects.fan_target = Some(target.slot);
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
        // sub_250B0 returns for unknown attack subphases.
        _ => {}
    }
}

fn step_cooldown(
    actor: &mut Creature18State,
    ctx: &Creature18Context<'_>,
    effects: &mut Creature18Effects,
) {
    match source_damage(actor, ctx) {
        2 => actor.action = 148,
        1 => request_action(actor, 0, 1),
        _ => {
            actor.target = 0;
            move_ground(actor, ctx.terrain, effects);
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
                // sub_24E20 leaves the tracking state untouched when its
                // target word is zero; it does not restart patrol or RNG.
                if actor.target == 0 {
                    return Some(effects);
                }
                let target = ctx.target(actor.target).unwrap_or_default();
                // sub_24E20 uses EuclideanDistXYZ_58490, whose source body
                // computes wrapped X/Y distance without the altitude term.
                if crate::legacy_sqrt(xy_squared(actor.position, target.position))
                    < RULES.target_range
                {
                    turn_toward(actor, target.position, RULES.turn_limit);
                    if random(actor) % 49 == 0 {
                        request_action(actor, 2, 0);
                    }
                } else {
                    actor.target = 0;
                    request_action(actor, 0, 0);
                }
            }
            _ if actor.attack_phase == 0 => {
                actor.timer = actor.timer.wrapping_sub(1);
                if actor.timer != 0 {
                    if actor.proximity != 0
                        && random(actor) & 1 == 0
                        && let Some(target) = nearest_player(actor, ctx)
                    {
                        actor.target = target.slot;
                        request_action(actor, 0, 1);
                    }
                } else {
                    request_action(actor, 1, 0);
                }
            }
            // sub_24E20 does not run patrol for nonzero phases other than 1.
            _ => {}
        },
        145 => step_cooldown(actor, ctx, &mut effects),
        146 => step_attack(actor, ctx, &mut effects),
        147 => request_action(actor, 0, 0),
        148 => prekill(actor, ctx, &mut effects),
        149 => effects.kill = actor.phase & 7 == 0,
        151 => {
            stage::step(actor, ctx, &mut effects);
            if !effects.expire {
                settle(actor, ctx.terrain);
                if actor.action == 146 {
                    request_action(actor, 2, 0);
                }
            }
        }
        _ => return None,
    }
    Some(effects)
}
