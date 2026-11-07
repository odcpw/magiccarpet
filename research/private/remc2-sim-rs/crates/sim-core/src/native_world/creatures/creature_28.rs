//! Source class5/model28 creature. The source row remains the only mutable
//! carrier; this module owns its descriptor-backed attack and damage phases.

use crate::level_definition::{CREATURE28_CLASS_PROPERTIES, Creature28ClassProperties};
use crate::native_world::{ENTITY_DRAW_DISABLED_BIT, SessionTerrain};
use crate::{FlightWorld, Position, move_entity};

pub(crate) mod mc2;
pub(crate) mod stage;

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 28;
pub(crate) const SOURCE_IDLE_ACTION: u8 = 225;
pub(crate) const SOURCE_ATTACK_ACTION: u8 = 226;
pub(crate) const SOURCE_DEATH_ACTION: u8 = 228;
pub(crate) const SOURCE_DESCRIPTOR: u16 = 93;
pub(crate) const SOURCE_RECOVERY_DESCRIPTOR: u16 = 94;
pub(crate) const SOURCE_IDLE_VISUAL: u16 = 292;
pub(crate) const SOURCE_ATTACK_VISUAL: u16 = 291;

/// Movement and attack defaults are canonical typed content; these source
/// descriptor and visual identities remain adapter-local.
pub(crate) const SOURCE_PROPERTIES: Creature28ClassProperties = CREATURE28_CLASS_PROPERTIES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Actor {
    pub slot: u16,
    pub source_id: u16,
    pub action: u8,
    pub phase: u8,
    pub flags: u32,
    pub position: Position,
    pub rng: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub half_width: i16,
    pub roll: i16,
    pub speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub life: i32,
    pub max_life: i32,
    pub timer: i32,
    pub source_animation: u16,
    pub descriptor: u16,
    pub subspell_index: u16,
    pub target: u16,
    pub child_head: u16,
    pub parent: u16,
    pub route_index: u8,
    pub route_mode: u8,
    pub route_relation: u16,
    pub route_remaining: i16,
    pub route_duration: u16,
    pub attacker: u16,
    pub killer: u16,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub target_scan_limit: u8,
    pub source_counter: u8,
    pub visual: u16,
    pub animation_frame: u8,
    pub animation_limit: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Target {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub phase: u8,
    pub flags: u32,
    pub life: i32,
    pub position: Position,
    pub yaw: i16,
    pub half_width: i16,
    pub next: u16,
    pub attacker: u16,
    pub killer: u16,
    pub route_index: u8,
    pub route_relation: u16,
    pub route_target: u16,
    pub parent: u16,
    pub route_mode: u8,
    pub relations: crate::native_world::source_target::Relations,
    pub source_yaw: Option<i16>,
    pub source_roll: Option<i16>,
    pub pitch_extent: Option<i16>,
}

impl Target {
    fn living(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }
}

pub(crate) struct Frame<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    pub current_target: Option<&'a dyn Fn(u16) -> Option<Target>>,
    pub sentinel: Option<Target>,
    pub class3: &'a [u16],
    pub families: Option<&'a [Vec<u16>; 29]>,
    pub attack_animation_count: Option<&'a dyn Fn() -> Result<Option<u16>, String>>,
    pub same_model: &'a [u16],
}

impl Frame<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        if slot == 0 { return None; }
        if let Some(current_target) = self.current_target {
            return current_target(slot);
        }
        self.targets.iter().find(|target| target.slot == slot).copied()
    }

    fn actor_target(&self, actor: &Actor, slot: u16) -> Option<Target> {
        if slot != actor.slot { return self.target(slot); }
        Some(Target {
            slot, source_id: actor.source_id, class: SOURCE_CLASS, model: SOURCE_MODEL,
            action: actor.action, phase: actor.phase, flags: actor.flags, life: actor.life,
            position: actor.position, yaw: actor.yaw, half_width: actor.half_width,
            next: actor.child_head, attacker: actor.attacker, killer: actor.killer,
            route_index: actor.route_index, route_relation: actor.route_relation,
            route_target: actor.target, parent: actor.parent, route_mode: actor.route_mode,
            relations: crate::native_world::source_target::Relations {
                target: Some(actor.target), attacker: Some(actor.attacker), parent: Some(actor.parent),
                stage_mode: Some(actor.route_mode), stage_index: Some(actor.route_index),
                stage_relation: Some(actor.route_relation), next: Some(actor.child_head),
            },
            source_yaw: Some(actor.yaw), source_roll: Some(actor.roll), pitch_extent: Some(actor.half_width),
        })
    }

    fn another_attacking(&self, actor: Actor) -> bool {
        self.same_model.iter().any(|slot| {
            *slot != actor.slot
                && self.target(*slot).is_some_and(|other| {
                    other.action == SOURCE_ATTACK_ACTION && matches!(other.phase, 3..=5)
                })
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MeleeRequest {
    pub target_slot: u16,
    pub attacker_source_id: u16,
    pub damage: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    pub melee: Option<MeleeRequest>,
    pub sound: Option<u8>,
    pub sound_origin: Option<SoundOrigin>,
    pub animation: Option<u16>,
    pub kill_credit: Option<u16>,
    pub feedback: Vec<(u16, u8)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SoundOrigin {
    pub position: Position,
    pub source_id: u16,
    pub control_flags: u32,
    pub action: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CallbackEffects {
    pub notices: Effects,
    pub water_splash: bool,
    pub prekill: bool,
    pub kill: bool,
    pub stage_hide: bool,
    pub stage_mana_expiry: bool,
    pub stage_mana_mode13: bool,
    pub child_fade_flags: Vec<(u16, u8)>,
    pub population_decrement: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DamageResult {
    None,
    Hit,
    Killed,
}

fn distance_xy(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn next_rng(actor: &mut Actor) -> u16 {
    actor.rng = actor.rng.wrapping_mul(9_377).wrapping_add(9_439);
    actor.rng
}

/// Source sub_2B9A0: consume the ordinary mailbox and inherit the weakest
/// linked child life before action226 decides whether to continue or die.
fn consume(actor: &mut Actor, frame: &Frame<'_>) -> Result<DamageResult, String> {
    let mut hit = false;
    if actor.pending_attacker != 0 {
        actor.life = actor.life.wrapping_sub(actor.pending_damage);
        actor.attacker = actor.pending_attacker;
        actor.pending_attacker = 0;
        hit = true;
    } else {
        actor.attacker = 0;
    }
    let mut child = actor.child_head;
    for _ in 0..1000 {
        let Some(linked) = frame.actor_target(actor, child) else {
            break;
        };
        if linked.life < actor.life {
            actor.life = linked.life;
            actor.attacker = linked.relations.attacker.ok_or_else(||
                format!("model28 linked slot{child} has no current attacker"))?;
            hit = true;
            break;
        }
        child = linked.relations.next.ok_or_else(||
            format!("model28 linked slot{child} has no current next relation"))?;
    }
    if actor.life < 0 {
        actor.killer = actor.attacker;
        actor.action = SOURCE_DEATH_ACTION;
        Ok(DamageResult::Killed)
    } else if hit {
        actor.target = actor.attacker;
        Ok(DamageResult::Hit)
    } else {
        Ok(DamageResult::None)
    }
}

fn set_animation(
    actor: &mut Actor,
    effects: &mut Effects,
    particles: &[crate::native_world::ParticleParameter],
    visual: u16,
) -> Result<(), String> {
    let parameter = particles
        .get(usize::from(visual))
        .ok_or_else(|| format!("model28 requires retained particle{visual}"))?;
    actor.animation_limit = parameter
        .animation_limit()
        .ok_or_else(|| format!("model28 particle{visual} has invalid animation mode"))?;
    actor.animation_frame = 0;
    actor.visual = visual;
    effects.animation = Some(visual);
    Ok(())
}

fn set_visual_state(actor: &mut Actor, state: u8) -> Result<(), String> {
    match state {
        1 => {
            actor.flags = (actor.flags & !0x09) | 0x08;
            actor.descriptor = SOURCE_DESCRIPTOR;
            actor.speed = actor.maximum_speed;
            actor.half_width = SOURCE_PROPERTIES.idle_half_width;
            // Visual292's descriptor-defined shift and sprite remain bound by
            // the row adapter; this transition only resets its frame.
        }
        2 => {
            actor.descriptor = SOURCE_DESCRIPTOR;
            actor.source_animation = 0;
            actor.speed = actor.minimum_speed;
            actor.half_width = SOURCE_PROPERTIES.attack_half_width;
            actor.flags = (actor.flags & !0x09) | 0x08;
        }
        3 => {
            actor.descriptor = SOURCE_RECOVERY_DESCRIPTOR;
            actor.target_scan_limit = 0;
            actor.speed = actor.minimum_speed.wrapping_sub(28);
            actor.flags = (actor.flags & !0x08) | 0x01;
        }
        _ => return Err(format!("unsupported model28 visual state{state}")),
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MoveResult {
    Clear,
    Avoided,
    Stuck,
}

/// Source sub_1B8C0 uses the shared collision/retry and prediction owner.
fn move_actor(actor: &mut Actor, terrain: &SessionTerrain) -> MoveResult {
    if actor.flags & 0x800 != 0 {
        actor.flags &= !0x800;
        return MoveResult::Clear;
    }
    let motion = crate::ground_creature_motion::step_ground_creature_motion(
        actor.position, actor.yaw, actor.roll, actor.speed, terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: actor.half_width,
            half_height: if actor.visual == SOURCE_ATTACK_VISUAL {
                SOURCE_PROPERTIES.attack_half_height
            } else {
                SOURCE_PROPERTIES.idle_half_height
            },
            ground_clearance: SOURCE_PROPERTIES.ground_clearance,
            descent: SOURCE_PROPERTIES.descent,
            turn_limit: SOURCE_PROPERTIES.turn_response,
            terrain_mask: SOURCE_PROPERTIES.terrain_mask,
            maximum_tile_slope: SOURCE_PROPERTIES.maximum_slope,
        },
    );
    actor.position = motion.position;
    actor.yaw = motion.yaw;
    if motion.movement_blocked { actor.flags |= 0x40000; } else { actor.flags &= !0x40000; }
    if motion.killed && (SOURCE_PROPERTIES.behavior_flags & 1 != 0 || terrain.material_mask(actor.position) == 1) {
        actor.life = -1;
    }
    match motion.result {
        crate::ground_creature_motion::GroundCreatureMoveResult::Alternate => MoveResult::Avoided,
        crate::ground_creature_motion::GroundCreatureMoveResult::Blocked => MoveResult::Stuck,
        _ => MoveResult::Clear,
    }
}

fn move_for_callback(actor: &mut Actor, terrain: &SessionTerrain, effects: &mut CallbackEffects) -> MoveResult {
    let before = actor.life;
    let result = move_actor(actor, terrain);
    if result == MoveResult::Stuck && before >= 0 && actor.life < 0 {
        effects.population_decrement = true;
    }
    result
}

fn target_attack(actor: &mut Actor, frame: &Frame<'_>) -> Option<Target> {
    let target = frame
        .actor_target(actor, actor.target)
        .filter(|target| target.living())?;
    actor.target = target.slot;
    Some(target)
}

fn acquire_target(actor: Actor, frame: &Frame<'_>) -> Option<Target> {
    let range_squared = SOURCE_PROPERTIES.target_radius * SOURCE_PROPERTIES.target_radius;
    frame.class3.iter().filter_map(|slot| frame.actor_target(&actor, *slot))
        .filter(|target| {
            target.flags & 0x20 == 0
                && distance_xy(actor.position, target.position) <= range_squared
                && (crate::angle_distance(
                    i32::from(actor.yaw),
                    i32::from(crate::heading_for_positions(
                        actor.position,
                        target.position,
                    )),
                ) as u16)
                    < SOURCE_PROPERTIES.target_half_angle as u16
        })
        .min_by_key(|target| distance_xy(actor.position, target.position))
}

fn update_roam_roll(actor: &mut Actor) {
    let direction_roll = next_rng(actor);
    let magnitude_roll = next_rng(actor);
    let direction = if direction_roll % 0x9d < 79 { -1 } else { 1 };
    let magnitude = i16::from((magnitude_roll & 0xff) as u8) + 85;
    actor.roll = actor.roll.wrapping_add(direction * magnitude) & 2047;
}

/// sub_2BA50 assigns both the phase and its source countdown.
fn set_attack_phase(actor: &mut Actor, phase: u8) {
    actor.phase = phase;
    actor.timer = match phase { 2 => 32, 8 => 16, _ => 0 };
}

/// Source's frozen model list supplies membership and order; each peer's
/// current fields determine whether it changes the desired heading.
fn avoid_nearby_peer(actor: &mut Actor, frame: &Frame<'_>) {
    for slot in frame.same_model {
        let Some(peer) = frame.target(*slot) else { continue };
        if peer.source_id != actor.source_id
            && (i32::from(actor.position.x) - i32::from(peer.position.x)).abs() < i32::from(actor.half_width)
            && (i32::from(actor.position.y) - i32::from(peer.position.y)).abs() < i32::from(actor.half_width)
        {
            actor.roll = crate::heading_for_positions(peer.position, actor.position) as i16;
            break;
        }
    }
}

fn step_attack_animation(actor: &mut Actor, frame: &Frame<'_>, effects: &mut CallbackEffects) {
    if actor.timer <= 0 {
        set_attack_phase(actor, 6);
        return;
    }

    actor.yaw = actor.route_duration as i16;
    actor.roll = actor.yaw;
    if actor.phase == 4
        && actor.source_counter & 7 == 0
        && let Some(target) = target_attack(actor, frame)
        && distance_xy(actor.position, target.position) > 802_816
    {
        actor.roll = crate::heading_for_positions(actor.position, target.position) as i16;
    }
    if actor.phase == 4
        && actor.timer < i32::from(actor.source_animation) - 3
        && actor.timer > 3
        && let Some(target) = target_attack(actor, frame)
        && distance(actor.position, target.position) < SOURCE_PROPERTIES.melee_radius
    {
        actor.phase = 5;
        effects.notices.melee = Some(MeleeRequest {
            target_slot: target.slot,
            attacker_source_id: actor.source_id,
            damage: actor.subspell_index,
        });
    }
    actor.timer = actor.timer.wrapping_sub(1);
    if actor.source_counter & 3 == 0 {
        avoid_nearby_peer(actor, frame);
    }
    let _ = move_for_callback(actor, frame.terrain, effects);
    actor.route_duration = actor.yaw as u16;
    let turn = if actor.timer & 4 != 0 { 56 } else { -56 };
    actor.yaw = actor.yaw.wrapping_add(turn) & 2047;
}

/// Actions224..229 cover the source callback chain: stationary and roaming
/// generic updates, model phase machine, reset, prekill, and kill consumption.
pub(crate) fn step(
    actor: &mut Actor,
    frame: &Frame<'_>,
    runtime: &crate::native_world::RuntimeSnapshot,
    particles: &[crate::native_world::ParticleParameter],
) -> Result<CallbackEffects, String> {
    let mut effects = CallbackEffects::default();
    let action_at_start = actor.action;
    let damage = if action_at_start == 231 {
        // sub_2B7B0 dispatches action231 straight into sub_1D5D0. Its selected
        // StageVar helper owns damage consumption, movement, and transitions.
        DamageResult::None
    } else if matches!(action_at_start, 224..=SOURCE_ATTACK_ACTION) {
        consume(actor, frame)?
    } else {
        DamageResult::None
    };
    if damage == DamageResult::Killed {
        if action_at_start == 224 {
            crate::ground_creature_motion::relax_to_terrain(&mut actor.position, frame.terrain,
                SOURCE_PROPERTIES.ground_clearance, SOURCE_PROPERTIES.descent);
        }
        return Ok(effects);
    }
    if action_at_start == SOURCE_DEATH_ACTION {
        actor.action = SOURCE_DEATH_ACTION + 1;
        effects.prekill = true;
        return Ok(effects);
    }
    if action_at_start == SOURCE_DEATH_ACTION + 1 {
        effects.kill = true;
        return Ok(effects);
    }

    match action_at_start {
        224 => {
            crate::ground_creature_motion::relax_to_terrain(&mut actor.position, frame.terrain,
                SOURCE_PROPERTIES.ground_clearance, SOURCE_PROPERTIES.descent);
            if damage == DamageResult::Hit {
                actor.action = SOURCE_ATTACK_ACTION;
                actor.phase = 0;
            }
        }
        SOURCE_IDLE_ACTION => {
            if damage == DamageResult::Hit {
                actor.action = SOURCE_ATTACK_ACTION;
                actor.phase = 0;
                crate::ground_creature_motion::relax_to_terrain(&mut actor.position, frame.terrain,
                    SOURCE_PROPERTIES.ground_clearance, SOURCE_PROPERTIES.descent);
            } else {
                let _ = move_for_callback(actor, frame.terrain, &mut effects);
                if actor.source_counter % SOURCE_PROPERTIES.scan_period == 0 {
                    update_roam_roll(actor);
                    if actor.target_scan_limit != 0
                        && let Some(target) = acquire_target(*actor, frame) {
                        actor.target = target.slot;
                        actor.action = SOURCE_ATTACK_ACTION;
                        actor.phase = 0;
                    }
                }
            }
        }
        227 => actor.action = SOURCE_IDLE_ACTION,
        SOURCE_ATTACK_ACTION => match actor.phase {
            0 => {
                set_visual_state(actor, 3)?;
                set_attack_phase(actor, 1);
            }
            1 => {
                effects.water_splash = true;
                set_attack_phase(actor, 2);
            }
            2 => {
                let target = target_attack(actor, frame);
                if let Some(target) = target {
                    actor.timer = actor.timer.wrapping_sub(1);
                    if actor.timer <= 0 {
                        if !frame.another_attacking(*actor) {
                            set_attack_phase(actor, 3);
                        }
                    } else {
                        let mut approach_position = target.position;
                        let yaw = target.source_yaw.ok_or_else(||
                            format!("model28 target{} has no current yaw", target.slot))?;
                        move_entity(&mut approach_position, yaw, 0, 768);
                        if actor.source_counter & 3 == 0 {
                            actor.roll = crate::heading_for_positions(actor.position, approach_position) as i16;
                            avoid_nearby_peer(actor, frame);
                        }
                        let movement = move_for_callback(actor, frame.terrain, &mut effects);
                        if movement == MoveResult::Avoided {
                            set_attack_phase(actor, 7);
                        } else if actor.source_counter & 3 == 0
                            && actor.timer < 14
                            && distance_xy(actor.position, approach_position) < 2_768_896
                            && !frame.another_attacking(*actor)
                        {
                            set_attack_phase(actor, 3);
                        }
                    }
                } else {
                    if !frame.another_attacking(*actor) {
                        set_attack_phase(actor, 3);
                    }
                }
            }
            3 => {
                set_attack_phase(actor, 4);
                set_visual_state(actor, 2)?;
                actor.route_duration = actor.yaw as u16;
                set_animation(actor, &mut effects.notices, particles, SOURCE_ATTACK_VISUAL)?;
                if let Some(count) = frame.attack_animation_count.map(|read| read()).transpose()?.flatten() {
                    actor.timer = i32::from(count);
                    actor.source_animation = count;
                }
                effects.notices.sound = Some(38);
                effects.notices.sound_origin = Some(SoundOrigin {
                    position: actor.position, source_id: actor.source_id,
                    control_flags: actor.flags, action: actor.action,
                });
                step_attack_animation(actor, frame, &mut effects);
            }
            4 | 5 => step_attack_animation(actor, frame, &mut effects),
            6 => {
                set_visual_state(actor, 3)?;
                effects.water_splash = true;
                if target_attack(actor, frame).is_some_and(|target| {
                    distance(actor.position, target.position) < SOURCE_PROPERTIES.target_radius
                }) {
                    set_attack_phase(actor, 2);
                } else {
                    set_attack_phase(actor, 7);
                }
            }
            7 => {
                actor.roll = (next_rng(actor) & 0x7ff) as i16;
                set_attack_phase(actor, 8);
                let _ = move_for_callback(actor, frame.terrain, &mut effects);
                actor.timer = actor.timer.wrapping_sub(1);
            }
            8 => {
                let _ = move_for_callback(actor, frame.terrain, &mut effects);
                actor.timer = actor.timer.wrapping_sub(1);
                if actor.timer <= 0 {
                    set_attack_phase(actor, 9);
                }
            }
            9 => {
                set_visual_state(actor, 1)?;
                set_animation(actor, &mut effects.notices, particles, SOURCE_IDLE_VISUAL)?;
                actor.action = SOURCE_IDLE_ACTION;
                actor.target = 0;
            }
            _ => {} // sub_2B260's default case returns without a state change.
        },
        231 => {
            stage::step(actor, frame, runtime, &mut effects)?;
            if actor.action == SOURCE_ATTACK_ACTION {
                // sub_2B840 is the same-pass tail of sub_2B7B0. The attack
                // callback itself is visited on the next source pass.
                actor.phase = 0;
            }
        }
        action => return Err(format!("unsupported model28 action{action}")),
    }
    Ok(effects)
}

/// Registered source callbacks for model28's schedule/dispatch contract.
pub(crate) const fn supports_action(action: u8) -> bool {
    matches!(action, 224..=229 | 231)
}
