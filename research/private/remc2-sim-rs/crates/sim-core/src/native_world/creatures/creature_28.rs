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
    pub route_duration: i16,
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
}

impl Target {
    fn living(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }
}

pub(crate) struct Frame<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    pub same_model: &'a [u16],
}

impl Frame<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| {
                self.targets
                    .iter()
                    .find(|target| target.slot == slot)
                    .copied()
            })
            .flatten()
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
    pub animation: Option<u16>,
    pub kill_credit: Option<u16>,
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

fn tile_compare(terrain: &SessionTerrain, position: Position) -> u8 {
    let tile_x = (position.x as u16 >> 8) as u8;
    let tile_y = (position.y as u16 >> 8) as u8;
    let heights = terrain.ground_heights();
    let index = |x: u8, y: u8| usize::from(y) * 256 + usize::from(x);
    let northwest = i16::from(heights[index(tile_x, tile_y)]);
    let northeast = i16::from(heights[index(tile_x.wrapping_add(1), tile_y)]);
    let southeast = i16::from(heights[index(tile_x.wrapping_add(1), tile_y.wrapping_add(1))]);
    let southwest = i16::from(heights[index(tile_x, tile_y.wrapping_add(1))]);
    let diagonal_a = (northwest + southwest - northeast - southeast).abs();
    let diagonal_b = (northwest + northeast - southwest - southeast).abs();
    diagonal_a.max(diagonal_b).min(i16::from(u8::MAX)) as u8
}

/// Source sub_2B9A0: consume the ordinary mailbox and inherit the weakest
/// linked child life before action226 decides whether to continue or die.
fn consume(actor: &mut Actor, frame: &Frame<'_>) -> DamageResult {
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
    for _ in 0..=frame.targets.len() {
        let Some(linked) = frame.target(child) else {
            break;
        };
        if linked.life < actor.life {
            actor.life = linked.life;
            actor.attacker = linked.attacker;
            hit = true;
            break;
        }
        child = linked.next;
    }
    if actor.life < 0 {
        actor.killer = actor.attacker;
        actor.action = SOURCE_DEATH_ACTION;
        DamageResult::Killed
    } else if hit {
        actor.target = actor.attacker;
        DamageResult::Hit
    } else {
        DamageResult::None
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
            actor.source_animation = 0;
            actor.speed = actor.maximum_speed;
            // Visual292's descriptor-defined shift and sprite remain bound by
            // the row adapter; this transition only resets its frame.
        }
        2 => {
            actor.descriptor = SOURCE_DESCRIPTOR;
            actor.source_animation = 0;
            actor.speed = actor.minimum_speed;
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

fn collision_half_width(visual: u16) -> i16 {
    match visual {
        SOURCE_IDLE_VISUAL => SOURCE_PROPERTIES.idle_half_width,
        SOURCE_ATTACK_VISUAL => SOURCE_PROPERTIES.attack_half_width,
        _ => 0,
    }
}

fn collision_blocked(
    terrain: &SessionTerrain,
    predicted: Position,
    heading: u16,
    visual: u16,
) -> bool {
    let mut probe = predicted;
    let extent = collision_half_width(visual).max(0);
    let mut offset = 0;
    loop {
        if terrain.material_mask(probe) & !SOURCE_PROPERTIES.terrain_mask != 0
            || terrain.cave_collision_blocked(probe)
        {
            return true;
        }
        if offset + 256 > extent {
            break;
        }
        move_entity(&mut probe, heading as i16, 0, 256);
        offset += 256;
    }
    false
}

/// Bounded movement projection from sub_1B8C0. The shared terrain owner
/// supplies the collision probe; the source fallback headings and rotation
/// cadence remain owned by this model.
fn move_actor(actor: &mut Actor, terrain: &SessionTerrain) -> MoveResult {
    if actor.flags & 0x800 != 0 {
        actor.flags &= !0x800;
        return MoveResult::Clear;
    }
    let original = actor.position;
    let yaw = actor.yaw as u16;
    let candidates = [
        yaw & 2047,
        yaw.wrapping_add(341) & 2047,
        (yaw.wrapping_sub(85) & 255) | (yaw.wrapping_sub(341) & 0x700),
        (yaw.wrapping_add(1024) & 0x700) | (yaw & 255),
    ];
    for (attempt, heading) in candidates.into_iter().enumerate() {
        let mut predicted = original;
        let ground = terrain.ground_height(original);
        if predicted.z > ground {
            predicted.z = predicted.z.wrapping_add(SOURCE_PROPERTIES.descent);
        }
        let floor = ground.wrapping_add(SOURCE_PROPERTIES.ground_clearance);
        if predicted.z <= floor {
            predicted.z = floor;
        }
        move_entity(&mut predicted, heading as i16, 0, actor.speed);
        let crossed_tile =
            (predicted.x >> 8, predicted.y >> 8) != (original.x >> 8, original.y >> 8);
        let blocked = crossed_tile
            && (collision_blocked(terrain, predicted, heading, actor.visual)
                || i32::from(tile_compare(terrain, predicted)) >= SOURCE_PROPERTIES.maximum_slope);
        if !blocked {
            actor.position = predicted;
            let heading = heading as i16;
            actor.yaw = heading.wrapping_add(crate::turn_toward(
                heading,
                actor.roll,
                SOURCE_PROPERTIES.turn_response,
            )) & 2047;
            if attempt == 0 {
                actor.flags &= !0x0004_0000;
            }
            return if attempt == 0 {
                MoveResult::Clear
            } else {
                MoveResult::Avoided
            };
        }
        actor.flags |= 0x40000;
    }
    actor.position = original;
    if SOURCE_PROPERTIES.behavior_flags & 1 != 0 || terrain.material_mask(original) == 1 {
        actor.life = -1;
    }
    MoveResult::Stuck
}

fn target_attack(actor: &mut Actor, frame: &Frame<'_>) -> Option<Target> {
    let target = frame
        .target(actor.target)
        .filter(|target| target.living())?;
    actor.target = target.slot;
    Some(target)
}

fn acquire_target(actor: Actor, frame: &Frame<'_>) -> Option<Target> {
    let range_squared = SOURCE_PROPERTIES.target_radius * SOURCE_PROPERTIES.target_radius;
    frame
        .targets
        .iter()
        .copied()
        .filter(|target| {
            target.slot != actor.slot
                && target.class == 3
                && target.living()
                && target.flags & 0x20 == 0
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

fn step_attack_animation(actor: &mut Actor, frame: &Frame<'_>, effects: &mut Effects) {
    if actor.timer <= 0 {
        actor.phase = 6;
        return;
    }

    actor.yaw = actor.source_animation as i16;
    actor.roll = actor.yaw;
    if actor.phase == 4
        && actor.source_counter & 7 == 0
        && let Some(target) = target_attack(actor, frame)
        && distance_xy(actor.position, target.position) > 802_816
    {
        actor.roll = crate::heading_for_positions(actor.position, target.position) as i16;
    }
    if actor.phase == 4
        && actor.timer < i32::from(actor.animation_limit).saturating_sub(3)
        && actor.timer > 3
        && let Some(target) = target_attack(actor, frame)
        && distance(actor.position, target.position) < SOURCE_PROPERTIES.melee_radius
    {
        actor.phase = 5;
        effects.melee = Some(MeleeRequest {
            target_slot: target.slot,
            attacker_source_id: actor.source_id,
            damage: actor.subspell_index,
        });
    }
    actor.timer = actor.timer.wrapping_sub(1);
    let _ = move_actor(actor, frame.terrain);
    actor.source_animation = actor.yaw as u16;
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
        consume(actor, frame)
    } else {
        DamageResult::None
    };
    if damage == DamageResult::Killed {
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
            actor.position.z = frame.terrain.ground_height(actor.position);
            if damage == DamageResult::Hit {
                actor.action = SOURCE_ATTACK_ACTION;
                actor.phase = 0;
            }
        }
        SOURCE_IDLE_ACTION => {
            if damage == DamageResult::Hit {
                actor.action = SOURCE_ATTACK_ACTION;
                actor.phase = 0;
            } else {
                let _ = move_actor(actor, frame.terrain);
                if actor.target_scan_limit != 0
                    && actor.source_counter % SOURCE_PROPERTIES.scan_period == 0
                {
                    update_roam_roll(actor);
                    if let Some(target) = acquire_target(*actor, frame) {
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
                actor.phase = 1;
            }
            1 => {
                effects.water_splash = true;
                actor.phase = 2;
                actor.timer = 32;
            }
            2 => {
                let target = target_attack(actor, frame);
                if let Some(target) = target {
                    actor.timer = actor.timer.wrapping_sub(1);
                    if actor.timer <= 0 {
                        if !frame.another_attacking(*actor) {
                            actor.phase = 3;
                        }
                    } else {
                        actor.roll =
                            crate::heading_for_positions(actor.position, target.position) as i16;
                        let movement = move_actor(actor, frame.terrain);
                        if movement == MoveResult::Avoided {
                            actor.phase = 7;
                        } else {
                            let mut approach_position = target.position;
                            move_entity(&mut approach_position, target.yaw, 0, 768);
                            if !frame.another_attacking(*actor)
                                && distance_xy(actor.position, approach_position) < 2_768_896
                            {
                                actor.phase = 3;
                            }
                        }
                    }
                } else {
                    if !frame.another_attacking(*actor) {
                        actor.phase = 3;
                    }
                }
            }
            3 => {
                actor.phase = 4;
                set_visual_state(actor, 2)?;
                actor.source_animation = actor.yaw as u16;
                set_animation(actor, &mut effects.notices, particles, SOURCE_ATTACK_VISUAL)?;
                actor.timer = i32::from(actor.animation_limit);
                effects.notices.sound = Some(38);
                step_attack_animation(actor, frame, &mut effects.notices);
            }
            4 | 5 => step_attack_animation(actor, frame, &mut effects.notices),
            6 => {
                set_visual_state(actor, 3)?;
                effects.water_splash = true;
                if target_attack(actor, frame).is_some_and(|target| {
                    distance(actor.position, target.position) < SOURCE_PROPERTIES.target_radius
                }) {
                    actor.phase = 2;
                    actor.timer = 32;
                } else {
                    actor.phase = 7;
                }
            }
            7 => {
                actor.roll = (next_rng(actor) & 0x7ff) as i16;
                actor.phase = 8;
                actor.timer = 16;
                let _ = move_actor(actor, frame.terrain);
                actor.timer = actor.timer.wrapping_sub(1);
            }
            8 => {
                let _ = move_actor(actor, frame.terrain);
                actor.timer = actor.timer.wrapping_sub(1);
                if actor.timer <= 0 {
                    actor.phase = 9;
                }
            }
            9 => {
                set_visual_state(actor, 1)?;
                set_animation(actor, &mut effects.notices, particles, SOURCE_IDLE_VISUAL)?;
                actor.action = SOURCE_IDLE_ACTION;
                actor.target = 0;
            }
            phase => return Err(format!("unsupported model28 phase{phase}")),
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
