//! Source model26: descriptor99 roam/acquisition, linked-peer response, and
//! action210's mana-drain/spell-theft transaction. The source row remains the
//! mutable authority; campaign-owned effects cross this boundary by handle.

use crate::campaign::source_arena::SourceEntityHandle;
use crate::level_definition::{CREATURE26_CLASS_PROPERTIES, Creature26ClassProperties};
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, RuntimeRelation, RuntimeSnapshot, SessionTerrain,
};
use crate::{FlightWorld, Position, angle_distance, heading_for_positions};

pub(crate) mod mc2;
mod stage;
#[cfg(test)]
mod tests;

pub(crate) const SOURCE_MODEL: u8 = 26;
pub(crate) const STATIONARY_ACTION: u8 = 208;
pub(crate) const ROAM_ACTION: u8 = 209;
pub(crate) const DRAIN_ACTION: u8 = 210;
pub(crate) const LINK_ACTION: u8 = 211;
pub(crate) const PREKILL_ACTION: u8 = 212;
pub(crate) const KILL_ACTION: u8 = 213;
pub(crate) const STAGE_ACTION: u8 = 215;
pub(crate) const DESCRIPTOR: u16 = 99;
pub(crate) const PARTICLE: u16 = 318;

pub(crate) const RULES: Creature26ClassProperties = CREATURE26_CLASS_PROPERTIES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpellOffer {
    pub family: u8,
    pub holder: SourceEntityHandle,
    /// Source word36. sub_69300 leaves a linked holder untouched.
    pub holder_thief: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Target {
    pub handle: SourceEntityHandle,
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
    pub player_entity: u16,
    pub mana: i32,
    pub mana_regen: i32,
    pub left_spell: Option<SpellOffer>,
    pub right_spell: Option<SpellOffer>,
    /// Raw source word at record bytes154..156, retained across peer actions.
    pub target: u16,
    pub next: u16,
    pub parent: u16,
    pub stage_mode: u8,
    pub link: u16,
    pub attacker: u16,
    pub killer: u16,
    pub actual_speed: i16,
    pub maximum_speed: i16,
}

impl Target {
    fn living(self) -> bool {
        self.life >= 0 && self.flags & ENTITY_DRAW_DISABLED_BIT == 0
    }

    fn drainable(self) -> bool {
        self.class == 3 && matches!(self.model, 0 | 1) && self.living()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpellHand {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpellTheft {
    pub thief: SourceEntityHandle,
    pub victim: SourceEntityHandle,
    pub victim_player: u16,
    pub holder: SourceEntityHandle,
    pub family: u8,
    pub hand: SpellHand,
    pub victim_position: Position,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ManaDrain {
    pub target: SourceEntityHandle,
    pub amount: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PeerUpdate {
    pub peer: SourceEntityHandle,
    pub action: Option<u8>,
    pub target: Option<u16>,
    pub link: Option<u16>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Effects {
    pub mana_drain: Option<ManaDrain>,
    pub spell_theft: Option<SpellTheft>,
    pub peer_updates: Vec<PeerUpdate>,
    pub sound: Option<u8>,
    pub earlier_feedback: Option<(u16, u8)>,
    /// Source owner and spell family awarded by the summon hit helper.
    pub feedback: Option<(u16, u8)>,
    /// Generation-bound linked actors whose Alliance blink flags change.
    pub linked_blink_flags: Vec<(SourceEntityHandle, u32)>,
    pub prekill: bool,
    /// Source killer slot from action212; the MC2 adapter commits and clears it.
    pub kill_credit: Option<u16>,
    pub kill: bool,
    pub population_decrements: u32,
    /// StageVar handlers that source-disable the row after their final visit.
    pub stage_hide: bool,
    /// `sub_1E580`: both modes emit an impact; mode13 also emits model73.
    pub stage_expiry: Option<bool>,
}

pub(crate) struct Actor<'a> {
    pub row: &'a mut EntitySnapshot,
    pub handle: SourceEntityHandle,
}

impl Actor<'_> {
    fn target(&self) -> u16 {
        word(self.row, 154)
    }

    fn set_target(&mut self, slot: u16) {
        put(self.row, 154, slot);
    }

    fn link(&self) -> u16 {
        word(self.row, 54)
    }

    fn set_link(&mut self, slot: u16) {
        put(self.row, 54, slot);
    }

    fn roll(&self) -> i16 {
        self.row.source_roll()
    }

    fn set_roll(&mut self, roll: i16) {
        self.row.set_source_roll(roll);
    }

    fn attacker(&self) -> u16 {
        word(self.row, 42)
    }
}

pub(crate) struct Context<'a> {
    pub rules: &'a Creature26ClassProperties,
    pub terrain: &'a SessionTerrain,
    /// Current active arena rows; links resolve through this live view.
    pub targets: &'a [Target],
    /// Frozen source membership for the class3 scan.
    pub class3: &'a [u16],
    /// Frozen source membership for model26 peer scans.
    pub model26: &'a [u16],
    /// StageVar table consumed by action215 (`sub_1D5D0` in remc2).
    pub runtime: Option<&'a RuntimeSnapshot>,
    /// The shared source D41A0 random stream used by action210 theft.
    pub global_rng: &'a mut u32,
}

impl Context<'_> {
    fn any_target(&self, slot: u16) -> Option<Target> {
        (slot != 0)
            .then(|| {
                self.targets
                    .iter()
                    .find(|target| target.slot == slot)
                    .copied()
            })
            .flatten()
    }

    fn player_target(&self, slot: u16) -> Option<Target> {
        (slot != 0 && self.class3.contains(&slot))
            .then(|| self.any_target(slot))
            .flatten()
    }

    fn model26_target(&self, slot: u16) -> Option<Target> {
        (slot != 0 && self.model26.contains(&slot))
            .then(|| self.any_target(slot))
            .flatten()
    }
}

fn word(row: &EntitySnapshot, at: usize) -> u16 {
    u16::from_le_bytes([row.record[at], row.record[at + 1]])
}

fn put(row: &mut EntitySnapshot, at: usize, value: u16) {
    row.record[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn distance_squared_xy(a: Position, b: Position) -> u32 {
    let dx = i32::from(b.x.wrapping_sub(a.x) as i16);
    let dy = i32::from(b.y.wrapping_sub(a.y) as i16);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

fn settle(position: &mut Position, rules: &Creature26ClassProperties, terrain: &SessionTerrain) {
    let ground = terrain.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(rules.descent);
    }
    if position.z <= ground.wrapping_add(rules.ground_clearance) {
        position.z = ground.wrapping_add(rules.ground_clearance);
    }
}

/// Source sub_1B8C0's descriptor99 movement probe. Failed retries preserve
/// their yaw; a fatal profile obstruction sets life negative for next-pass
/// action212 instead of reclaiming from this callback.
fn move_source(actor: &mut Actor<'_>, ctx: &Context<'_>) -> bool {
    if actor.row.control_flags & 0x800 != 0 {
        actor
            .row
            .set_control_flags(actor.row.control_flags & !0x800);
        return false;
    }

    let motion = crate::ground_creature_motion::step_ground_creature_motion(
        actor.row.position,
        actor.row.yaw,
        actor.roll(),
        actor.row.actual_speed,
        ctx.terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: ctx.rules.collision_half_width,
            half_height: ctx.rules.collision_half_height,
            ground_clearance: ctx.rules.ground_clearance,
            descent: ctx.rules.descent,
            turn_limit: ctx.rules.turn_limit,
            terrain_mask: ctx.rules.terrain_mask,
            maximum_tile_slope: ctx.rules.maximum_slope,
        },
    );
    actor.row.set_position(motion.position);
    actor.row.set_yaw(motion.yaw);
    if motion.movement_blocked {
        actor
            .row
            .set_control_flags(actor.row.control_flags | 0x40000);
    } else {
        actor
            .row
            .set_control_flags(actor.row.control_flags & !0x40000);
    }
    if motion.killed && ctx.rules.behavior_flags & 1 != 0 {
        actor.row.set_life(-1);
    }
    motion.killed && actor.row.life < 0
}

fn consume_damage(actor: &mut Actor<'_>, ctx: &Context<'_>) -> bool {
    let mut life = actor.row.life;
    let mut damage = i32::from_le_bytes(
        actor.row.record[98..102]
            .try_into()
            .expect("source damage dword extent"),
    );
    let mut pending_attacker = word(actor.row, 102);
    let mut attacker = actor.attacker();
    let direct = crate::combat::actors::consume_damage(
        &mut life,
        &mut damage,
        &mut pending_attacker,
        &mut attacker,
    )
    .is_some();

    let mut hit = direct;
    let mut next = word(actor.row, 56);
    for _ in 0..=ctx.targets.len() {
        let Some(linked) = ctx.any_target(next) else {
            break;
        };
        if crate::combat::actors::inherit_linked_life(
            &mut life,
            &mut attacker,
            linked.life,
            linked.attacker,
        ) {
            hit = true;
            break;
        }
        next = linked.next;
    }

    actor.row.set_life(life);
    actor.row.record[98..102].copy_from_slice(&damage.to_le_bytes());
    put(actor.row, 102, pending_attacker);
    put(actor.row, 42, attacker);
    if life < 0 {
        put(actor.row, 40, attacker);
        actor.row.set_source_action(PREKILL_ACTION);
    } else if hit {
        actor.set_target(attacker);
    } else {
        put(actor.row, 42, 0);
    }
    hit
}

fn random_roam_heading(actor: &mut Actor<'_>) {
    let mut rng = actor.row.entity_rng;
    rng = rng.wrapping_mul(9377).wrapping_add(9439);
    let first = rng;
    rng = rng.wrapping_mul(9377).wrapping_add(9439);
    let sign = if (first % 157) / 79 == 0 {
        -1_i32
    } else {
        1_i32
    };
    let delta = ((rng & 0xff) as i32 + 85) * sign;
    actor.set_roll((i32::from(actor.roll()).wrapping_add(delta) & 2047) as i16);
    actor.row.set_entity_rng(rng);
}

fn scan_target(actor: &Actor<'_>, ctx: &Context<'_>) -> Option<Target> {
    let range_squared = ctx.rules.target_range.wrapping_mul(ctx.rules.target_range);
    let mut best = None;
    let mut best_distance = u32::MAX;
    for candidate in ctx
        .class3
        .iter()
        .filter_map(|slot| ctx.player_target(*slot))
        .filter(|target| target.flags & 0x20 == 0)
    {
        let distance_squared = distance_squared_xy(actor.row.position, candidate.position);
        if distance_squared <= range_squared
            && distance_squared < best_distance
            && angle_distance(
                i32::from(actor.row.yaw),
                heading_for_positions(actor.row.position, candidate.position),
            )
            .cast_unsigned()
                < ctx.rules.target_half_angle.cast_unsigned()
        {
            best = Some(candidate);
            best_distance = distance_squared;
        }
    }
    best
}

fn scan_peer(actor: &Actor<'_>, ctx: &Context<'_>) -> Option<Target> {
    let range_squared = ctx.rules.target_range.wrapping_mul(ctx.rules.target_range);
    let mut best = None;
    let mut best_distance = u32::MAX;
    for candidate in ctx
        .model26
        .iter()
        .filter_map(|slot| ctx.model26_target(*slot))
        .filter(|target| target.slot != actor.row.index && target.link == 0)
    {
        let distance_squared = distance_squared_xy(actor.row.position, candidate.position);
        if distance_squared <= range_squared
            && distance_squared < best_distance
            && angle_distance(
                i32::from(actor.row.yaw),
                heading_for_positions(actor.row.position, candidate.position),
            )
            .cast_unsigned()
                < ctx.rules.target_half_angle.cast_unsigned()
        {
            best = Some(candidate);
            best_distance = distance_squared;
        }
    }
    best
}

fn generic_tail(actor: &mut Actor<'_>) {
    // sub_293B0 affects only the acquired response action210.
    if actor.row.action == DRAIN_ACTION {
        actor
            .row
            .set_control_flags(actor.row.control_flags & !0x800000);
        let minimum = i16::from_le_bytes([actor.row.record[136], actor.row.record[137]]);
        actor.row.set_actual_speed(minimum);
    }
}

fn drain_tail(actor: &mut Actor<'_>) {
    // sub_293D0 deliberately does nothing while action remains210. Explicit
    // returns from sub_28FF0 retain target/speed; it performs no sub_1EEE0
    // terrain correction on this path.
    if actor.row.action != DRAIN_ACTION {
        actor.set_target(0);
        actor
            .row
            .set_control_flags(actor.row.control_flags | 0x800000);
        let maximum = i16::from_le_bytes([actor.row.record[138], actor.row.record[139]]);
        actor.row.set_actual_speed(maximum);
    }
}

fn stationary_step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Effects {
    let hit = consume_damage(actor, ctx);
    if hit && actor.row.life >= 0 {
        actor.row.set_source_action(DRAIN_ACTION);
    } else if !hit
        && actor.row.life >= 0
        && ctx.rules.scan_period != 0
        && actor.row.phase % ctx.rules.scan_period == 0
        && ctx.rules.behavior_flags & 4 == 0
        && let Some(peer) = scan_peer(actor, ctx)
    {
        actor.set_link(peer.slot);
        actor.row.set_source_action(LINK_ACTION);
    }
    let mut position = actor.row.position;
    settle(&mut position, ctx.rules, ctx.terrain);
    actor.row.set_position(position);
    generic_tail(actor);
    Effects::default()
}

fn roam_step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Effects {
    let hit = consume_damage(actor, ctx);
    if actor.row.life < 0 {
        return Effects::default();
    }
    if hit {
        actor.row.set_source_action(DRAIN_ACTION);
        let mut position = actor.row.position;
        settle(&mut position, ctx.rules, ctx.terrain);
        actor.row.set_position(position);
        generic_tail(actor);
        return Effects::default();
    }

    let obstructed = move_source(actor, ctx);
    if ctx.rules.scan_period != 0 && actor.row.phase % ctx.rules.scan_period == 0 {
        random_roam_heading(actor);
        if actor.row.record[61] != 0
            && let Some(target) = scan_target(actor, ctx)
        {
            actor.set_target(target.slot);
            let action = if ctx.rules.behavior_flags & 8 != 0 {
                214
            } else {
                DRAIN_ACTION
            };
            actor.row.set_source_action(action);
        } else if ctx.rules.behavior_flags & 4 == 0
            && let Some(peer) = scan_peer(actor, ctx)
        {
            actor.set_link(peer.slot);
            actor.row.set_source_action(LINK_ACTION);
        }
    }
    generic_tail(actor);
    let mut effects = Effects::default();
    effects.population_decrements = u32::from(obstructed && actor.row.life < 0);
    effects
}

fn steal(actor: &Actor<'_>, ctx: &mut Context<'_>, target: Target) -> Option<SpellTheft> {
    if !target.drainable() || target.model != 0 {
        return None;
    }
    let roll = (*ctx.global_rng).wrapping_mul(9377).wrapping_add(9439);
    *ctx.global_rng = roll;
    let selected = roll % u32::from(ctx.rules.theft_rng_modulus);
    let (hand, offer) = if selected == u32::from(ctx.rules.right_hand_roll) {
        (SpellHand::Right, target.right_spell)
    } else if selected == u32::from(ctx.rules.left_hand_roll) {
        (SpellHand::Left, target.left_spell)
    } else {
        return None;
    };
    let offer = offer?;
    if offer.holder_thief != 0 {
        return None;
    }
    Some(SpellTheft {
        thief: actor.handle,
        victim: target.handle,
        victim_player: target.player_entity,
        holder: offer.holder,
        family: offer.family,
        hand,
        victim_position: target.position,
    })
}

fn drain_step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<Effects, String> {
    let mut effects = Effects::default();
    if actor.row.phase & 0x1f == 0 {
        effects.sound = Some(62);
    }
    consume_damage(actor, ctx);
    if actor.row.life < 0 {
        drain_tail(actor);
        return Ok(effects);
    }

    let obstructed = move_source(actor, ctx);
    effects.population_decrements = u32::from(obstructed && actor.row.life < 0);
    if let Some(target) = ctx
        .player_target(actor.target())
        .filter(|target| target.drainable())
    {
        if actor.row.phase & ctx.rules.theft_cadence_mask == 0 {
            actor.set_roll(heading_for_positions(actor.row.position, target.position) as i16);
            for peer in
                ctx.model26
                    .iter()
                    .filter_map(|slot| ctx.model26_target(*slot))
                    .filter(|peer| {
                        peer.source_id != actor.row.source_id
                            && (i32::from(actor.row.position.x.wrapping_sub(peer.position.x) as i16)
                                .abs() < i32::from(actor.row.pitch))
                            && (i32::from(actor.row.position.y.wrapping_sub(peer.position.y) as i16)
                                .abs() < i32::from(actor.row.pitch))
                    })
            {
                actor.set_roll(heading_for_positions(peer.position, actor.row.position) as i16);
                break;
            }
        }

        effects.mana_drain = Some(ManaDrain {
            target: target.handle,
            amount: target.mana_regen.wrapping_add(ctx.rules.mana_drain_bonus),
        });
        if actor.row.phase & ctx.rules.theft_cadence_mask != 0 {
            return Ok(effects);
        }
        let target_distance = distance(actor.row.position, target.position);
        if target_distance <= ctx.rules.target_range {
            if target_distance < ctx.rules.theft_range && target.model == 0 {
                effects.spell_theft = steal(actor, ctx, target);
            }
            return Ok(effects);
        }
    }

    // Only the fall-through exit changes action210 back to209. sub_293D0 then
    // clears its target and restores max speed at the same-pass tail.
    actor.row.set_source_action(ROAM_ACTION);
    drain_tail(actor);
    Ok(effects)
}

fn link_step(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Effects {
    let mut effects = Effects::default();
    let mut peer = ctx.model26_target(actor.link()).filter(|peer| {
        peer.life >= 0
            && peer.flags & ENTITY_DRAW_DISABLED_BIT == 0
            && peer.class == 5
            && peer.model == SOURCE_MODEL
    });
    let hit = consume_damage(actor, ctx);
    if actor.row.life < 0 {
        if let Some(peer) = peer {
            effects.peer_updates.push(PeerUpdate {
                peer: peer.handle,
                action: Some(DRAIN_ACTION),
                target: Some(word(actor.row, 42)),
                link: Some(0),
            });
        }
        actor.set_link(0);
        actor.row.set_source_action(PREKILL_ACTION);
        return effects;
    }
    if hit {
        if let Some(peer) = peer {
            effects.peer_updates.push(PeerUpdate {
                peer: peer.handle,
                action: Some(DRAIN_ACTION),
                target: Some(actor.attacker()),
                link: Some(0),
            });
        }
        actor.set_target(actor.attacker());
        actor.set_link(0);
        actor.row.set_source_action(DRAIN_ACTION);
        let mut position = actor.row.position;
        settle(&mut position, ctx.rules, ctx.terrain);
        actor.row.set_position(position);
        return effects;
    }
    let Some(current_peer) = peer.take() else {
        actor.set_link(0);
        actor.row.set_source_action(ROAM_ACTION);
        return effects;
    };

    let obstructed = move_source(actor, ctx);
    effects.population_decrements = u32::from(obstructed && actor.row.life < 0);
    if ctx.rules.scan_period != 0 && actor.row.phase % ctx.rules.scan_period == 0 {
        match current_peer.action.wrapping_sub(STATIONARY_ACTION) {
            0 | 1 => {
                actor.set_roll(
                    heading_for_positions(actor.row.position, current_peer.position) as i16,
                );
                if let Some(other) = ctx
                    .model26
                    .iter()
                    .filter_map(|slot| ctx.model26_target(*slot))
                    .find(|other| {
                        other.source_id != actor.row.source_id
                            && i32::from(actor.row.position.x.wrapping_sub(other.position.x) as i16)
                                .abs()
                                < i32::from(actor.row.pitch)
                            && i32::from(actor.row.position.y.wrapping_sub(other.position.y) as i16)
                                .abs()
                                < i32::from(actor.row.pitch)
                    })
                {
                    actor
                        .set_roll(heading_for_positions(other.position, actor.row.position) as i16);
                }
            }
            2 => {
                actor.set_target(current_peer.target);
                actor.set_link(0);
                actor.row.set_source_action(DRAIN_ACTION);
                return effects;
            }
            6 => {
                actor.set_target(current_peer.target);
                actor.set_link(0);
                actor.row.set_source_action(214);
                return effects;
            }
            3 => {
                actor.set_link(current_peer.link);
                if let Some(next) = ctx.model26_target(current_peer.link) {
                    actor.set_roll(heading_for_positions(actor.row.position, next.position) as i16);
                }
            }
            _ => {
                actor.set_link(0);
                actor.row.set_source_action(ROAM_ACTION);
            }
        }
        if matches!(
            current_peer.action.wrapping_sub(STATIONARY_ACTION),
            0 | 1 | 3
        ) {
            actor.row.set_actual_speed(
                current_peer
                    .maximum_speed
                    .wrapping_add(actor.row.actual_speed),
            );
        }
    }
    effects
}

fn prekill(actor: &mut Actor<'_>, ctx: &Context<'_>) -> Effects {
    let mut effects = Effects {
        prekill: true,
        ..Effects::default()
    };
    let mut child = word(actor.row, 56);
    for _ in 0..ctx.targets.len() {
        let Some(linked) = ctx.any_target(child) else {
            break;
        };
        effects.peer_updates.push(PeerUpdate {
            peer: linked.handle,
            action: Some(KILL_ACTION),
            target: None,
            link: None,
        });
        if linked.killer != 0 {
            put(actor.row, 40, linked.killer);
        }
        child = linked.next;
    }
    let killer = word(actor.row, 40);
    if let Some(target) = ctx.player_target(killer)
        && target.model == 0
        && target.source_id != actor.row.source_id
    {
        effects.kill_credit = Some(target.slot);
    }
    actor.row.set_source_action(KILL_ACTION);
    effects
}

/// Handle one ordinary model26 source action. Actions208/209 keep their
/// separate stationary/roaming scans; 210 drains and steals; 211 coordinates a
/// peer; 212 schedules prekill propagation; 213 reaches the shared terminal
/// helper on source cadence; and215 runs its source StageVar callback. Source
/// dispatch has no action214 case, so it falls through to the unsupported-
/// action error below.
pub(crate) fn step(actor: &mut Actor<'_>, ctx: &mut Context<'_>) -> Result<Effects, String> {
    match actor.row.action {
        STATIONARY_ACTION => Ok(stationary_step(actor, ctx)),
        ROAM_ACTION => Ok(roam_step(actor, ctx)),
        DRAIN_ACTION => drain_step(actor, ctx),
        LINK_ACTION => {
            let effects = link_step(actor, ctx);
            generic_tail(actor);
            Ok(effects)
        }
        PREKILL_ACTION => Ok(prekill(actor, ctx)),
        KILL_ACTION => Ok(Effects {
            kill: actor.row.phase & 7 == 0,
            ..Effects::default()
        }),
        STAGE_ACTION => {
            let effects = stage::step(actor, ctx)?;
            if effects.stage_expiry.is_none() {
                generic_tail(actor);
            }
            Ok(effects)
        }
        action => Err(format!("unsupported model26 action{action}")),
    }
}
