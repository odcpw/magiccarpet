//! Model21 ordinary creature behavior (MC2 actions168..175).
//!
//! The source arena row remains authoritative. This module borrows one row for
//! a callback; its leaf adapter commits the Fireball and common death effects.

use super::archer::ArcherStageRuntime as StageRuntime;
use super::creature_15::DefenderTarget as Target;
use crate::level_definition::CREATURE21_CLASS_PROPERTIES as RULES;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, SessionTerrain, level2_next_rng16,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;
use mc2::{JUMP_SOUND, SOURCE_VISUALS};
#[cfg(test)]
use mc2::DESCRIPTOR;
#[cfg(test)]
pub(crate) mod tests;

pub(crate) const SOURCE_MODEL: u8 = 21;

/// Source action174 has no callback in the pinned table; descriptor96 has bit8
/// clear. Keep it rejected rather than turning it into a successful empty step.
pub(crate) const fn enabled(action: u8) -> bool {
    matches!(action, 168..=173 | 175)
}

pub(crate) struct Actor<'a> {
    pub row: &'a mut EntitySnapshot,
}

impl Actor<'_> {
    fn word(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.row.record[at], self.row.record[at + 1]])
    }

    fn put(&mut self, at: usize, value: u16) {
        self.row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn target(&self) -> u16 {
        self.word(154)
    }

    fn set_target(&mut self, slot: u16) {
        self.put(154, slot);
    }

    fn attacker(&self) -> u16 {
        self.word(42)
    }

    fn set_action(&mut self, action: u8) {
        // sub_268F0 owns both ordinary action transitions.
        if action == 169 {
            self.row.record[71] = 64;
            self.set_target(0);
        } else if action == 170 {
            self.row.record[71] = 0;
        }
        self.row.set_source_action(action);
    }

    fn jump_state(&self) -> u8 {
        self.row.record[74]
    }

    fn set_jump_state(&mut self, state: u8) {
        self.row.record[74] = state;
    }

    fn stage_index(&self) -> u8 {
        self.row.record[76]
    }

    fn stage_mode(&self) -> u8 {
        self.row.record[77]
    }

    fn stage_parent(&self) -> u16 {
        self.word(44)
    }

    fn stage_relation(&self) -> u16 {
        self.word(78)
    }
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) struct Context<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    pub class3: &'a [u16],
    pub model21: &'a [u16],
    /// Frozen class5 family membership from the beginning of this source pass.
    pub families: &'a [Vec<u16>],
    pub runtime: &'a mut dyn StageRuntime,
}

impl Context<'_> {
    fn source_address(&self, slot: u16) -> Option<Target> {
        self.targets.iter().find(|target| target.slot == slot).copied()
    }

    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| self.source_address(slot))
            .flatten()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    /// A successful source sub_1CC20 request. The adapter must allocate its
    /// real source9/0 product before it applies target-side sub_5EF70.
    pub strike_target: Option<u16>,
    /// Player feedback published only after the leaf allocates the source
    /// Fireball and applies the target's `sub_5EF70` write.
    pub target_music: Option<u16>,
    pub water_splash: Option<Position>,
    /// The source resumes this tail against the current address after NewEvent.
    pub water_splash_tail: Option<(bool, WaterContinuation)>,
    pub sound: Option<u8>,
    pub sound_position: Option<Position>,
    pub sound_identity: Option<(u16, u8, u32)>,
    pub kill_credit: Option<u16>,
    pub earlier_feedback: Option<(u16, u8)>,
    pub feedback: Option<(u16, u8)>,
    pub linked_blink_flags: Vec<(u16, u32)>,
    pub prekill: bool,
    pub population_decrements: u32,
    pub kill: bool,
    pub expire: bool,
    pub summon_expire: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaterContinuation { None, Roam, AttackReset, Stage }

fn living(target: Target) -> bool {
    target.life >= 0 && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn xy(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn consume(actor: &mut Actor<'_>, ctx: &Context<'_>, linked: bool) -> u8 {
    let mut damage = i32::from_le_bytes(
        actor.row.record[98..102]
            .try_into()
            .expect("fixed source damage mailbox"),
    );
    let mut pending = actor.word(102);
    let mut attacker = actor.attacker();
    let mut life = actor.row.life;
    let mut hit = u8::from(
        crate::combat::actors::consume_damage(&mut life, &mut damage, &mut pending, &mut attacker)
            .is_some(),
    );

    let mut next = actor.word(56);
    for _ in 0..ctx.targets.len() {
        let Some(child) = ctx.target(next) else { break };
        if crate::combat::actors::inherit_linked_life(
            &mut life,
            &mut attacker,
            child.life,
            child.attacker,
        ) {
            if linked {
                hit = 1;
            }
            break;
        }
        next = child.next;
    }

    actor.row.set_life(life);
    actor.row.record[98..102].copy_from_slice(&damage.to_le_bytes());
    actor.put(102, pending);
    actor.put(42, attacker);
    if life < 0 {
        actor.put(40, attacker);
        2
    } else {
        hit
    }
}

fn jitter(actor: &mut Actor<'_>) {
    let first = level2_next_rng16(actor.row.entity_rng);
    let next = level2_next_rng16(first);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    actor.row.set_entity_rng(next);
    let yaw = actor
        .row
        .yaw
        .wrapping_add(((i32::from(next & 255) + 85) * direction) as i16)
        & 2047;
    actor.row.set_yaw(yaw);
    actor.row.set_source_roll(yaw);
}

fn move_actor(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    if actor.row.control_flags & 0x800 != 0 {
        actor
            .row
            .set_control_flags(actor.row.control_flags & !0x800);
        return;
    }
    let (rules, behavior_flags) = mc2::motion_override(actor.row).unwrap_or((
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: actor.row.array_pitch().max(actor.row.array_roll()),
            half_height: actor.row.array_fov(),
            ground_clearance: RULES.ground_clearance,
            descent: RULES.descent,
            turn_limit: RULES.turn_limit,
            terrain_mask: RULES.terrain_mask,
            maximum_tile_slope: RULES.maximum_slope,
        },
        RULES.behavior_flags,
    ));
    let motion = crate::ground_creature_motion::step_ground_creature_motion(
        actor.row.position,
        actor.row.yaw,
        actor.row.source_roll(),
        actor.row.actual_speed,
        ctx.terrain,
        rules,
    );
    actor.row.set_position(motion.position);
    actor.row.set_yaw(motion.yaw);
    let flags = if motion.movement_blocked {
        actor.row.control_flags | 0x40000
    } else {
        actor.row.control_flags & !0x40000
    };
    actor.row.set_control_flags(flags);
    if motion.killed {
        if ctx.terrain.material_mask(actor.row.position) == crate::WATER_AMBIENCE_MASK
            || behavior_flags & 1 != 0
        {
            actor.row.set_life(-1);
        }
        if actor.row.life == -1 {
            effects.population_decrements = effects.population_decrements.wrapping_add(1);
        }
    }
}

fn nearest(actor: &Actor<'_>, ctx: &Context<'_>) -> Option<Target> {
    let mut best = None;
    let mut shortest = u32::MAX;
    for target in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|target| target.control_flags & 0x20 == 0)
    {
        let d = xy(actor.row.position, target.position);
        let heading = heading_for_positions(actor.row.position, target.position);
        if d <= RULES.target_range.wrapping_mul(RULES.target_range)
            && d < shortest
            && angle_distance(i32::from(actor.row.yaw), heading) < RULES.target_half_angle
        {
            best = Some(target);
            shortest = d;
        }
    }
    best
}

fn visuals(actor: &Actor<'_>) -> u16 {
    match actor.jump_state() {
        0 => SOURCE_VISUALS[0],
        1..=3 | 9 => SOURCE_VISUALS[1],
        4 => SOURCE_VISUALS[2],
        5 => SOURCE_VISUALS[3],
        6 => SOURCE_VISUALS[4],
        7 => SOURCE_VISUALS[5],
        8 => SOURCE_VISUALS[6],
        10 => SOURCE_VISUALS[7],
        _ => SOURCE_VISUALS[0],
    }
}

/// sub_265A0: jump impulse, gravity, cave ceiling, water landing and animation
/// selection. Its one returned splash is consumed by the existing effect tail.
fn jump(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let mut settle = false;
    let mut hold_movement = false;
    match actor.jump_state() {
        0 | 1 => {
            let delay = actor.row.record[72];
            if delay != 0 {
                actor.row.record[72] = delay - 1;
            } else {
                actor.set_jump_state(2);
            }
            settle = true;
            hold_movement = true;
        }
        2 => {
            actor.set_jump_state(3);
            settle = true;
            hold_movement = true;
        }
        3 => actor.set_jump_state(4),
        4 => {
            actor
                .row
                .set_entity_rng(level2_next_rng16(actor.row.entity_rng));
            let impulse = RULES
                .jump_impulse_base
                .wrapping_add((actor.row.entity_rng % RULES.jump_impulse_span.max(1)) as i16);
            actor.row.record[48..50].copy_from_slice(&impulse.to_le_bytes());
            actor.set_jump_state(5);
        }
        5 => {
            let velocity = actor.word(48) as i16;
            if velocity < 0 {
                actor.set_jump_state(6);
            }
        }
        6 => {
            let ground = ctx.terrain.ground_height(actor.row.position);
            if actor.row.position.z.wrapping_sub(ground) < RULES.landing_clearance {
                actor.set_jump_state(7);
            }
        }
        7 => {
            actor.set_jump_state(8);
            hold_movement = true;
        }
        8 => {
            actor.set_jump_state(9);
            settle = true;
            hold_movement = true;
        }
        9 => {
            actor
                .row
                .set_entity_rng(level2_next_rng16(actor.row.entity_rng));
            if actor.row.entity_rng % 11 == 0 {
                effects.sound = Some(JUMP_SOUND);
                effects.sound_position = Some(actor.row.position);
                effects.sound_identity = Some((
                    actor.row.source_id,
                    actor.row.action,
                    actor.row.control_flags,
                ));
            }
            let count = actor.row.record[71];
            if count != 0 {
                actor
                    .row
                    .set_entity_rng(level2_next_rng16(actor.row.entity_rng));
                let delay = actor.row.entity_rng % u16::from(count);
                actor.row.record[72] = delay as u8;
                actor.set_jump_state(u8::from(delay % 2 == 0));
            } else {
                actor.row.record[72] = 1;
                actor.set_jump_state(0);
            }
            settle = true;
            hold_movement = true;
        }
        10 => {
            settle = true;
        }
        _ => {}
    }

    let vertical_delta = if settle {
        -RULES.gravity_step
    } else {
        let velocity = actor.word(48) as i16;
        actor.put(48, velocity.wrapping_sub(RULES.gravity_step) as u16);
        velocity
    };
    actor.row.position.z = actor.row.position.z.wrapping_add(vertical_delta);
    let ground = ctx.terrain.ground_height(actor.row.position);
    if actor.row.position.z < ground {
        actor.row.position.z = ground;
    }
    if ctx.terrain.map_type() == crate::native_world::MapType::Cave
        && let Some(ceiling) = ctx.terrain.ceiling_height(actor.row.position)
        && actor.row.position.z > ceiling.wrapping_sub(actor.row.array_fov())
    {
        actor.row.record[48..50].copy_from_slice(&0_i16.to_le_bytes());
        actor.row.position.z = ceiling.wrapping_sub(actor.row.array_fov());
    }
    let water = ctx.terrain.material_mask(actor.row.position) == crate::WATER_AMBIENCE_MASK;
    if water {
        if actor.jump_state() == 10 {
            if actor.row.position.z > ground {
                actor.set_jump_state(0);
            }
        } else if actor.row.position.z == ground {
            actor.set_jump_state(10);
            // sub_265A0 uses the shared predicted-axis scratch, which may
            // retain a previous writer when sub_1B8C0 was held by bit 0x800.
            actor.row.set_position(actor.row.position);
            effects.water_splash = Some(ctx.terrain.source_predicted_position());
            effects.water_splash_tail = Some((hold_movement, WaterContinuation::None));
            return;
        }
        actor.row.set_actual_speed(if actor.row.action == 170 {
            RULES.attack_water_speed
        } else {
            RULES.roam_water_speed
        });
    } else {
        if actor.jump_state() == 10 {
            actor.set_jump_state(0);
        }
        actor.row.set_actual_speed(if actor.row.action == 170 {
            RULES.attack_ground_speed
        } else {
            RULES.roam_ground_speed
        });
    }
    finish_jump_tail(actor, hold_movement);
}

/// Resume the water branch after its source NewEvent, including allocation
/// failure. The caller supplies the current occupant of the original address.
pub(super) fn finish_water_splash(actor: &mut Actor<'_>, ctx: &Context<'_>,
    hold_movement: bool, continuation: WaterContinuation) {
    actor.row.set_actual_speed(if actor.row.action == 170 {
        RULES.attack_water_speed
    } else {
        RULES.roam_water_speed
    });
    finish_jump_tail(actor, hold_movement);
    match continuation {
        WaterContinuation::None => {}
        WaterContinuation::Roam => finish_roam(actor, ctx),
        WaterContinuation::AttackReset => actor.set_action(169),
        WaterContinuation::Stage => if actor.row.action != 175 { actor.set_action(actor.row.action); },
    }
}

fn finish_jump_tail(actor: &mut Actor<'_>, hold_movement: bool) {
    if hold_movement {
        actor.row.set_control_flags(actor.row.control_flags | 0x800);
    }
    actor.row.set_position(actor.row.position);
    let visual = visuals(actor);
    if actor.row.particle_parameter_index != Some(visual) {
        actor.row.particle_parameter_index = Some(visual);
        put(actor.row, 94, visual);
        actor.row.record[96] = 0;
    }
}

fn apply_hit(actor: &mut Actor<'_>, hit: u8) {
    match hit {
        1 => {
            actor.set_target(actor.attacker());
            actor.set_action(170);
        }
        2 => actor.set_action(172),
        _ => {}
    }
}

fn roam(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let hit = consume(actor, ctx, true);
    if hit != 0 {
        apply_hit(actor, hit);
        if hit == 2 {
            return;
        }
    }
    move_actor(actor, ctx, effects);
    jump(actor, ctx, effects);
    if let Some((_, continuation)) = &mut effects.water_splash_tail {
        *continuation = WaterContinuation::Roam;
        return;
    }
    finish_roam(actor, ctx);
}

fn finish_roam(actor: &mut Actor<'_>, ctx: &Context<'_>) {
    if can_steer_jump(actor) {
        jitter(actor);
    }
    if actor.row.phase & RULES.target_scan_mask == 0
        && actor.row.record[61] != 0
        && let Some(target) = nearest(actor, ctx)
    {
        actor.set_target(target.slot);
        actor.set_action(170);
    }
}

fn attack(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    let hit = consume(actor, ctx, true);
    if hit != 0 {
        apply_hit(actor, hit);
        if hit == 2 {
            return;
        }
    }
    let target_slot = actor.target();
    let target = if target_slot != 0 {
        stage::alliance_target(actor, ctx, effects, ctx.target(target_slot))
    } else {
        None
    }
    .filter(|target| living(*target));
    let mut reset = target.is_none();
    if let Some(target) = target {
        if can_steer_jump(actor) {
            actor
                .row
                .set_source_roll(heading_for_positions(actor.row.position, target.position) as i16);
            if actor.row.phase & 3 == 0 {
                for peer in ctx.model21.iter().filter_map(|slot| ctx.target(*slot)) {
                    if peer.source_id != actor.row.source_id
                        && i32::from(actor.row.position.x)
                            .wrapping_sub(i32::from(peer.position.x))
                            .abs()
                            < i32::from(actor.row.array_pitch())
                        && i32::from(actor.row.position.y)
                            .wrapping_sub(i32::from(peer.position.y))
                            .abs()
                            < i32::from(actor.row.array_pitch())
                    {
                        actor.row.set_source_roll(heading_for_positions(
                            peer.position,
                            actor.row.position,
                        ) as i16);
                        break;
                    }
                }
            }
        }
        if actor.row.phase & RULES.attack_cadence_mask == 0 {
            if distance(actor.row.position, target.position) < RULES.target_range {
                // sub_1CC20 allocates and reads current addresses before the
                // remaining movement and jump tail. The adapter resumes that
                // tail after the real allocation attempt, including failure.
                effects.strike_target = Some(target.slot);
                return;
            } else {
                reset = true;
            }
        }
    }
    finish_attack(actor, ctx, effects);
    if reset {
        if let Some((_, continuation)) = &mut effects.water_splash_tail {
            *continuation = WaterContinuation::AttackReset;
            return;
        }
        actor.set_action(169);
    }
}

/// Remaining source action170 tail after the actual strike attempt.
pub(super) fn finish_attack(actor: &mut Actor<'_>, ctx: &Context<'_>, effects: &mut Effects) {
    move_actor(actor, ctx, effects);
    jump(actor, ctx, effects);
}

fn can_steer_jump(actor: &Actor<'_>) -> bool {
    actor.jump_state() == 9 || (actor.jump_state() == 10 && actor.row.phase & 7 == 0)
}

/// Apply one source callback. The ordinary schedule still owns byte3E's
/// same-pass tail and next-pass reclaim.
pub(crate) fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<Effects, String> {
    let mut effects = Effects::default();
    match actor.row.action {
        168 => actor.set_action(169),
        169 => roam(actor, ctx, &mut effects),
        170 => attack(actor, ctx, &mut effects),
        171 => actor.set_action(169),
        172 => {
            actor.set_action(173);
            effects.prekill = true;
        }
        173 => effects.kill = actor.row.phase & 7 == 0,
        175 => stage::step(actor, ctx, &mut effects),
        action => return Err(format!("model21 action{action} has no source callback")),
    }
    Ok(effects)
}
