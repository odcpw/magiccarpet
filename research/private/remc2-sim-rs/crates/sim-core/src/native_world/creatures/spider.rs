//! Spider behavior. The MC2 adapter borrows the authoritative arena row;
//! this owner receives only its actor, terrain, live views and frozen lists.

use crate::combat::actors::{consume_damage, inherit_killer, inherit_linked_life};
use crate::level_definition::SPIDER_CLASS_PROPERTIES as RULES;
use crate::native_world::{ENTITY_DRAW_DISABLED_BIT, MapType, SessionTerrain, level2_next_rng16};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;
#[cfg(test)]
mod tests;
pub(crate) mod web;

pub(crate) use super::archer::{
    ArcherStageDescriptor as StageDescriptor, ArcherStageRuntime as StageRuntime,
    ArcherStageState as StageState,
};
pub(crate) use stage::{bind_stage, stage_prepass};

pub(crate) const SOURCE_MODEL: u8 = 20;
pub(crate) const DESCRIPTOR: u16 = 89;

/// A callback-local typed borrow of the arena carrier, never a second live row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpiderState {
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
    pub damage: u16,
    pub attack_phase: u8,
    pub attack_timer: i32,
    pub pending_damage: i32,
    pub pending_attacker: u16,
    pub attacker: u16,
    pub killer: u16,
    pub target: u16,
    pub previous: u16,
    pub next: u16,
    pub phase: u8,
    pub proximity: u8,
    pub visual: u16,
    pub animation_frame: u8,
    pub animation_limit: u8,
    pub half_width: i16,
    pub half_height: i16,
    pub vertical_extent: i16,
    pub stage: StageState,
}

pub(crate) fn construct(
    slot: u16,
    generation: u32,
    seed: u16,
    position: Position,
    _map_type: MapType,
    counter: &mut u8,
) -> Option<SpiderState> {
    let phase = *counter;
    *counter = counter.wrapping_add(1);
    let rng = level2_next_rng16(seed);
    let angle = (rng & 2047).wrapping_sub(1) as i16;
    Some(SpiderState {
        slot,
        generation,
        source_id: slot,
        player: 0,
        rng,
        active: true,
        action: 161,
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
        damage: RULES.attack_damage,
        attack_phase: 0,
        attack_timer: i32::from(slot % 100),
        pending_damage: 0,
        pending_attacker: 0,
        attacker: 0,
        killer: 0,
        target: 0,
        previous: 0,
        next: 0,
        phase,
        proximity: 64,
        visual: 288,
        animation_frame: 0,
        animation_limit: 0,
        half_width: RULES.collision_half_width,
        half_height: RULES.collision_half_height,
        vertical_extent: 0,
        stage: StageState::default(),
    })
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
    pub half_width: i16,
    pub speed: i16,
    pub maximum_speed: i16,
    pub stage: StageState,
}

impl Target {
    fn living(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }
}

pub(crate) struct SpiderContext<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [Target],
    pub class3: &'a [u16],
    pub model20: &'a [u16],
    pub class5: &'a [u16],
    pub stage_runtime: Option<&'a mut dyn StageRuntime>,
    pub local_player_status: Option<(u16, i8)>,
}

impl SpiderContext<'_> {
    fn target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| self.targets.iter().find(|t| t.slot == slot).copied())
            .flatten()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SpiderEffects {
    /// sub_11900 contact mailbox, with the actor's source ID as attacker.
    pub melee: Option<(u16, i32, u16)>,
    pub web_target: Option<u16>,
    pub web_charge_on_launch: bool,
    pub web_launched_target: Option<u16>,
    pub linked_updates: Vec<Target>,
    pub kill: bool,
    pub expire: bool,
    pub summon_expire: bool,
    pub kill_credit: Option<u16>,
    pub feedback: Option<(u16, u8)>,
    pub sound: Option<u8>,
    pub population_decrements: u32,
}

fn xy(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn consume(actor: &mut SpiderState, ctx: &SpiderContext<'_>, linked_hit: bool) -> u8 {
    let mut hit = u8::from(
        consume_damage(
            &mut actor.life,
            &mut actor.pending_damage,
            &mut actor.pending_attacker,
            &mut actor.attacker,
        )
        .is_some(),
    );
    let mut next = actor.next;
    for _ in 0..=ctx.targets.len() {
        let Some(child) = ctx.target(next) else { break };
        if inherit_linked_life(
            &mut actor.life,
            &mut actor.attacker,
            child.life,
            child.attacker,
        ) {
            if linked_hit {
                hit = 1;
            }
            break;
        }
        next = child.next;
    }
    if actor.life < 0 {
        actor.killer = actor.attacker;
        hit = 2;
    }
    hit
}

fn settle(actor: &mut SpiderState, terrain: &SessionTerrain) {
    let ground = terrain.ground_height(actor.position);
    if actor.position.z > ground {
        actor.position.z = actor.position.z.wrapping_add(RULES.descent);
    }
    if i32::from(actor.position.z) <= i32::from(ground) + i32::from(RULES.ground_clearance) {
        actor.position.z = ground.wrapping_add(RULES.ground_clearance);
    }
}

fn slope(at: Position, terrain: &SessionTerrain) -> i32 {
    let x = at.x.to_be_bytes()[0];
    let y = at.y.to_be_bytes()[0];
    let height = |x, y| {
        i32::from(terrain.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let (a, b, c, d) = (
        height(x, y),
        height(x.wrapping_add(1), y),
        height(x.wrapping_add(1), y.wrapping_add(1)),
        height(x, y.wrapping_add(1)),
    );
    (a + d - b - c).abs().max((a + b - d - c).abs())
}

fn move_actor(actor: &mut SpiderState, ctx: &SpiderContext<'_>) -> bool {
    if actor.control_flags & 0x800 != 0 {
        actor.control_flags &= !0x800;
        return false;
    }
    let original = actor.position;
    let yaw = actor.yaw as u16;
    // Preserve source byte writes on the third and fourth obstruction probes.
    let third = (yaw.wrapping_sub(85) & 255) | (yaw.wrapping_sub(341) & 0x700);
    let fourth = (yaw.wrapping_add(1024) & 0x700) | (yaw & 255);
    for (attempt, heading) in [yaw & 2047, yaw.wrapping_add(341) & 2047, third, fourth]
        .into_iter()
        .enumerate()
    {
        actor.yaw = heading as i16;
        actor.position = original;
        settle(actor, ctx.terrain);
        let mut predicted = actor.position;
        actor.position = original;
        crate::move_entity(&mut predicted, actor.yaw, 0, actor.speed);
        let crosses = (predicted.x >> 8, predicted.y >> 8) != (original.x >> 8, original.y >> 8);
        let mut blocked = false;
        if crosses || attempt != 0 {
            let mut probe = predicted;
            let mut advance = 0;
            while advance <= i32::from(actor.half_width.max(0)) {
                blocked = ctx.terrain.material_mask(probe) & !RULES.terrain_mask != 0
                    || ctx.terrain.cave_collision_blocked(probe)
                    || ctx.terrain.ceiling_height(probe).is_some_and(|ceiling| {
                        ctx.terrain.angle_flags(crate::Tile {
                            x: probe.x.to_be_bytes()[0],
                            y: probe.y.to_be_bytes()[0],
                        }) & 8
                            != 0
                            || i32::from(ctx.terrain.ground_height(probe))
                                + i32::from(actor.half_height)
                                + i32::from(RULES.ground_clearance)
                                > i32::from(ceiling)
                    });
                if blocked {
                    break;
                }
                crate::move_entity(&mut probe, actor.yaw, 0, 256);
                advance += 256;
            }
            blocked |= slope(predicted, ctx.terrain) >= RULES.maximum_slope;
        }
        if !blocked {
            actor.position = predicted;
            actor.yaw =
                actor
                    .yaw
                    .wrapping_add(crate::turn_toward(actor.yaw, actor.roll, RULES.turn_limit))
                    & 2047;
            if attempt == 0 {
                actor.control_flags &= !0x40000;
            }
            return false;
        }
        actor.control_flags |= 0x40000;
    }
    actor.position = original;
    // Descriptor89 bit1 is clear; the terminal obstruction can still kill
    // through the source's water predicate.
    if RULES.behavior_flags & 1 != 0 || ctx.terrain.terrain_type(original) == 0 {
        actor.life = -1;
    }
    actor.life == -1
}

fn nearest(
    actor: &SpiderState,
    ctx: &SpiderContext<'_>,
    slots: &[u16],
    reverse: bool,
    eligible: impl Fn(Target) -> bool,
) -> Option<Target> {
    let mut best = None;
    let mut best_distance = u32::MAX;
    for target in slots
        .iter()
        .filter_map(|slot| ctx.target(*slot))
        .filter(|t| eligible(*t))
    {
        let d = xy(actor.position, target.position);
        let heading = if reverse {
            heading_for_positions(target.position, actor.position)
        } else {
            heading_for_positions(actor.position, target.position)
        };
        if d <= RULES.target_range * RULES.target_range
            && d < best_distance
            && angle_distance(i32::from(actor.yaw), heading) < RULES.target_half_angle
        {
            best = Some(target);
            best_distance = d;
        }
    }
    best
}

fn jitter(actor: &mut SpiderState) {
    let first = level2_next_rng16(actor.rng);
    actor.rng = level2_next_rng16(first);
    let delta = (2 * (i32::from(first % 157) / 79) - 1) * (i32::from(actor.rng & 255) + 85);
    actor.roll = actor.roll.wrapping_add(delta as i16) & 2047;
}

fn accept_player_target(actor: &mut SpiderState, ctx: &SpiderContext<'_>) {
    if actor.action == 162 {
        if ctx
            .target(actor.target)
            .is_none_or(|t| t.class != 3 || !matches!(t.model, 0 | 1))
        {
            actor.target = 0;
        }
        actor.attack_phase = 0;
    }
}

fn patrol(
    actor: &mut SpiderState,
    ctx: &SpiderContext<'_>,
    effects: &mut SpiderEffects,
    moving: bool,
) {
    match consume(actor, ctx, true) {
        1 => {
            actor.target = actor.attacker;
            actor.action = 162;
            settle(actor, ctx.terrain);
        }
        2 => {
            actor.action = 164;
            if !moving {
                settle(actor, ctx.terrain);
            }
        }
        _ => {
            if moving {
                effects.population_decrements += u32::from(move_actor(actor, ctx));
                if actor.phase % RULES.scan_period == 0 {
                    jitter(actor);
                    if actor.proximity != 0
                        && let Some(target) =
                            nearest(actor, ctx, ctx.class3, false, |t| t.flags & 0x20 == 0)
                    {
                        actor.target = target.slot;
                        actor.action = 162;
                    }
                }
            }
            if (!moving || (actor.proximity != 0 && actor.action == 161))
                && actor.phase % RULES.scan_period == 0
                && let Some(peer) = nearest(actor, ctx, ctx.model20, true, |t| {
                    t.slot != actor.slot && t.previous == 0
                })
            {
                actor.previous = peer.slot;
                actor.action = 163;
            }
            if !moving {
                settle(actor, ctx.terrain);
            }
        }
    }
    accept_player_target(actor, ctx);
}

/// `sub_1C310`, before its supplied ranged/contact callback. A hit or death
/// skips movement and the callback, including their cadence and RNG effects.
fn pursue(
    actor: &mut SpiderState,
    ctx: &SpiderContext<'_>,
    effects: &mut SpiderEffects,
) -> Option<Target> {
    match consume(actor, ctx, true) {
        2 => {
            actor.action = 164;
            return None;
        }
        1 => {
            actor.target = actor.attacker;
            settle(actor, ctx.terrain);
            return None;
        }
        _ => {}
    }
    effects.population_decrements += u32::from(move_actor(actor, ctx));
    let Some(target) = ctx
        .target(actor.target)
        .and_then(|t| stage::alliance_target(actor, ctx, t, effects))
        .filter(|t| t.living())
    else {
        actor.action = 161;
        return None;
    };
    if actor.phase & 3 == 0 {
        actor.roll = heading_for_positions(actor.position, target.position) as i16;
        if let Some(peer) = ctx.model20.iter().filter_map(|s| ctx.target(*s)).find(|t| {
            t.source_id != actor.source_id
                && actor.position.x.abs_diff(t.position.x) < actor.half_width as u16
                && actor.position.y.abs_diff(t.position.y) < actor.half_width as u16
        }) {
            actor.roll = heading_for_positions(peer.position, actor.position) as i16;
        }
    }
    if actor.phase % RULES.scan_period != 0 {
        return None;
    }
    if distance(actor.position, target.position) >= RULES.target_range {
        actor.action = 161;
        None
    } else {
        Some(target)
    }
}

fn linked(actor: &mut SpiderState, ctx: &SpiderContext<'_>, effects: &mut SpiderEffects) {
    if actor.previous == 0 {
        actor.action = 161;
        return;
    }
    let parent = ctx
        .target(actor.previous)
        .filter(|t| t.living() && t.class == 5 && t.model == SOURCE_MODEL);
    match consume(actor, ctx, true) {
        1 | 2 => {
            let lethal = actor.life < 0;
            if let Some(mut parent) = parent {
                parent.target = actor.attacker;
                parent.previous = 0;
                parent.action = 162;
                effects.linked_updates.push(parent);
            }
            actor.previous = 0;
            actor.target = actor.attacker;
            actor.action = if lethal { 164 } else { 162 };
            if !lethal {
                settle(actor, ctx.terrain);
            }
        }
        _ => {
            effects.population_decrements += u32::from(move_actor(actor, ctx));
            let Some(parent) = parent else {
                actor.previous = 0;
                actor.action = 161;
                return;
            };
            if actor.phase % RULES.scan_period != 0 {
                return;
            }
            match parent.action.wrapping_sub(160) {
                2 | 6 => {
                    actor.target = parent.target;
                    actor.previous = 0;
                    actor.action = parent.action;
                }
                0 | 1 | 3 => {
                    if parent.action == 163 {
                        actor.previous = parent.previous;
                    }
                    let point = ctx
                        .target(actor.previous)
                        .map_or(Position::default(), |t| t.position);
                    actor.roll = heading_for_positions(actor.position, point) as i16;
                    if let Some(peer) =
                        ctx.model20.iter().filter_map(|s| ctx.target(*s)).find(|t| {
                            t.source_id != actor.source_id
                                && i32::from(actor.position.x as i16)
                                    .abs_diff(i32::from(t.position.x as i16))
                                    < 256
                                && i32::from(actor.position.y as i16)
                                    .abs_diff(i32::from(t.position.y as i16))
                                    < 256
                        })
                    {
                        actor.roll = heading_for_positions(peer.position, actor.position) as i16;
                    }
                    actor.speed = parent.maximum_speed.wrapping_add(parent.speed);
                }
                _ => {
                    actor.previous = 0;
                    actor.action = 161;
                }
            }
        }
    }
    accept_player_target(actor, ctx);
}

/// Source callbacks160..165/167. Enabled address166 has no source dispatcher
/// case and remains unsupported, including externally forced linked state.
pub(crate) fn step(actor: &mut SpiderState, ctx: &mut SpiderContext<'_>) -> Option<SpiderEffects> {
    let mut effects = SpiderEffects::default();
    match actor.action {
        160 | 161 => {
            let moving = actor.action == 161;
            patrol(actor, ctx, &mut effects, moving);
        }
        162 => {
            let original_target = ctx.target(actor.target);
            if original_target.is_none() {
                actor.action = 161;
            } else {
                effects.sound = Some(32);
                match actor.attack_phase {
                    0 => {
                        if let Some(target) = pursue(actor, ctx, &mut effects) {
                            effects.web_target = Some(target.slot);
                            effects.web_charge_on_launch =
                                original_target.is_some_and(|t| t.class != 3 || t.model != 0);
                        }
                        // Human phase0 tests the authoritative immobilization
                        // counter even when no projectile was allocated.
                        if let Some(target) = original_target
                            && target.class == 3
                            && target.model == 0
                            && ctx
                                .local_player_status
                                .is_some_and(|(slot, value)| slot == target.slot && value != 0)
                        {
                            actor.attack_phase = 1;
                        }
                    }
                    1 | 2 => {
                        if actor.attack_phase == 1 {
                            actor.attack_phase = 2;
                            actor.attack_timer = RULES.charge_ticks;
                            actor.speed = actor.minimum_speed.wrapping_mul(2);
                        }
                        let contact = pursue(actor, ctx, &mut effects).is_some_and(|t| {
                            if distance(actor.position, t.position) < RULES.melee_range {
                                effects.melee =
                                    Some((t.slot, i32::from(actor.damage), actor.source_id));
                                true
                            } else {
                                false
                            }
                        });
                        actor.attack_timer = actor.attack_timer.wrapping_sub(1);
                        if contact || actor.attack_timer == 0 {
                            actor.attack_phase = 0;
                            actor.speed = actor.minimum_speed;
                        }
                    }
                    _ => {}
                }
            }
            if actor.action != 162 {
                actor.speed = actor.minimum_speed;
            }
        }
        163 => linked(actor, ctx, &mut effects),
        164 => {
            let mut next = actor.next;
            for _ in 0..=ctx.targets.len() {
                let Some(mut child) = ctx.target(next) else {
                    break;
                };
                next = child.next;
                child.action = 165;
                inherit_killer(&mut actor.killer, child.killer);
                effects.linked_updates.push(child);
            }
            if let Some(killer) = ctx.target(actor.killer)
                && crate::combat::actors::awards_creature_kill(
                    SOURCE_MODEL,
                    actor.source_id,
                    killer.source_id,
                    killer.class,
                    killer.model,
                )
            {
                effects.kill_credit = Some(killer.slot);
            }
            actor.action = 165;
        }
        165 => effects.kill = actor.phase & 7 == 0,
        167 => {
            stage::step(actor, ctx, &mut effects);
            if actor.action == 162 {
                actor.attack_phase = 0;
            }
        }
        _ => return None,
    }
    Some(effects)
}
