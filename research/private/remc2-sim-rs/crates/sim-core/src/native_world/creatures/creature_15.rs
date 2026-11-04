//! Castle defender constructor and source actions 120–127.
//!
//! The arena adapter owns allocation and frozen memberships. This module owns
//! the live actor, including its mailbox, links, StageVar route and lifecycle.

use crate::combat::actors::{consume_damage, inherit_killer, inherit_linked_life};
use crate::level_definition::CASTLE_DEFENDER_CLASS_PROPERTIES as PROPERTIES;
use crate::native_world::{ENTITY_DRAW_DISABLED_BIT, MapType, SessionTerrain, level2_next_rng16};
use crate::{FlightWorld, Position, Tile, angle_distance, heading_for_positions};

pub(crate) mod mc2;
#[cfg(test)]
mod tests;

pub(crate) use super::archer::{
    ArcherStageDescriptor as StageDescriptor, ArcherStageRuntime as StageRuntime,
    ArcherStageState as StageState,
};

pub(crate) const SOURCE_CLASS: u8 = 5;
pub(crate) const SOURCE_MODEL: u8 = 15;
pub(crate) const DESCRIPTOR: u16 = 83;
pub(crate) const CADENCE: u8 = PROPERTIES.scan_period;
pub(crate) const TARGET_RANGE: u32 = PROPERTIES.target_range;
pub(crate) const TARGET_HALF_ANGLE: i16 = PROPERTIES.target_half_angle;
pub(crate) const MINIMUM_SPEED: i16 = PROPERTIES.minimum_speed;
pub(crate) const TERRAIN_MASK: u32 = PROPERTIES.terrain_mask;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefenderOrigin {
    Castle(u16),
    InitialPackage,
    AuthoredRecord(u16),
}

/// A single authoritative class-5/model-15 row; Castle ownership is an arena
/// handle, independently of the source ID copied into the actor by 5FF50.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CastleDefenderState {
    pub source_entity_index: u16,
    pub slot_generation: u32,
    pub(crate) origin: DefenderOrigin,
    pub(crate) castle_owner: Option<crate::campaign::source_arena::SourceEntityHandle>,
    pub entity_rng: u16,
    pub id_entity_index: u16,
    pub player_entity_index: u16,
    pub active: bool,
    pub control_flags: u32,
    pub action: u8,
    pub position: Position,
    pub yaw: u16,
    pub pitch: i16,
    pub orientation_fov: i16,
    pub roll: u16,
    pub speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub life: i32,
    pub max_life: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub target_sphere_slot: u16,
    pub chain_previous_slot: u16,
    pub chain_next_slot: u16,
    pub pending_damage: i32,
    pub pending_attacker_entity_index: u16,
    pub damage_attacker_entity_index: u16,
    pub killer_entity_index: u16,
    pub source_byte_3e: u8,
    pub source_byte_39: u8,
    pub source_dword_10: i32,
    pub source_subspell_index: u16,
    pub byte_38: u8,
    pub xtype: u8,
    pub xsubtype: u8,
    pub visual_index: u16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    /// Source `array_0x52.yaw`, the vertical collision-centre offset.
    pub collision_vertical_offset: i16,
    pub fov: i16,
    pub proximity_delay: u8,
    pub(crate) stage: StageState,
}

impl CastleDefenderState {
    /// `sub_4C1E0`, after NewEvent has allocated/seeded this fixed row.
    /// Allocation failure must not advance the shared model counter.
    pub(crate) fn construct(
        source_entity_index: u16,
        slot_generation: u32,
        entity_rng: u16,
        position: Position,
        model_counter: &mut u8,
        origin: DefenderOrigin,
    ) -> Self {
        let phase = *model_counter;
        *model_counter = model_counter.wrapping_add(1);
        Self {
            source_entity_index,
            slot_generation,
            origin,
            entity_rng,
            castle_owner: None,
            id_entity_index: source_entity_index,
            player_entity_index: 0,
            active: true,
            control_flags: 0x0002_0008,
            action: 121,
            position,
            yaw: 0,
            pitch: 0,
            orientation_fov: 0,
            roll: 0,
            speed: MINIMUM_SPEED,
            minimum_speed: MINIMUM_SPEED,
            maximum_speed: PROPERTIES.maximum_speed,
            life: PROPERTIES.max_life as i32,
            max_life: PROPERTIES.max_life as i32,
            mana: PROPERTIES.mana,
            max_mana: PROPERTIES.max_mana,
            target_sphere_slot: 0,
            chain_previous_slot: 0,
            chain_next_slot: 0,
            pending_damage: 0,
            pending_attacker_entity_index: 0,
            damage_attacker_entity_index: 0,
            killer_entity_index: 0,
            source_byte_3e: phase,
            source_byte_39: CADENCE.wrapping_sub(phase % CADENCE).wrapping_add(4),
            source_dword_10: i32::from(source_entity_index % 100),
            source_subspell_index: PROPERTIES.attack_damage,
            byte_38: 1,
            xtype: 3,
            xsubtype: u8::MAX,
            visual_index: 0,
            shift_rot_x: PROPERTIES.collision_half_width,
            shift_rot_y: PROPERTIES.collision_half_height,
            collision_vertical_offset: 125,
            fov: PROPERTIES.collision_half_height,
            proximity_delay: 0,
            stage: StageState::default(),
        }
    }

    /// The ordinary Castle tail writes its owner relations after construction.
    pub(crate) fn bind_castle(&mut self, castle_id: u16, terrain: &SessionTerrain) {
        self.id_entity_index = castle_id;
        self.player_entity_index = castle_id;
        self.yaw = 512;
        self.roll = 512;
        self.position.x = self.position.x.wrapping_add(128);
        self.position.y = self.position.y.wrapping_add(640);
        self.position.z = terrain.ground_height(self.position);
    }

    pub(crate) fn drawing_enabled(&self) -> bool {
        self.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
    }

    pub fn drawing_disabled(&self) -> bool {
        !self.drawing_enabled()
    }

    pub(crate) fn target_view(&self) -> DefenderTarget {
        DefenderTarget {
            slot: self.source_entity_index,
            source_id: self.id_entity_index,
            class: SOURCE_CLASS,
            model: SOURCE_MODEL,
            action: self.action,
            life: self.life,
            control_flags: self.control_flags,
            position: self.position,
            previous: self.chain_previous_slot,
            next: self.chain_next_slot,
            attacker: self.damage_attacker_entity_index,
            killer: self.killer_entity_index,
            target: self.target_sphere_slot,
            yaw: self.yaw,
            roll: self.roll,
            speed: self.speed,
            maximum_speed: self.maximum_speed,
            half_width: self.shift_rot_x,
            parent: self.stage.parent,
            stage: self.stage,
        }
    }

    pub(crate) fn hide(&mut self) {
        self.control_flags |= ENTITY_DRAW_DISABLED_BIT;
    }

    pub(crate) fn apply_construction_kill(&mut self, source_id: u16) {
        self.apply_source_kill(source_id, Some(source_id));
    }

    pub(crate) fn apply_source_kill(&mut self, killer: u16, attacker: Option<u16>) {
        self.life = -1;
        self.killer_entity_index = killer;
        if let Some(attacker) = attacker {
            self.damage_attacker_entity_index = attacker;
        }
    }

    /// Called only for an enabled callback, on the current fixed-slot occupant.
    pub(crate) fn finish_source_visit(&mut self) {
        self.source_byte_3e = self.source_byte_3e.wrapping_add(1);
    }

    pub(crate) fn reclaimable(&self) -> bool {
        self.active && !self.drawing_enabled()
    }

    pub(crate) fn hash_source_state(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        let (origin, relation) = match self.origin {
            DefenderOrigin::Castle(slot) => (0, slot),
            DefenderOrigin::InitialPackage => (1, 0),
            DefenderOrigin::AuthoredRecord(record) => (2, record),
        };
        hasher.update([origin]);
        hasher.update(relation.to_le_bytes());
        hasher.update(self.slot_generation.to_le_bytes());
        hasher.update([u8::from(self.castle_owner.is_some())]);
        if let Some(owner) = self.castle_owner {
            hasher.update(owner.source_slot.to_le_bytes());
            hasher.update(owner.generation.to_le_bytes());
        }
        for value in [
            self.source_entity_index,
            self.entity_rng,
            self.id_entity_index,
            self.player_entity_index,
            self.position.x,
            self.position.y,
            self.yaw,
            self.roll,
            self.target_sphere_slot,
            self.chain_previous_slot,
            self.chain_next_slot,
            self.pending_attacker_entity_index,
            self.damage_attacker_entity_index,
            self.killer_entity_index,
            self.source_subspell_index,
            self.visual_index,
            self.stage.relation,
            self.stage.parent,
        ] {
            hasher.update(value.to_le_bytes());
        }
        for value in [
            self.position.z,
            self.pitch,
            self.orientation_fov,
            self.speed,
            self.minimum_speed,
            self.maximum_speed,
            self.shift_rot_x,
            self.shift_rot_y,
            self.collision_vertical_offset,
            self.fov,
            self.stage.remaining,
            self.stage.duration,
        ] {
            hasher.update(value.to_le_bytes());
        }
        for value in [
            self.life,
            self.max_life,
            self.mana,
            self.max_mana,
            self.pending_damage,
            self.source_dword_10,
        ] {
            hasher.update(value.to_le_bytes());
        }
        hasher.update(self.control_flags.to_le_bytes());
        hasher.update([
            u8::from(self.active),
            self.action,
            self.source_byte_3e,
            self.source_byte_39,
            self.byte_38,
            self.xtype,
            self.xsubtype,
            self.proximity_delay,
            self.stage.index,
            self.stage.mode,
        ]);
    }
}

/// Live pointed-to fields; list membership is supplied separately and frozen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DefenderTarget {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub life: i32,
    pub control_flags: u32,
    pub position: Position,
    pub previous: u16,
    pub next: u16,
    pub attacker: u16,
    pub killer: u16,
    pub target: u16,
    pub yaw: u16,
    pub roll: u16,
    pub speed: i16,
    pub maximum_speed: i16,
    pub half_width: i16,
    pub parent: u16,
    pub stage: StageState,
}

pub(crate) struct DefenderContext<'a> {
    pub terrain: &'a SessionTerrain,
    pub targets: &'a [DefenderTarget],
    pub class3: &'a [u16],
    pub model15: &'a [u16],
    pub class5: &'a [u16],
    pub stage_runtime: Option<&'a mut dyn StageRuntime>,
}

impl DefenderContext<'_> {
    fn target(&self, slot: u16) -> Option<DefenderTarget> {
        (slot != 0)
            .then(|| self.targets.iter().find(|row| row.slot == slot).copied())
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DefenderArrowRequest {
    pub owner: u16,
    pub position: Position,
    pub fov: i16,
    pub target: DefenderTarget,
    pub word96: u16,
    pub xtype: u8,
    pub xsubtype: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DefenderEffects {
    pub arrow: Option<DefenderArrowRequest>,
    pub linked_updates: Vec<DefenderTarget>,
    pub kill: bool,
    pub expire: bool,
    pub summon_expire: bool,
    pub feedback: Option<(u16, u8)>,
    pub target_music: Option<u16>,
    pub population_decrements: u32,
}

fn distance_xy(a: Position, b: Position) -> u32 {
    let x = i32::from(a.x.wrapping_sub(b.x).cast_signed());
    let y = i32::from(a.y.wrapping_sub(b.y).cast_signed());
    x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as u32
}

fn distance(a: Position, b: Position) -> u32 {
    crate::legacy_sqrt(crate::native_world::level2_wrapping_source_distance_3d(
        a, b,
    ))
}

fn consume_mailbox(actor: &mut CastleDefenderState, context: &DefenderContext<'_>) -> u8 {
    let mut state = u8::from(
        consume_damage(
            &mut actor.life,
            &mut actor.pending_damage,
            &mut actor.pending_attacker_entity_index,
            &mut actor.damage_attacker_entity_index,
        )
        .is_some(),
    );
    let mut slot = actor.chain_next_slot;
    for _ in 0..=context.targets.len() {
        let Some(linked) = context.target(slot) else {
            break;
        };
        if inherit_linked_life(
            &mut actor.life,
            &mut actor.damage_attacker_entity_index,
            linked.life,
            linked.attacker,
        ) {
            state = 1;
            break;
        }
        slot = linked.next;
    }
    if actor.life < 0 {
        actor.killer_entity_index = actor.damage_attacker_entity_index;
        state = 2;
    }
    state
}

fn settle(actor: &mut CastleDefenderState, terrain: &SessionTerrain) {
    let ground = terrain.ground_height(actor.position);
    if actor.position.z > ground {
        actor.position.z = actor.position.z.wrapping_add(PROPERTIES.descent);
    }
    if actor.position.z <= ground {
        actor.position.z = ground;
    }
}

pub(crate) fn probe_blocked(terrain: &SessionTerrain, candidate: Position, fov: i16) -> bool {
    if terrain.material_mask(candidate) & !TERRAIN_MASK != 0 {
        return true;
    }
    terrain.map_type == MapType::Cave
        && (terrain.angle_flags(Tile {
            x: candidate.x.to_be_bytes()[0],
            y: candidate.y.to_be_bytes()[0],
        }) & 8
            != 0
            || terrain.ceiling_height(candidate).is_some_and(|ceiling| {
                i32::from(terrain.ground_height(candidate)) + i32::from(fov) > i32::from(ceiling)
            }))
}

/// `sub_102D0(..., 1)` checks the whole collision extent, advancing from
/// the predicted point along the actor's current yaw, not the trial yaw.
fn roam_probe_blocked(actor: &CastleDefenderState, terrain: &SessionTerrain, candidate: Position) -> bool {
    let mut point = candidate;
    let mut distance = 0;
    while distance <= i32::from(actor.shift_rot_x.max(actor.shift_rot_y)) {
        if probe_blocked(terrain, point, actor.fov) {
            return true;
        }
        crate::move_entity(&mut point, actor.yaw as i16, 0, 256);
        distance += 256;
    }
    false
}

/// `sub_24190`: four RNG probes, grid alignment and first overlapping peer.
pub(crate) fn roam(actor: &mut CastleDefenderState, context: &DefenderContext<'_>) {
    if actor.source_byte_3e & 7 == 0 {
        if context.terrain.material_mask(actor.position) & !TERRAIN_MASK != 0 {
            actor.action = 124;
            return;
        }
        let mut best = 1;
        let mut heading = actor.yaw;
        for modulus in [7_000, 7_000, 10, 7_000] {
            let mut candidate = actor.position;
            crate::move_entity(&mut candidate, heading as i16, 0, 256);
            actor.entity_rng = level2_next_rng16(actor.entity_rng);
            let score = if roam_probe_blocked(actor, context.terrain, candidate) {
                0
            } else {
                actor.entity_rng % modulus + 2
            };
            if score > best {
                best = score;
                actor.yaw = heading;
            }
            heading = heading.wrapping_add(512) & 2047;
        }
    }
    let mut predicted = actor.position;
    if actor.source_byte_3e & 15 == 0 {
        match (actor.yaw.wrapping_sub(256) >> 9) & 3 {
            0 | 2 => predicted.y = (predicted.y & 0xff00).wrapping_add(128),
            _ => predicted.x = (predicted.x & 0xff00).wrapping_add(128),
        }
    }
    if let Some(peer) = context
        .model15
        .iter()
        .filter_map(|slot| context.target(*slot))
        .find(|peer| {
            peer.source_id != actor.id_entity_index
                && actor.position.x.abs_diff(peer.position.x) < 256
                && actor.position.y.abs_diff(peer.position.y) < 256
        })
    {
        actor.roll = heading_for_positions(peer.position, actor.position) as u16;
    }
    let move_now = actor.roll == actor.yaw || {
        actor.entity_rng = level2_next_rng16(actor.entity_rng);
        actor.entity_rng % 20 <= 10
    };
    if move_now {
        crate::move_entity(&mut predicted, actor.yaw as i16, 0, actor.speed);
        actor.position = predicted;
    }
    settle(actor, context.terrain);
}

fn aim(actor: &mut CastleDefenderState) {
    actor.entity_rng = level2_next_rng16(actor.entity_rng);
    actor.speed = 0;
    actor.visual_index = if actor.entity_rng % 20 <= 10 { 206 } else { 1 };
    actor.fov = if actor.visual_index == 206 { 120 } else { 125 };
    actor.collision_vertical_offset = actor.fov;
    actor.shift_rot_x = 0;
    actor.shift_rot_y = 0;
}

fn idle(actor: &mut CastleDefenderState) {
    actor.speed = actor.minimum_speed;
    actor.visual_index = 0;
    actor.fov = 125;
    actor.collision_vertical_offset = 125;
    actor.shift_rot_x = 0;
    actor.shift_rot_y = 0;
}

fn nearest(
    actor: &CastleDefenderState,
    context: &DefenderContext<'_>,
    slots: &[u16],
    reverse: bool,
    eligible: impl Fn(DefenderTarget) -> bool,
) -> Option<DefenderTarget> {
    let mut best = None;
    let mut best_distance = u32::MAX;
    for target in slots
        .iter()
        .filter_map(|slot| context.target(*slot))
        .filter(|target| eligible(*target))
    {
        let candidate = distance_xy(actor.position, target.position);
        let bearing = if reverse {
            heading_for_positions(target.position, actor.position)
        } else {
            heading_for_positions(actor.position, target.position)
        };
        if candidate <= TARGET_RANGE * TARGET_RANGE
            && candidate < best_distance
            && angle_distance(i32::from(actor.yaw), bearing) < TARGET_HALF_ANGLE
        {
            best = Some(target);
            best_distance = candidate;
        }
    }
    best
}

/// 1DBF0 promotes the X difference but stores the Y difference in an i16.
/// This differs from 23C40, which truncates both coordinate differences.
fn guard_target(
    actor: &CastleDefenderState,
    context: &DefenderContext<'_>,
) -> Option<DefenderTarget> {
    let mut best = None;
    let mut best_distance = u32::MAX;
    for target in context
        .class3
        .iter()
        .filter_map(|slot| context.target(*slot))
    {
        let dx = i32::from(target.position.x) - i32::from(actor.position.x);
        let dy = i32::from(target.position.y.wrapping_sub(actor.position.y).cast_signed());
        let d = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32;
        if target.control_flags & 0x20 == 0
            && d <= TARGET_RANGE * TARGET_RANGE
            && d < best_distance
            && angle_distance(
                i32::from(actor.yaw),
                heading_for_positions(actor.position, target.position),
            ) < TARGET_HALF_ANGLE
        {
            best = Some(target);
            best_distance = d;
        }
    }
    best
}

/// Returns None for action126: descriptor83 has flee bit8 clear and str50
/// has no callback there. External state must not turn it into a fake no-op.
pub(crate) fn step(
    actor: &mut CastleDefenderState,
    context: &mut DefenderContext<'_>,
) -> Option<DefenderEffects> {
    let mut effects = DefenderEffects::default();
    match actor.action {
        120 => stationary(actor, context),
        121 => {
            match consume_mailbox(actor, context) {
                0 => {
                    roam(actor, context);
                    if actor.source_byte_3e % CADENCE == 0
                        && actor.source_byte_39 != 0
                        && let Some(target) =
                            nearest(actor, context, context.class3, false, |target| {
                                target.source_id != actor.id_entity_index
                                    && target.control_flags & 0x20 == 0
                            })
                    {
                        actor.target_sphere_slot = target.slot;
                        actor.action = 122;
                    }
                }
                1 => {
                    if context
                        .target(actor.damage_attacker_entity_index)
                        .is_some_and(|target| {
                            target.class == 3 && target.source_id != actor.id_entity_index
                        })
                    {
                        actor.action = 122;
                        actor.target_sphere_slot = actor.damage_attacker_entity_index;
                    }
                    settle(actor, context.terrain);
                }
                _ => actor.action = 124,
            }
            if actor.action == 122 {
                aim(actor);
            }
        }
        122 => {
            if consume_mailbox(actor, context) == 2 {
                actor.action = 124;
            } else if let Some(target) = context.target(actor.target_sphere_slot) {
                if actor.source_byte_3e & 3 == 0 {
                    actor.roll = heading_for_positions(actor.position, target.position) as u16;
                }
                if target.life < 0 || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0 {
                    actor.action = 121;
                } else if actor.source_byte_3e % CADENCE == 0 {
                    if distance(actor.position, target.position) >= TARGET_RANGE {
                        actor.action = 121;
                    } else {
                        effects.arrow = Some(DefenderArrowRequest {
                            owner: actor.id_entity_index,
                            position: actor.position,
                            fov: actor.fov,
                            target,
                            word96: actor.target_sphere_slot,
                            xtype: actor.xtype,
                            xsubtype: actor.xsubtype,
                        });
                    }
                }
                if actor.action == 122 {
                    settle(actor, context.terrain);
                }
            } else {
                actor.action = 121;
            }
            if actor.action != 122 {
                idle(actor);
            }
        }
        123 => linked(actor, context, &mut effects),
        124 => {
            let mut slot = actor.chain_next_slot;
            for _ in 0..=context.targets.len() {
                let Some(mut linked) = context.target(slot) else {
                    break;
                };
                slot = linked.next;
                linked.action = 125;
                inherit_killer(&mut actor.killer_entity_index, linked.killer);
                effects.linked_updates.push(linked);
            }
            // PreKill deliberately excludes model15 from creature-kill credit.
            actor.action = 125;
        }
        125 => effects.kill = actor.source_byte_3e & 7 == 0,
        127 => stage_step(actor, context, &mut effects),
        _ => return None,
    }
    Some(effects)
}

fn stationary(actor: &mut CastleDefenderState, context: &DefenderContext<'_>) {
    match consume_mailbox(actor, context) {
        2 => actor.action = 124,
        1 => {
            actor.target_sphere_slot = actor.damage_attacker_entity_index;
            actor.action = 122;
        }
        _ if actor.source_byte_3e % CADENCE == 0 => {
            if let Some(peer) = nearest(actor, context, context.model15, true, |peer| {
                peer.slot != actor.source_entity_index && peer.previous == 0
            }) {
                actor.chain_previous_slot = peer.slot;
                actor.action = 123;
            }
        }
        _ => {}
    }
    settle(actor, context.terrain);
}

fn linked(
    actor: &mut CastleDefenderState,
    context: &DefenderContext<'_>,
    effects: &mut DefenderEffects,
) {
    if actor.chain_previous_slot == 0 {
        actor.action = 121;
        return;
    }
    let leader = context.target(actor.chain_previous_slot).filter(|leader| {
        leader.life >= 0
            && leader.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            && leader.class == SOURCE_CLASS
            && leader.model == SOURCE_MODEL
    });
    let damage = consume_mailbox(actor, context);
    if damage != 0 {
        if let Some(mut leader) = leader {
            leader.target = actor.damage_attacker_entity_index;
            leader.previous = 0;
            leader.action = 122;
            effects.linked_updates.push(leader);
        }
        actor.chain_previous_slot = 0;
        if damage == 2 {
            actor.action = 124;
        } else {
            actor.target_sphere_slot = actor.damage_attacker_entity_index;
            actor.action = 122;
            settle(actor, context.terrain);
        }
        return;
    }
    effects.population_decrements += u32::from(generic_move(actor, context));
    let Some(leader) = leader else {
        actor.chain_previous_slot = 0;
        actor.action = 121;
        return;
    };
    if actor.source_byte_3e % CADENCE != 0 {
        return;
    }
    match leader.action.wrapping_sub(120) {
        0 | 1 => {}
        3 => actor.chain_previous_slot = leader.previous,
        2 | 6 => {
            actor.target_sphere_slot = leader.target;
            actor.chain_previous_slot = 0;
            actor.action = leader.action;
            return;
        }
        _ => {
            actor.chain_previous_slot = 0;
            actor.action = 121;
            return;
        }
    }
    if let Some(head) = context.target(actor.chain_previous_slot) {
        actor.roll = heading_for_positions(actor.position, head.position) as u16;
    }
    if let Some(peer) = context
        .model15
        .iter()
        .filter_map(|slot| context.target(*slot))
        .find(|peer| {
            peer.source_id != actor.id_entity_index
                && i32::from(actor.position.x.cast_signed())
                    .abs_diff(i32::from(peer.position.x.cast_signed()))
                    < 256
                && i32::from(actor.position.y.cast_signed())
                    .abs_diff(i32::from(peer.position.y.cast_signed()))
                    < 256
        })
    {
        actor.roll = heading_for_positions(peer.position, actor.position) as u16;
    }
    actor.speed = leader.maximum_speed.wrapping_add(leader.speed);
}

/// Descriptor83's generic 1B8C0 path, used by link/StageVar helpers rather
/// than the grid-specific action121 path.
fn generic_move(actor: &mut CastleDefenderState, context: &DefenderContext<'_>) -> bool {
    if actor.control_flags & 0x800 != 0 {
        actor.control_flags &= !0x800;
        return false;
    }
    let original = actor.position;
    let yaw = actor.yaw;
    let third = (yaw.wrapping_sub(85) & 255) | (yaw.wrapping_sub(341) & 0x700);
    let fourth = (yaw.wrapping_add(1024) & 0x700) | (yaw & 255);
    for (attempt, heading) in [yaw & 2047, yaw.wrapping_add(341) & 2047, third, fourth]
        .into_iter()
        .enumerate()
    {
        actor.yaw = heading;
        actor.position = original;
        settle(actor, context.terrain);
        let mut predicted = actor.position;
        actor.position = original;
        crate::move_entity(&mut predicted, heading as i16, 0, actor.speed);
        let crosses = (predicted.x >> 8, predicted.y >> 8) != (original.x >> 8, original.y >> 8);
        let mut blocked = false;
        if crosses || attempt != 0 {
            let mut probe = predicted;
            let mut distance = 0;
            while distance <= i32::from(actor.shift_rot_x.max(actor.shift_rot_y)) {
                blocked = probe_blocked(context.terrain, probe, actor.fov);
                if blocked {
                    break;
                }
                crate::move_entity(&mut probe, heading as i16, 0, 256);
                distance += 256;
            }
            let x = predicted.x.to_be_bytes()[0];
            let y = predicted.y.to_be_bytes()[0];
            let height = |x, y| {
                i32::from(context.terrain.ground_height(Position {
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
            blocked |= (a + d - b - c).abs().max((a + b - d - c).abs()) >= PROPERTIES.maximum_slope;
        }
        if !blocked {
            actor.position = predicted;
            actor.yaw = (heading as i16).wrapping_add(crate::turn_toward(
                heading as i16,
                actor.roll as i16,
                PROPERTIES.turn_limit,
            )) as u16
                & 2047;
            if attempt == 0 {
                actor.control_flags &= !0x40000;
            }
            return false;
        }
        actor.control_flags |= 0x40000;
    }
    actor.position = original;
    actor.life = -1;
    true
}

pub(crate) fn bind_stage(
    actor: &mut CastleDefenderState,
    index: u8,
    runtime: &mut dyn StageRuntime,
) {
    if index != 0
        && let Some(descriptor) = runtime.bind(index)
    {
        actor.stage.index = index;
        actor.stage.mode = descriptor.kind;
        actor.stage.relation = if descriptor.kind == 6 {
            descriptor.scalar
        } else {
            0
        };
        actor.action = 127;
    } else {
        actor.stage.index = 0;
        actor.stage.mode = 0;
        actor.stage.relation = 0;
        actor.action = 121;
    }
}

fn wrapped_near(a: Position, b: Position, radius: u16) -> bool {
    a.x.wrapping_sub(b.x).cast_signed().unsigned_abs() <= radius
        && a.y.wrapping_sub(b.y).cast_signed().unsigned_abs() <= radius
}

fn signed_near(a: Position, b: Position, radius: u32) -> bool {
    i32::from(a.x.cast_signed()).abs_diff(i32::from(b.x.cast_signed())) <= radius
        && i32::from(a.y.cast_signed()).abs_diff(i32::from(b.y.cast_signed())) <= radius
}

/// Frozen living-root StageVar prepass, before any action callback.
pub(crate) fn stage_prepass(actor: &mut CastleDefenderState, context: &mut DefenderContext<'_>) {
    let action = actor.action & 7;
    if matches!(action, 4 | 5) {
        return;
    }
    if matches!(actor.stage.mode, 13 | 14 | 16 | 17) {
        if !matches!(action, 2 | 6) {
            actor.action = 127;
        }
        return;
    }
    if actor.stage.mode == 10 || actor.stage.mode == 15 {
        if (actor.stage.mode == 10 && !matches!(action, 2 | 6))
            || (actor.stage.mode == 15 && action != 0)
        {
            if let Some(runtime) = context.stage_runtime.as_mut() {
                bind_stage(actor, actor.stage.index, *runtime);
            }
        }
        return;
    }
    let Some(descriptor) = context
        .stage_runtime
        .as_ref()
        .and_then(|runtime| runtime.descriptor(actor.stage.index))
    else {
        return;
    };
    let ended = match descriptor.kind {
        1 => wrapped_near(actor.position, descriptor.point, 2048),
        3 | 4 | 5 | 8 | 9 => {
            if descriptor.flags & 4 != 0 {
                if descriptor.kind == 3 {
                    actor.stage.relation = 0;
                }
                !matches!(action, 2 | 6) || descriptor.kind != 3
            } else {
                if descriptor.flags & 2 != 0
                    && matches!(descriptor.kind, 3..=5)
                    && context.target(actor.stage.relation).is_some_and(|row| {
                        row.life < 0 || row.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
                    })
                {
                    actor.stage.relation = 0;
                }
                descriptor.kind == 9
                    && descriptor.flags & 2 == 0
                    && (descriptor.point.x != 0 || descriptor.point.y != 0)
                    && wrapped_near(actor.position, descriptor.point, 3072)
            }
        }
        6 => {
            actor.stage.relation = actor.stage.relation.wrapping_sub(1);
            actor.stage.relation == 0
        }
        7 => descriptor.flags & 0x18 != 0,
        _ => false,
    };
    if ended && let Some(runtime) = context.stage_runtime.as_mut() {
        if descriptor.kind == 7 {
            runtime.rearm_model_relations();
        }
        bind_stage(actor, descriptor.next, *runtime);
    }
}

pub(crate) fn advance_proximity(actor: &mut CastleDefenderState, player: Position) -> Option<u8> {
    if actor.life < 0 {
        actor.source_byte_39 = 250;
        actor.proximity_delay = 0;
        return None;
    }
    if actor.source_byte_39 != 0 {
        let prior = actor.source_byte_39;
        actor.source_byte_39 = prior.wrapping_sub(1);
        return Some(prior);
    }
    if actor.control_flags & 1 != 0 {
        return None;
    }
    if actor.proximity_delay != 0 {
        actor.proximity_delay -= 1;
        return None;
    }
    if distance_xy(actor.position, player) < 0x0240_0000 {
        actor.source_byte_39 = 16;
        Some(18)
    } else {
        None
    }
}

fn jitter(actor: &mut CastleDefenderState) {
    let first = level2_next_rng16(actor.entity_rng);
    actor.entity_rng = level2_next_rng16(first);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    actor.roll = (i32::from(actor.roll) + (i32::from(actor.entity_rng & 255) + 85) * direction)
        as u16
        & 2047;
}

fn follow(
    actor: &mut CastleDefenderState,
    context: &DefenderContext<'_>,
    target: Position,
    point: bool,
    width: Option<i16>,
) {
    if actor.source_byte_3e & 7 != 0 {
        return;
    }
    if actor.control_flags & 0x40000 == 0 {
        actor.roll = heading_for_positions(actor.position, target) as u16;
        if actor.source_byte_3e & 63 == 0 {
            jitter(actor);
        }
    }
    if let Some(peer) = context
        .model15
        .iter()
        .filter_map(|slot| context.target(*slot))
        .find(|peer| {
            peer.source_id != actor.id_entity_index
                && if point {
                    signed_near(
                        actor.position,
                        peer.position,
                        actor.shift_rot_x.saturating_sub(1) as u32,
                    )
                } else {
                    actor.position.x.abs_diff(peer.position.x) < actor.shift_rot_x as u16
                        && actor.position.y.abs_diff(peer.position.y) < actor.shift_rot_x as u16
                }
        })
    {
        actor.roll = heading_for_positions(peer.position, actor.position) as u16;
    }
    if let Some(width) = width {
        let separation = i32::from(actor.shift_rot_x) + i32::from(width);
        if i32::from(actor.position.x.abs_diff(target.x)) < separation
            && i32::from(actor.position.y.abs_diff(target.y)) < separation
        {
            actor.roll = heading_for_positions(target, actor.position) as u16;
        }
    }
}

fn stage_target(
    actor: &mut CastleDefenderState,
    context: &DefenderContext<'_>,
    descriptor: StageDescriptor,
) -> Option<DefenderTarget> {
    let slot = if descriptor.flags & 2 == 0 {
        descriptor.actor
    } else {
        if actor.stage.relation == 0 {
            if descriptor.flags & 1 != 0 {
                actor.stage.relation = context
                    .model15
                    .iter()
                    .filter_map(|slot| context.target(*slot))
                    .find(|peer| peer.stage.index == actor.stage.index && peer.stage.relation != 0)
                    .map_or(0, |peer| peer.stage.relation);
            }
            if actor.stage.relation == 0 {
                let mut best = u32::MAX;
                for target in context
                    .class5
                    .iter()
                    .filter_map(|slot| context.target(*slot))
                    .filter(|target| target.model == descriptor.model)
                {
                    let d = distance_xy(actor.position, target.position);
                    if d < best {
                        best = d;
                        actor.stage.relation = target.slot;
                    }
                }
            }
        }
        actor.stage.relation
    };
    context.target(slot)
}

fn blink(
    actor: &mut CastleDefenderState,
    context: &DefenderContext<'_>,
    effects: &mut DefenderEffects,
) {
    let stage = actor.stage;
    let phase = if stage.remaining <= stage.duration.wrapping_sub(16) && stage.remaining >= 16 {
        if actor.source_byte_3e & 0x1c == 16 {
            actor.source_byte_3e & 3
        } else {
            0
        }
    } else if actor.source_byte_3e & 1 != 0 {
        0
    } else {
        2
    };
    let bits = match phase {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    actor.control_flags = (actor.control_flags & !0x0600_0000) | bits;
    let mut slot = actor.chain_next_slot;
    for _ in 0..=context.targets.len() {
        let Some(mut linked) = context.target(slot) else {
            break;
        };
        slot = linked.next;
        linked.control_flags = (linked.control_flags & !0x0600_0000) | bits;
        effects.linked_updates.push(linked);
    }
}

fn summon_follow(
    actor: &mut CastleDefenderState,
    context: &DefenderContext<'_>,
    effects: &mut DefenderEffects,
) {
    let hit = consume_mailbox(actor, context);
    if hit == 2 {
        actor.stage.remaining = 1;
        return;
    }
    effects.population_decrements += u32::from(generic_move(actor, context));
    if hit == 1 {
        if actor.damage_attacker_entity_index != actor.stage.parent
            && context
                .target(actor.damage_attacker_entity_index)
                .is_none_or(|row| row.class != SOURCE_CLASS || row.model != SOURCE_MODEL)
        {
            actor.target_sphere_slot = actor.damage_attacker_entity_index;
            actor.action = 122;
            effects.feedback = Some((actor.stage.parent, 19));
        }
    } else if let Some(target) = context.target(actor.target_sphere_slot) {
        follow(actor, context, target.position, false, None);
    }
}

fn summoned_stage(
    actor: &mut CastleDefenderState,
    context: &DefenderContext<'_>,
    effects: &mut DefenderEffects,
) {
    let parent = context.target(actor.stage.parent);
    if actor.stage.mode == 14 {
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
        if actor.stage.remaining <= 0 || parent.is_none_or(|row| row.life <= 0) {
            actor.stage.mode = 10;
            actor.target_sphere_slot = 0;
            actor.stage.parent = 0;
            actor.stage.index = actor.stage.relation as u8;
            actor.control_flags &= !0x0600_0000;
            blink(actor, context, effects);
            // Clearing applies regardless of the old duration/phase.
            actor.control_flags &= !0x0600_0000;
            for linked in &mut effects.linked_updates {
                linked.control_flags &= !0x0600_0000;
            }
            return;
        }
        blink(actor, context, effects);
        let parent = parent.unwrap();
        let previous = actor.target_sphere_slot;
        actor.target_sphere_slot = actor.stage.parent;
        summon_follow(actor, context, effects);
        let inherited = if parent.target != 0 {
            parent.target
        } else {
            parent.attacker
        };
        actor.target_sphere_slot = if inherited != 0 { inherited } else { previous };
        if let Some(target) = context.target(actor.target_sphere_slot) {
            if (target.parent == actor.stage.parent && target.stage.mode == 14)
                || target.life <= 0
                || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
            {
                actor.target_sphere_slot = 0;
            } else if distance(actor.position, target.position) < TARGET_RANGE {
                actor.action = 122;
                effects.feedback = Some((actor.stage.parent, 24));
            }
        }
        return;
    }
    if parent.is_none_or(|row| row.life < 0 || row.control_flags & ENTITY_DRAW_DISABLED_BIT != 0) {
        actor.stage.remaining = 0;
    } else if actor.stage.mode == 13 {
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(1);
    }
    if actor.stage.remaining <= 0 {
        effects.expire = true;
        effects.summon_expire = actor.stage.mode == 13;
        // The adapter applies allocation(s), then hides the current occupant.
        return;
    }
    let parent = parent.unwrap();
    let mut target = context.target(actor.target_sphere_slot);
    if target.is_some_and(|row| row.life <= 0 || row.control_flags & ENTITY_DRAW_DISABLED_BIT != 0)
    {
        actor.target_sphere_slot = 0;
        target = None;
    }
    if target.is_none() && actor.source_byte_3e & 7 == 0 {
        let mut best = u32::MAX;
        for row in context
            .class3
            .iter()
            .filter_map(|slot| context.target(*slot))
            .filter(|row| matches!(row.model, 0 | 1) && row.source_id != parent.source_id)
        {
            let d = distance_xy(parent.position, row.position);
            if d < best {
                best = d;
                target = Some(row);
            }
        }
        actor.target_sphere_slot = target.map_or(0, |row| row.slot);
    }
    if let Some(target) = target {
        summon_follow(actor, context, effects);
        if actor.source_byte_3e & 7 == 0 && distance(actor.position, target.position) < TARGET_RANGE
        {
            actor.action = 122;
        }
    } else {
        actor.target_sphere_slot = actor.stage.parent;
        summon_follow(actor, context, effects);
        actor.target_sphere_slot = 0;
        actor.stage.remaining = actor.stage.remaining.wrapping_sub(4);
    }
}

fn stage_step(
    actor: &mut CastleDefenderState,
    context: &mut DefenderContext<'_>,
    effects: &mut DefenderEffects,
) {
    let mode = actor.stage.mode;
    match mode {
        10 => actor.action = 122,
        12 => {
            if let Some(parent) = context.target(actor.stage.parent) {
                if parent.life < 0 {
                    actor.hide();
                }
                actor.position = parent.position;
                actor.position.z = actor.position.z.max(0);
                actor.yaw = parent.yaw;
                actor.roll = parent.roll;
            } else {
                actor.hide();
                actor.position = Position::default();
                actor.yaw = 0;
                actor.roll = 0;
            }
        }
        17 => {
            effects.population_decrements += u32::from(generic_move(actor, context));
            if actor.life < 0 {
                actor.hide();
            } else if let Some(target) = context.target(actor.target_sphere_slot).filter(|target| {
                target.life >= 0 && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            }) {
                actor.roll = heading_for_positions(actor.position, target.position) as u16;
                actor.speed = actor.speed.wrapping_sub(8);
                if actor.speed <= 16 {
                    actor.stage.mode = 16;
                }
            } else {
                actor.stage.mode = 16;
            }
        }
        13 | 14 | 16 => summoned_stage(actor, context, effects),
        1..=9 => {
            let descriptor = context
                .stage_runtime
                .as_ref()
                .and_then(|runtime| runtime.descriptor(actor.stage.index));
            match consume_mailbox(actor, context) {
                2 => actor.action = 124,
                1 => {
                    if context
                        .target(actor.damage_attacker_entity_index)
                        .is_none_or(|target| {
                            target.class != SOURCE_CLASS || target.model != SOURCE_MODEL
                        })
                    {
                        actor.target_sphere_slot = actor.damage_attacker_entity_index;
                        actor.stage.mode = 10;
                        actor.action = 122;
                    }
                    settle(actor, context.terrain);
                }
                _ => {
                    effects.population_decrements += u32::from(generic_move(actor, context));
                    if let Some(descriptor) = descriptor {
                        if mode == 1
                            || (mode == 2 && !signed_near(actor.position, descriptor.point, 3072))
                        {
                            follow(actor, context, descriptor.point, true, None);
                        } else if matches!(mode, 3..=5) && actor.source_byte_3e & 7 == 0 {
                            if let Some(target) = stage_target(actor, context, descriptor) {
                                follow(
                                    actor,
                                    context,
                                    target.position,
                                    false,
                                    Some(target.half_width),
                                );
                            } else {
                                idle_jitter(actor);
                            }
                        } else {
                            idle_jitter(actor);
                        }
                    }
                }
            }
            if matches!(mode, 3 | 4)
                && actor.source_byte_3e & 7 == 0
                && let Some(descriptor) = descriptor
            {
                let slot = if descriptor.flags & 2 != 0 {
                    actor.stage.relation
                } else {
                    descriptor.actor
                };
                if let Some(target) = context.target(slot) {
                    let selected = if mode == 4 {
                        context.target(target.target)
                    } else {
                        Some(target)
                    };
                    if let Some(selected) = selected.filter(|row| row.slot != 0xae02)
                        && distance(actor.position, selected.position) <= TARGET_RANGE
                    {
                        actor.target_sphere_slot = selected.slot;
                        actor.stage.mode = 10;
                        actor.action = 122;
                    }
                }
            }
            if mode == 2
                && actor.action == 127
                && actor.stage.mode == 2
                && actor.source_byte_39 != 0
                && actor.source_byte_3e % CADENCE == 0
                && let Some(target) = guard_target(actor, context)
            {
                actor.target_sphere_slot = target.slot;
                actor.stage.mode = 10;
                actor.action = 122;
            }
        }
        _ => {}
    }
    // sub_24400 only delegates: unlike Archer39 it has no dword reset or
    // transition visual tail. The source helper's own writes are final.
}

fn idle_jitter(actor: &mut CastleDefenderState) {
    if actor.source_byte_3e & 15 == 0 && actor.control_flags & 0x40000 == 0 {
        actor.entity_rng = level2_next_rng16(actor.entity_rng);
        actor.roll = actor.roll.wrapping_add(actor.entity_rng % 113 + 142) & 2047;
    }
}

/// NewEvent may replace the currently executing source slot. These writes
/// then observe the newborn Arrow's zeroed fields through the aliased pointer.
pub(crate) fn finish_self_replaced_arrow(
    arrow: &mut super::archer::Level2AuthoredClass5ArcherChild,
    target: Position,
    terrain: &SessionTerrain,
) {
    arrow.entity_id = arrow.source_entity_index;
    arrow.parent_entity_id = arrow.source_entity_index;
    arrow.word_0x96 = 0;
    arrow.xtype = u8::MAX;
    arrow.xsubtype = u8::MAX;
    arrow.position = Position {
        z: 30,
        ..Position::default()
    };
    arrow.yaw = heading_for_positions(Position::default(), target) as i16;
    let horizontal = crate::legacy_sqrt(distance_xy(Position::default(), target));
    arrow.pitch = crate::pitch_for_delta(horizontal as i32, i32::from(target.z));
    let ground = terrain.ground_height(arrow.position);
    if arrow.position.z > ground {
        arrow.position.z = arrow.position.z.wrapping_sub(4);
    }
    if arrow.position.z <= ground {
        arrow.position.z = ground;
    }
    arrow.actual_speed = arrow.minimum_speed;
    arrow.particle_index = 0;
    arrow.shift_rot_x = 0;
    arrow.shift_rot_y = 0;
    arrow.collision_half_width = 0;
    arrow.collision_vertical_offset = 125;
    arrow.collision_half_height = 125;
    arrow.fov = 125;
}

pub(crate) fn finish_self_replaced_burst(burst: &mut crate::native_world::Level2DeathBurst) {
    burst.id_entity_index = burst.source_entity_index;
    burst.position = Position::default();
    burst.control_flags |= ENTITY_DRAW_DISABLED_BIT;
    burst.active = false;
}
