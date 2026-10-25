//! Bee construction, action16..23, StageVar routes and combat lifetime.
//!
//! The arena adapter supplies frozen memberships and current actor projections.
//! This owner mutates its one authoritative Bee row and returns explicit writes
//! for other domains. It never borrows the session or economy.

pub(crate) mod mc2;

use crate::combat::actors::{DamageHit, consume_damage, inherit_linked_life};
use crate::level_definition::BEE_CLASS_PROPERTIES;
use crate::native_world::MapType;
use crate::native_world::{
    SessionTerrain, angle_distance, heading_for_positions, level2_next_rng16,
    level2_wrapping_source_distance_3d,
};
use crate::{FlightWorld, Position};

const LEVEL2_BEE_AWARENESS_RADIUS: i64 = BEE_CLASS_PROPERTIES.target_range as i64;
const LEVEL2_BEE_AWARENESS_HALF_ANGLE: i32 = BEE_CLASS_PROPERTIES.target_half_angle as i32;
const LEVEL2_BEE_GROUND_CLEARANCE: i16 = BEE_CLASS_PROPERTIES.ground_clearance;
const LEVEL2_BEE_GROUND_RELAXATION: i16 = BEE_CLASS_PROPERTIES.descent;
const LEVEL2_BEE_COLLISION_HALF_EXTENT: i16 = BEE_CLASS_PROPERTIES.collision_half_width;
const LEVEL2_BEE_TERRAIN_THRESHOLD: i32 = BEE_CLASS_PROPERTIES.maximum_slope;
const LEVEL2_BEE_TURN_LIMIT: i16 = BEE_CLASS_PROPERTIES.turn_limit;
const LEVEL2_BEE_MOVEMENT_SKIP_BIT: u32 = 0x800;
const LEVEL2_BEE_OBSTRUCTION_BIT: u32 = 0x40000;
const DRAW_DISABLED: u32 = 0x400;

/// Current arena fields, in the source's frozen linked-list membership order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BeeActor {
    pub slot: u16,
    pub source_id: u16,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub position: Position,
    pub life: i32,
    pub control_flags: u32,
    pub target: u16,
    pub attacker: u16,
    pub killer: u16,
    pub peer: u16,
    pub child: u16,
    pub parent: u16,
    pub stage_index: u8,
    pub stage_mode: u8,
    pub stage_relation: u16,
    pub yaw: i16,
    pub roll: i16,
    pub actual_speed: i16,
    pub maximum_speed: i16,
    pub half_width: i16,
    pub height_offset: i16,
    pub frozen_class3_member: bool,
    pub frozen_class5_member: bool,
    pub frozen_family_member: bool,
}

impl BeeActor {
    fn live(self) -> bool {
        self.slot != 0 && self.life >= 0 && self.control_flags & DRAW_DISABLED == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BeeStageDescriptor {
    pub kind: u8,
    pub flags: u8,
    pub next: u8,
    pub point: Position,
    pub actor: u16,
    pub model: u8,
    pub scalar: u16,
}

/// Borrows the real mutable StageVar table; bind owns its thinning counter.
pub(crate) trait BeeStageRuntime {
    fn descriptor(&self, index: u8) -> Option<BeeStageDescriptor>;
    fn bind(&mut self, index: u8) -> Option<BeeStageDescriptor>;
    fn rearm_model_relations(&mut self);
}

pub(crate) struct BeeActionContext<'a> {
    pub terrain: &'a SessionTerrain,
    pub actors: &'a [BeeActor],
    pub stage: Option<BeeStageDescriptor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BeeSound {
    Acquire(u16),
    Contact(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BeeContact {
    pub target: u16,
    pub attacker: u16,
    pub damage: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BeePeerUpdate {
    pub slot: u16,
    pub action: u8,
    pub target: u16,
    pub peer: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct BeeEffects {
    pub callback_executed: bool,
    pub sound: Option<BeeSound>,
    pub damage: Option<DamageHit>,
    pub contact: Option<BeeContact>,
    pub peer_update: Option<BeePeerUpdate>,
    pub death_conversion: bool,
    pub expire_effect: bool,
    pub summon_expire_effect: bool,
    pub kill_credit: Option<u16>,
    pub population_decrement: bool,
    pub experience_award: Option<(u16, u8)>,
    pub kill_linked_children: bool,
    pub linked_blink_flags: Option<u32>,
}

/// `sub_4B590`, after the shared arena has allocated a source slot/RNG word.
pub(crate) fn construct(
    slot: u16,
    initial_rng: u16,
    constructor_phase: u8,
    origin: BeeCreatureOrigin,
    position: Position,
    max_life: i32,
    map_type: MapType,
) -> Option<BeeCreatureRuntime> {
    if map_type != MapType::Day || max_life <= 0 || slot == 0 {
        return None;
    }
    let properties = BEE_CLASS_PROPERTIES;
    let entity_rng = level2_next_rng16(initial_rng);
    let angle = (entity_rng & 2047) as i16 - 1;
    Some(BeeCreatureRuntime {
        source_entity_index: slot,
        entity_id: slot,
        entity_rng,
        origin,
        parent_entity_index: 0,
        source_class: 5,
        model: 2,
        action: 17,
        minimum_speed: properties.minimum_speed,
        maximum_speed: properties.maximum_speed,
        actual_speed: properties.initial_speed,
        control_flags: 12,
        // SetEvent144's source default is one half of authored maximum life.
        // Keep that ratio in the typed class's default mana/life values.
        mana: (i64::from(max_life) * i64::from(properties.initial_mana)
            / i64::from(properties.max_life)) as i32,
        max_life,
        life: max_life,
        pending_damage: 0,
        pending_attacker_entity_index: 0,
        damage_attacker_entity_index: 0,
        kill_credit_entity_index: 0,
        subspell_index: properties.attack_damage,
        dword_0x10: i32::from(slot % 100),
        target_entity_index: None,
        yaw: angle,
        pitch: angle,
        roll: angle,
        source_byte_3e: constructor_phase,
        source_byte_39: i16::from(properties.scan_period)
            - i16::from(constructor_phase % properties.scan_period)
            + 4,
        vertical_step: properties.descent,
        action_window: i16::from(properties.scan_period),
        stage_var1: 0,
        stage_var2: 0,
        stage_target: None,
        last_sound_id: 0,
        linked_peer_entity_index: 0,
        linked_child_entity_index: 0,
        stage_relation: 0,
        stage_remaining: 0,
        stage_duration: 0,
        proximity_delay: 0,
        position,
        particle_index: 3,
        shift_rot_x: properties.collision_half_width as u16,
        shift_rot_y: properties.collision_half_height as u16,
        collision_vertical_offset: properties.collision_vertical_offset,
        phase: 0,
        drawing_enabled: true,
        active: true,
    })
}

/// The production path which created a live Bee.
///
/// Package records and Morph spells share the same source arena, constructor,
/// scheduler, and callbacks. Keeping their origins typed prevents a
/// spell-created Bee from masquerading as an authored package row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeeCreatureOrigin {
    InitialPackage,
    AuthoredRecord { authored_index: u16 },
    MorphSpell { holder_slot: u16 },
}

/// Live semantic Bee creature constructed by remc2 `sub_4B590` (class 5,
/// model 2). The state keeps the words consumed by the ordinary Bee actions
/// and Morph's selector-12 parent follower.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeeCreatureRuntime {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub origin: BeeCreatureOrigin,
    pub parent_entity_index: u16,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub actual_speed: i16,
    /// NewEvent's source control dword. Action-17 movement consumes bit
    /// 0x800 as the one-tick movement skip and stores the `sub_1B8C0`
    /// obstruction marker in bit 0x40000.
    pub control_flags: u32,
    /// `SetEvent144_49C70` initializes the Bee's death-conversion mana from
    /// half of its max life (the package-default constructor therefore starts
    /// at 1500, while an authored maximum derives the matching half value).
    pub mana: i32,
    pub max_life: i32,
    pub life: i32,
    /// Source `sub_1BF90` damage mailbox. A Fireball writes these fields;
    /// action 17 consumes and clears them at the next Bee callback.
    pub pending_damage: i32,
    pub pending_attacker_entity_index: u16,
    /// Source word-0x26 after mailbox consumption.
    pub damage_attacker_entity_index: u16,
    /// Source word-0x24 copied from word-0x26 by `PreKillEntity_1C890`.
    /// This is the identity carried into the qualifying creature census.
    pub kill_credit_entity_index: u16,
    pub subspell_index: u16,
    pub dword_0x10: i32,
    pub target_entity_index: Option<u16>,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    /// Source `byte_0x3E_62`, the per-model constructor counter.
    pub source_byte_3e: u8,
    /// Source `byte_0x39_57`, derived from descriptor row 73's cycle window.
    pub source_byte_39: i16,
    /// Descriptor row 73 `word_0xE_14` used by action 18 vertical tracking.
    pub vertical_step: i16,
    /// Descriptor row 73 `word_0x1A_26` used by the action-18 recoil latch.
    pub action_window: i16,
    /// Source `StageVar1_0x48_72`, the one-based runtime StageVar index set
    /// by `sub_12330` after a successful constructor allocation.
    pub stage_var1: u8,
    /// Source `StageVar2_0x49_73`, used by scripted action 23's movement
    /// callback.
    pub stage_var2: u8,
    /// Runtime kind-2 point copied from the selected StageVar. The authored
    /// selector remains in `MissionContentSession`; this is the mutable event
    /// callback's point carrier.
    pub stage_target: Option<Position>,
    /// Most recent `PrepareEventSound_6E450` sound selected by the Bee helper.
    /// Zero means that this source has not emitted one yet.
    pub last_sound_id: u16,
    /// Source word32/34 and StageVar words4A/2E/30, owned by this row.
    pub linked_peer_entity_index: u16,
    pub linked_child_entity_index: u16,
    pub stage_relation: u16,
    pub stage_remaining: i16,
    pub stage_duration: i16,
    pub proximity_delay: u8,
    pub position: Position,
    pub particle_index: u16,
    pub shift_rot_x: u16,
    pub shift_rot_y: u16,
    /// Source array.yaw, retained when SetEntityShiftRot changes other axes.
    pub collision_vertical_offset: i16,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

impl BeeCreatureRuntime {
    pub(crate) fn projection(&self, frozen_member: bool) -> BeeActor {
        BeeActor {
            slot: self.source_entity_index,
            source_id: self.entity_id,
            class: self.source_class,
            model: self.model,
            action: self.action,
            position: self.position,
            life: self.life,
            control_flags: self.control_flags
                | if self.drawing_enabled {
                    0
                } else {
                    DRAW_DISABLED
                },
            target: self.target_entity_index.unwrap_or(0),
            attacker: self.damage_attacker_entity_index,
            killer: self.kill_credit_entity_index,
            peer: self.linked_peer_entity_index,
            child: self.linked_child_entity_index,
            parent: self.parent_entity_index,
            stage_index: self.stage_var1,
            stage_mode: self.stage_var2,
            stage_relation: self.stage_relation,
            yaw: self.yaw,
            roll: self.roll,
            actual_speed: self.actual_speed,
            maximum_speed: self.maximum_speed,
            half_width: self.shift_rot_x as i16,
            height_offset: self.collision_vertical_offset,
            frozen_class3_member: false,
            frozen_class5_member: frozen_member,
            frozen_family_member: frozen_member,
        }
    }

    pub(crate) fn hash_owned_relations(&self, hash: &mut sha2::Sha256) {
        use sha2::Digest;
        hash.update(self.linked_peer_entity_index.to_le_bytes());
        hash.update(self.linked_child_entity_index.to_le_bytes());
        hash.update(self.stage_relation.to_le_bytes());
        hash.update(self.stage_remaining.to_le_bytes());
        hash.update(self.stage_duration.to_le_bytes());
        hash.update([self.proximity_delay]);
        hash.update(self.collision_vertical_offset.to_le_bytes());
    }

    #[must_use]
    pub const fn authored_index(&self) -> Option<u16> {
        match self.origin {
            BeeCreatureOrigin::AuthoredRecord { authored_index } => Some(authored_index),
            BeeCreatureOrigin::InitialPackage | BeeCreatureOrigin::MorphSpell { .. } => None,
        }
    }
}

/// Live parent projection consumed by Morph's selector-12 Bee follower.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeeFollowerParent {
    pub entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub roll: i16,
    pub life: i32,
    pub active: bool,
}

pub(crate) fn level2_bee_source_tile(position: Position) -> (u8, u8) {
    (position.x.to_be_bytes()[0], position.y.to_be_bytes()[0])
}

pub(crate) fn level2_bee_slope_measure(terrain: &SessionTerrain, position: Position) -> i32 {
    let (tile_x, tile_y) = level2_bee_source_tile(position);
    let height = |x: u8, y: u8| {
        i32::from(terrain.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let point_1 = height(tile_x, tile_y);
    let point_2 = height(tile_x.wrapping_add(1), tile_y);
    let point_3 = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let point_4 = height(tile_x, tile_y.wrapping_add(1));
    (point_1 + point_4 - point_2 - point_3)
        .abs()
        .max((point_1 + point_2 - point_4 - point_3).abs())
}

fn level2_bee_probe_blocked(terrain: &SessionTerrain, candidate: Position, heading: i16) -> bool {
    // Descriptor 73's dword_0x14 is 0xffff_ffff, so the source's
    // `~dword_0x14 & material_mask` test rejects no Day material. Its
    // collision probe still has a 128-unit footprint and a Cave-only angle /
    // ceiling branch; the authored Bee constructor is Day-only, but retaining
    // that branch keeps this helper source-shaped if a preserved Cave row is
    // ever supplied.
    let mut probe = candidate;
    let mut distance = 0_i32;
    while distance <= i32::from(LEVEL2_BEE_COLLISION_HALF_EXTENT) {
        if terrain.map_type == MapType::Cave {
            let tile = crate::Tile {
                x: probe.x.to_be_bytes()[0],
                y: probe.y.to_be_bytes()[0],
            };
            if terrain.angle_flags(tile) & 8 != 0 {
                return true;
            }
            if let Some(ceiling) = terrain.ceiling_height(probe) {
                let ground = terrain.ground_height(probe);
                if i32::from(128) + i32::from(ground) + i32::from(LEVEL2_BEE_GROUND_CLEARANCE)
                    > i32::from(ceiling)
                {
                    return true;
                }
            }
        }
        distance += 256;
        crate::move_entity(&mut probe, heading, 0, 256);
    }
    false
}

pub(crate) fn level2_bee_retry_headings(yaw: i16) -> [i16; 4] {
    // This is the source's byte-preserving retry sequence from sub_1B8C0;
    // the third and fourth headings are not ordinary +/-341 turns.
    let yaw = i32::from(yaw);
    let first = yaw & 0x07ff;
    let second = (yaw + 341) & 0x07ff;
    let third = ((yaw.wrapping_sub(85) as u16 & 0x00ff)
        | ((((yaw.wrapping_sub(341) as u16) >> 8) & 7) << 8))
        & 0x07ff;
    let fourth = ((((yaw + 0x400) as u16) & 0x0700) | (yaw as u8 as u16)) & 0x07ff;
    [
        i16::try_from(first).expect("Bee source retry heading fits"),
        i16::try_from(second).expect("Bee source retry heading fits"),
        i16::try_from(third).expect("Bee source retry heading fits"),
        i16::try_from(fourth).expect("Bee source retry heading fits"),
    ]
}

pub(crate) fn level2_bee_turn_delta(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    if delta == 0 {
        return 0;
    }
    let magnitude = delta.abs().min(i32::from(LEVEL2_BEE_TURN_LIMIT));
    i16::try_from(magnitude * delta.signum()).expect("Bee source turn delta fits")
}

pub(crate) fn move_level2_bee_source(source: &mut BeeCreatureRuntime, terrain: &SessionTerrain) {
    if source.control_flags & LEVEL2_BEE_MOVEMENT_SKIP_BIT != 0 {
        source.control_flags &= !LEVEL2_BEE_MOVEMENT_SKIP_BIT;
        return;
    }

    let original = source.position;
    let original_yaw = source.yaw;
    for (attempt, heading) in level2_bee_retry_headings(original_yaw)
        .into_iter()
        .enumerate()
    {
        let mut candidate = original;
        let ground = terrain.ground_height(candidate);
        if candidate.z > ground {
            candidate.z = candidate.z.wrapping_add(LEVEL2_BEE_GROUND_RELAXATION);
        }
        if candidate.z <= ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE) {
            candidate.z = ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE);
        }
        crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
        let crossed_tile = level2_bee_source_tile(candidate) != level2_bee_source_tile(original);
        let blocked = (attempt != 0 || crossed_tile)
            && (level2_bee_probe_blocked(terrain, candidate, heading)
                || level2_bee_slope_measure(terrain, candidate) >= LEVEL2_BEE_TERRAIN_THRESHOLD);
        if blocked {
            source.control_flags |= LEVEL2_BEE_OBSTRUCTION_BIT;
            // The source commits each failed heading before the next retry;
            // a later successful retry therefore retains the marker.
            source.yaw = heading;
            continue;
        }

        source.position = candidate;
        source.yaw = heading.wrapping_add(level2_bee_turn_delta(heading, source.roll)) & 0x07ff;
        if attempt == 0 {
            source.control_flags &= !LEVEL2_BEE_OBSTRUCTION_BIT;
        }
        return;
    }

    // Descriptor 73 has byte_0x20 bit 0 set, so the source kills a Bee when
    // every retry is blocked. Population-census decrementation is owned by
    // the broader class-5 census projection and remains outside this typed
    // economy seam.
    if BEE_CLASS_PROPERTIES.behavior_flags & 1 != 0 || terrain.material_mask(source.position) == 1 {
        source.life = -1;
    }
}

fn actor(context: &BeeActionContext<'_>, slot: u16) -> Option<BeeActor> {
    (slot != 0)
        .then(|| context.actors.iter().copied().find(|row| row.slot == slot))
        .flatten()
}

fn cadence(source: &BeeCreatureRuntime) -> bool {
    source.action_window > 0 && i16::from(source.source_byte_3e) % source.action_window == 0
}

fn settle(source: &mut BeeCreatureRuntime, terrain: &SessionTerrain) {
    let ground = terrain.ground_height(source.position);
    if source.position.z > ground {
        source.position.z = source.position.z.wrapping_add(source.vertical_step);
    }
    if i32::from(source.position.z) <= i32::from(ground) + i32::from(LEVEL2_BEE_GROUND_CLEARANCE) {
        source.position.z = ground.wrapping_add(LEVEL2_BEE_GROUND_CLEARANCE);
    }
}

fn move_source(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    let before = source.life;
    move_level2_bee_source(source, context.terrain);
    effects.population_decrement |= source.life == -1 && before != -1;
}

/// Returns0/no-hit,1/hit,2/dead in source order. Life remains a signed dword.
fn consume(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) -> u8 {
    effects.damage = consume_damage(
        &mut source.life,
        &mut source.pending_damage,
        &mut source.pending_attacker_entity_index,
        &mut source.damage_attacker_entity_index,
    );
    let mut hit = effects.damage.is_some();
    let mut next = source.linked_child_entity_index;
    for _ in 0..context.actors.len() {
        let Some(child) = actor(context, next) else {
            break;
        };
        if inherit_linked_life(
            &mut source.life,
            &mut source.damage_attacker_entity_index,
            child.life,
            child.attacker,
        ) {
            hit = true;
            break;
        }
        next = child.child;
    }
    if source.life < 0 {
        source.kill_credit_entity_index = source.damage_attacker_entity_index;
        2
    } else {
        u8::from(hit)
    }
}

fn acquire_sound(source: &mut BeeCreatureRuntime, effects: &mut BeeEffects) {
    source.entity_rng = level2_next_rng16(source.entity_rng);
    source.last_sound_id = (source.entity_rng & 1) + 12;
    source.dword_0x10 = 1;
    effects.sound = Some(BeeSound::Acquire(source.last_sound_id));
}

/// Legacy direct mailbox entry. Production uses the complete callback below;
/// this retains the established action17-only caller contract.
pub(crate) fn consume_action17_mailbox(source: &mut BeeCreatureRuntime) -> Option<DamageHit> {
    if source.pending_attacker_entity_index == 0 {
        return None;
    }
    let hit = consume_damage(
        &mut source.life,
        &mut source.pending_damage,
        &mut source.pending_attacker_entity_index,
        &mut source.damage_attacker_entity_index,
    )?;
    if hit.lethal {
        source.kill_credit_entity_index = hit.attacker;
        source.action = 20;
    } else {
        source.target_entity_index = Some(hit.attacker).filter(|slot| *slot != 0);
        source.action = 18;
        acquire_sound(source, &mut BeeEffects::default());
    }
    Some(hit)
}

pub(crate) fn death_conversion_requested(source: &mut BeeCreatureRuntime) -> bool {
    if source.source_byte_3e & 7 != 0 {
        return false;
    }
    // TransformEntityToManaSphere advances the dying row before NewEvent,
    // even when no sphere slot is available. Empty mana skips that draw.
    if source.mana > 0 {
        source.entity_rng = level2_next_rng16(source.entity_rng);
    }
    source.drawing_enabled = false;
    true
}

fn jitter(source: &mut BeeCreatureRuntime) {
    let first = level2_next_rng16(source.entity_rng);
    source.entity_rng = level2_next_rng16(first);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    source.roll = (i32::from(source.roll) + (i32::from(source.entity_rng & 255) + 85) * direction)
        as i16
        & 2047;
}

fn square_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

fn nearest(
    source: &BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    eligible: impl Fn(BeeActor) -> bool,
    reverse_cone: bool,
) -> Option<BeeActor> {
    let radius = BEE_CLASS_PROPERTIES.target_range;
    let mut selected = None;
    let mut best = u32::MAX;
    for row in context.actors.iter().copied().filter(|row| eligible(*row)) {
        let distance = square_distance(source.position, row.position);
        if distance > radius * radius {
            continue;
        }
        let heading = if reverse_cone {
            heading_for_positions(row.position, source.position)
        } else {
            heading_for_positions(source.position, row.position)
        };
        if i32::from(angle_distance(i32::from(source.yaw), heading))
            >= LEVEL2_BEE_AWARENESS_HALF_ANGLE
        {
            continue;
        }
        if distance < best {
            selected = Some(row);
            best = distance;
        }
    }
    selected
}

fn select_peer(source: &mut BeeCreatureRuntime, context: &BeeActionContext<'_>) {
    if BEE_CLASS_PROPERTIES.behavior_flags & 4 == 0 {
        if let Some(peer) = nearest(
            source,
            context,
            |row| {
                row.frozen_family_member && row.slot != source.source_entity_index && row.peer == 0
            },
            true,
        ) {
            source.linked_peer_entity_index = peer.slot;
            source.action = 19;
        }
    }
}

fn avoid_peers(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    bound: i32,
    signed: bool,
) {
    for peer in context
        .actors
        .iter()
        .filter(|row| row.frozen_family_member && row.source_id != source.entity_id)
    {
        let (dx, dy) = if signed {
            (
                (i32::from(source.position.x as i16) - i32::from(peer.position.x as i16)).abs(),
                (i32::from(source.position.y as i16) - i32::from(peer.position.y as i16)).abs(),
            )
        } else {
            (
                i32::from(source.position.x.abs_diff(peer.position.x)),
                i32::from(source.position.y.abs_diff(peer.position.y)),
            )
        };
        if dx < bound && dy < bound {
            source.roll = heading_for_positions(peer.position, source.position) as i16;
            break;
        }
    }
}

fn action16(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    match consume(source, context, effects) {
        2 => source.action = 20,
        1 => {
            source.target_entity_index =
                Some(source.damage_attacker_entity_index).filter(|slot| *slot != 0);
            source.action = 18;
        }
        _ => {
            if cadence(source) {
                select_peer(source, context);
            }
        }
    }
    settle(source, context.terrain);
    if source.action == 18 {
        source.dword_0x10 = 1;
    }
}

fn action17(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    match consume(source, context, effects) {
        2 => source.action = 20,
        1 => {
            source.target_entity_index =
                Some(source.damage_attacker_entity_index).filter(|slot| *slot != 0);
            source.action = 18;
            settle(source, context.terrain);
        }
        _ => {
            move_source(source, context, effects);
            if cadence(source) {
                jitter(source);
                if source.source_byte_39 != 0 {
                    if let Some(target) = nearest(
                        source,
                        context,
                        |row| row.frozen_class3_member && row.control_flags & 0x20 == 0,
                        false,
                    ) {
                        source.target_entity_index = Some(target.slot);
                        source.action = 18;
                    } else {
                        select_peer(source, context);
                    }
                }
            }
        }
    }
    if source.action == 18 {
        acquire_sound(source, effects);
    }
}

fn blink(source: &mut BeeCreatureRuntime, effects: &mut BeeEffects) {
    let phase = if i32::from(source.stage_remaining) <= i32::from(source.stage_duration) - 16
        && source.stage_remaining >= 16
    {
        if source.source_byte_3e & 0x1c == 16 {
            source.source_byte_3e & 3
        } else {
            0
        }
    } else if source.source_byte_3e & 1 == 0 {
        2
    } else {
        0
    };
    source.control_flags &= !0x0600_0000;
    source.control_flags |= match phase {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
    effects.linked_blink_flags = Some(source.control_flags & 0x0600_0000);
}

fn alliance_target(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    target: Option<BeeActor>,
    effects: &mut BeeEffects,
) -> Option<BeeActor> {
    if source.stage_var2 != 14 {
        return target;
    }
    blink(source, effects);
    source.stage_remaining = source.stage_remaining.wrapping_sub(1);
    if source.stage_remaining <= 0 {
        return None;
    }
    let Some(parent) = actor(context, source.parent_entity_index) else {
        return target;
    };
    let target = target?;
    let selected = if parent.target != 0 {
        parent.target
    } else {
        parent.attacker
    };
    (target.slot != parent.slot && (selected == 0 || selected == target.slot)).then_some(target)
}

fn action18(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    if source.dword_0x10 != 0 {
        let before = source.dword_0x10;
        source.dword_0x10 = before.wrapping_sub(1);
        if before == 1 {
            source.actual_speed = ((5 * i32::from(source.minimum_speed)) >> 1) as i16;
        }
    }
    if let Some(slot) = source.target_entity_index.filter(|slot| *slot != 0) {
        // Source reads the target slot's coordinates before mailbox/validity.
        if let Some(target) = actor(context, slot) {
            let desired = i32::from(target.position.z) + i32::from(target.height_offset);
            let delta = (i32::from(source.position.z) - desired) as i16;
            let direction = if i32::from(source.position.z) == desired {
                0
            } else if delta <= 0 {
                -1_i16
            } else {
                1_i16
            };
            source.position.z = source
                .position
                .z
                .wrapping_add(direction.wrapping_mul(source.vertical_step));
        }
        match consume(source, context, effects) {
            2 => source.action = 20,
            1 => {
                source.target_entity_index =
                    Some(source.damage_attacker_entity_index).filter(|slot| *slot != 0);
                settle(source, context.terrain);
            }
            _ => {
                move_source(source, context, effects);
                let selected = actor(context, slot);
                if let Some(target) =
                    alliance_target(source, context, selected, effects).filter(|row| row.live())
                {
                    if source.source_byte_3e & 3 == 0 {
                        source.roll =
                            heading_for_positions(source.position, target.position) as i16;
                        avoid_peers(source, context, i32::from(source.shift_rot_x), false);
                    }
                    if cadence(source) {
                        let distance = crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                            source.position,
                            target.position,
                        ));
                        if distance >= BEE_CLASS_PROPERTIES.target_range {
                            source.action = 17;
                        } else if distance < 1024 {
                            effects.contact = Some(BeeContact {
                                target: target.slot,
                                attacker: source.entity_id,
                                damage: i32::from(source.subspell_index),
                            });
                            source.entity_rng = level2_next_rng16(source.entity_rng);
                            source.last_sound_id = (source.entity_rng & 1) + 12;
                            effects.sound = Some(BeeSound::Contact(source.last_sound_id));
                            source.actual_speed = source.maximum_speed.wrapping_neg();
                            source.dword_0x10 = 3 * i32::from(source.action_window);
                        }
                    }
                } else {
                    source.action = 17;
                }
            }
        }
    } else {
        source.action = 17;
    }
    if source.action != 18 {
        source.actual_speed = source.minimum_speed;
    }
}

fn action19(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    if source.linked_peer_entity_index == 0 {
        source.action = 17;
        return;
    }
    let peer = actor(context, source.linked_peer_entity_index)
        .filter(|row| row.live() && row.class == source.source_class && row.model == source.model);
    let hit = consume(source, context, effects);
    if hit != 0 {
        if let Some(peer) = peer {
            effects.peer_update = Some(BeePeerUpdate {
                slot: peer.slot,
                action: 18,
                target: source.damage_attacker_entity_index,
                peer: 0,
            });
        }
        source.linked_peer_entity_index = 0;
        if hit == 2 {
            source.action = 20;
        } else {
            source.target_entity_index =
                Some(source.damage_attacker_entity_index).filter(|slot| *slot != 0);
            source.action = 18;
            settle(source, context.terrain);
        }
    } else {
        move_source(source, context, effects);
        if let Some(peer) = peer {
            if cadence(source) {
                match peer.action.wrapping_sub(16) {
                    0 | 1 | 3 => {
                        if peer.action == 19 {
                            source.linked_peer_entity_index = peer.peer;
                        }
                        if let Some(followed) = actor(context, source.linked_peer_entity_index) {
                            source.roll =
                                heading_for_positions(source.position, followed.position) as i16;
                        }
                        avoid_peers(source, context, 256, true);
                        source.actual_speed = peer.maximum_speed.wrapping_add(peer.actual_speed);
                    }
                    2 => {
                        source.target_entity_index = Some(peer.target).filter(|slot| *slot != 0);
                        source.linked_peer_entity_index = 0;
                        source.action = 18;
                    }
                    // Bee descriptor73 never selects the unsupported +6 callback.
                    _ => {
                        source.linked_peer_entity_index = 0;
                        source.action = 17;
                    }
                }
            }
        } else {
            source.linked_peer_entity_index = 0;
            source.action = 17;
        }
    }
    if source.action == 18 {
        source.dword_0x10 = 1;
    }
}

/// One enabled source callback plus its byte3E epilogue. Unsupported action22
/// has no callback and therefore does not advance; it is never selected by73.
pub(crate) fn step(source: &mut BeeCreatureRuntime, context: &BeeActionContext<'_>) -> BeeEffects {
    let mut effects = BeeEffects::default();
    if !source.active || !matches!(source.action, 16..=21 | 23) {
        return effects;
    }
    effects.callback_executed = true;
    match source.action {
        16 => action16(source, context, &mut effects),
        17 => action17(source, context, &mut effects),
        18 => action18(source, context, &mut effects),
        19 => action19(source, context, &mut effects),
        20 => {
            let mut next = source.linked_child_entity_index;
            for _ in 0..context.actors.len() {
                let Some(child) = actor(context, next) else {
                    break;
                };
                if child.killer != 0 {
                    source.kill_credit_entity_index = child.killer;
                }
                next = child.child;
            }
            effects.kill_linked_children = source.linked_child_entity_index != 0;
            effects.kill_credit = Some(source.kill_credit_entity_index).filter(|slot| *slot != 0);
            source.action = 21;
        }
        21 => {
            effects.death_conversion = death_conversion_requested(source);
        }
        23 => {
            stage_step(source, context, &mut effects);
            if source.source_byte_3e & 7 == 0 && (1..=9).contains(&source.stage_var2) {
                let first = level2_next_rng16(source.entity_rng);
                source.entity_rng = level2_next_rng16(first);
                let direction = 2 * (i32::from(first % 157) / 79) - 1;
                source.roll =
                    (i32::from(source.roll) + i32::from(source.entity_rng % 85) * direction) as i16
                        & 2047;
            }
            if source.action == 18 {
                source.dword_0x10 = 1;
            }
        }
        _ => unreachable!(),
    }
    source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
    source.phase = source.phase.wrapping_add(1);
    effects
}

fn stage_actor(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    descriptor: BeeStageDescriptor,
) -> Option<BeeActor> {
    let slot = if descriptor.flags & 2 == 0 {
        descriptor.actor
    } else {
        if source.stage_relation == 0 && descriptor.flags & 1 != 0 {
            source.stage_relation = context
                .actors
                .iter()
                .find(|row| {
                    row.frozen_family_member
                        && row.stage_index == source.stage_var1
                        && row.stage_relation != 0
                })
                .map_or(0, |row| row.stage_relation);
        }
        if source.stage_relation == 0 {
            let mut best = u32::MAX;
            for row in context.actors.iter().filter(|row| {
                row.frozen_class5_member && row.class == 5 && row.model == descriptor.model
            }) {
                let distance = square_distance(source.position, row.position);
                if distance < best {
                    source.stage_relation = row.slot;
                    best = distance;
                }
            }
        }
        source.stage_relation
    };
    actor(context, slot)
}

fn follow(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    position: Position,
    point: bool,
    width: Option<i16>,
) {
    if source.source_byte_3e & 7 != 0 {
        return;
    }
    if source.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT == 0 {
        source.roll = heading_for_positions(source.position, position) as i16;
        if source.source_byte_3e & 63 == 0 {
            jitter(source);
        }
    }
    avoid_peers(source, context, i32::from(source.shift_rot_x), point);
    if let Some(width) = width {
        let bound = i32::from(source.shift_rot_x) + i32::from(width);
        if i32::from(source.position.x.abs_diff(position.x)) < bound
            && i32::from(source.position.y.abs_diff(position.y)) < bound
        {
            source.roll = heading_for_positions(position, source.position) as i16;
        }
    }
}

fn idle_turn(source: &mut BeeCreatureRuntime) {
    if source.source_byte_3e & 15 == 0 && source.control_flags & LEVEL2_BEE_OBSTRUCTION_BIT == 0 {
        source.entity_rng = level2_next_rng16(source.entity_rng);
        source.roll = source
            .roll
            .wrapping_add((source.entity_rng % 113) as i16 + 142)
            & 2047;
    }
}

fn summoned_follow(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    let hit = consume(source, context, effects);
    if hit == 2 {
        source.stage_remaining = 1;
        return;
    }
    move_source(source, context, effects);
    if hit == 1 {
        let attacker = source.damage_attacker_entity_index;
        if attacker != source.parent_entity_index
            && actor(context, attacker).is_none_or(|row| row.class != 5 || row.model != 2)
        {
            source.target_entity_index = Some(attacker).filter(|slot| *slot != 0);
            source.action = 18;
            effects.experience_award = Some((source.parent_entity_index, 19));
        }
    } else if let Some(target) = source
        .target_entity_index
        .and_then(|slot| actor(context, slot))
    {
        follow(source, context, target.position, false, None);
    }
}

fn summoned_stage(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    let mode = source.stage_var2;
    let parent = actor(context, source.parent_entity_index);
    if mode == 14 {
        source.stage_remaining = source.stage_remaining.wrapping_sub(1);
        if source.stage_remaining <= 0 || parent.is_none_or(|row| row.life <= 0) {
            source.stage_var2 = 10;
            source.target_entity_index = None;
            source.parent_entity_index = 0;
            source.stage_var1 = source.stage_relation as u8;
            source.control_flags &= !0x0600_0000;
            effects.linked_blink_flags = Some(0);
            return;
        }
        blink(source, effects);
        let parent = parent.unwrap();
        let previous = source.target_entity_index;
        source.target_entity_index = Some(source.parent_entity_index);
        summoned_follow(source, context, effects);
        let selected = if parent.target != 0 {
            parent.target
        } else {
            parent.attacker
        };
        source.target_entity_index = if selected != 0 {
            Some(selected)
        } else {
            previous
        };
        if let Some(target) = source
            .target_entity_index
            .and_then(|slot| actor(context, slot))
        {
            if (target.parent == source.parent_entity_index && target.stage_mode == 14)
                || target.life <= 0
                || target.control_flags & DRAW_DISABLED != 0
            {
                source.target_entity_index = None;
            } else if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                source.position,
                target.position,
            )) < BEE_CLASS_PROPERTIES.target_range
            {
                source.action = 18;
                effects.experience_award = Some((source.parent_entity_index, 24));
            }
        }
        return;
    }
    if parent.is_none_or(|row| !row.live()) {
        source.stage_remaining = 0;
    } else if mode == 13 {
        source.stage_remaining = source.stage_remaining.wrapping_sub(1);
    }
    if source.stage_remaining <= 0 {
        effects.expire_effect = true;
        effects.summon_expire_effect = mode == 13;
        source.drawing_enabled = false;
        return;
    }
    let parent = parent.unwrap();
    let mut target = source
        .target_entity_index
        .and_then(|slot| actor(context, slot));
    if target.is_some_and(|row| row.life <= 0 || row.control_flags & DRAW_DISABLED != 0) {
        source.target_entity_index = None;
        target = None;
    }
    if target.is_none() && source.source_byte_3e & 7 == 0 {
        let mut best = u32::MAX;
        for row in context.actors.iter().copied().filter(|row| {
            row.frozen_class3_member
                && matches!(row.model, 0 | 1)
                && row.source_id != parent.source_id
        }) {
            let distance = square_distance(parent.position, row.position);
            if distance < best {
                best = distance;
                target = Some(row);
            }
        }
        source.target_entity_index = target.map(|row| row.slot);
    }
    if let Some(target) = target {
        summoned_follow(source, context, effects);
        if source.source_byte_3e & 7 == 0
            && crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                source.position,
                target.position,
            )) < BEE_CLASS_PROPERTIES.target_range
        {
            source.action = 18;
        }
    } else {
        source.target_entity_index = Some(source.parent_entity_index);
        summoned_follow(source, context, effects);
        source.target_entity_index = None;
        source.stage_remaining = source.stage_remaining.wrapping_sub(4);
    }
}

fn stage_step(
    source: &mut BeeCreatureRuntime,
    context: &BeeActionContext<'_>,
    effects: &mut BeeEffects,
) {
    let mode = source.stage_var2;
    match mode {
        1..=9 => {
            let descriptor = context.stage;
            let hit = consume(source, context, effects);
            if hit == 2 {
                source.action = 20;
            } else if hit == 1 {
                if actor(context, source.damage_attacker_entity_index)
                    .is_none_or(|row| row.class != 5 || row.model != 2)
                {
                    source.target_entity_index =
                        Some(source.damage_attacker_entity_index).filter(|slot| *slot != 0);
                    source.stage_var2 = 10;
                    source.action = 18;
                }
                settle(source, context.terrain);
            } else {
                let directed = descriptor.is_some_and(|row| {
                    mode == 1
                        || (mode == 2
                            && ((i32::from(source.position.x as i16)
                                - i32::from(row.point.x as i16))
                            .abs()
                                > 3072
                                || (i32::from(source.position.y as i16)
                                    - i32::from(row.point.y as i16))
                                .abs()
                                    > 3072))
                });
                move_source(source, context, effects);
                if let Some(descriptor) = descriptor {
                    if directed {
                        follow(source, context, descriptor.point, true, None);
                    } else if matches!(mode, 3..=5) && source.source_byte_3e & 7 == 0 {
                        if let Some(target) = stage_actor(source, context, descriptor) {
                            follow(
                                source,
                                context,
                                target.position,
                                false,
                                Some(target.half_width),
                            );
                        } else {
                            idle_turn(source);
                        }
                    } else if BEE_CLASS_PROPERTIES.behavior_flags & 2 == 0 {
                        idle_turn(source);
                    }
                }
            }
            // These wrappers run even after their base helper's hit/dead branch.
            if matches!(mode, 3 | 4) && source.source_byte_3e & 7 == 0 {
                if let Some(descriptor) = descriptor {
                    let slot = if descriptor.flags & 2 != 0 {
                        source.stage_relation
                    } else {
                        descriptor.actor
                    };
                    if let Some(relation) = actor(context, slot) {
                        let target = if mode == 4 {
                            actor(context, relation.target)
                        } else {
                            Some(relation)
                        };
                        if let Some(target) = target.filter(|row| row.slot != 0xae02)
                            && crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                                source.position,
                                target.position,
                            )) <= BEE_CLASS_PROPERTIES.target_range
                        {
                            source.target_entity_index = Some(target.slot);
                            source.stage_var2 = 10;
                            source.action = 18;
                        }
                    }
                }
            }
            if mode == 2
                && source.action == 23
                && source.stage_var2 == 2
                && source.source_byte_39 != 0
                && cadence(source)
            {
                // 1DBF0 uses an unsigned-promoted X delta and signed16 Y delta.
                let mut best = u32::MAX;
                let mut selected = None;
                for target in context
                    .actors
                    .iter()
                    .filter(|row| row.frozen_class3_member && row.control_flags & 0x20 == 0)
                {
                    let dx = i32::from(target.position.x) - i32::from(source.position.x);
                    let dy = i32::from(
                        target
                            .position
                            .y
                            .wrapping_sub(source.position.y)
                            .cast_signed(),
                    );
                    let distance = dx
                        .wrapping_mul(dx)
                        .wrapping_add(dy.wrapping_mul(dy))
                        .cast_unsigned();
                    let heading = heading_for_positions(source.position, target.position);
                    if distance
                        <= BEE_CLASS_PROPERTIES.target_range * BEE_CLASS_PROPERTIES.target_range
                        && i32::from(angle_distance(i32::from(source.yaw), heading))
                            < LEVEL2_BEE_AWARENESS_HALF_ANGLE
                        && distance < best
                    {
                        best = distance;
                        selected = Some(target.slot);
                    }
                }
                if let Some(slot) = selected {
                    source.target_entity_index = Some(slot);
                    source.stage_var2 = 10;
                    source.action = 18;
                }
            }
        }
        10 => source.action = 18,
        12 => {
            if let Some(parent) = actor(context, source.parent_entity_index) {
                if parent.life < 0 {
                    source.drawing_enabled = false;
                }
                source.position = parent.position;
                source.position.z = source.position.z.max(0);
                source.roll = parent.roll;
                source.yaw = parent.yaw;
            } else {
                source.drawing_enabled = false;
                source.position = Position { x: 0, y: 0, z: 0 };
                source.roll = 0;
                source.yaw = 0;
            }
        }
        13 | 14 | 16 => summoned_stage(source, context, effects),
        17 => {
            move_source(source, context, effects);
            if source.life < 0 {
                source.drawing_enabled = false;
                return;
            }
            if let Some(target) = source
                .target_entity_index
                .and_then(|slot| actor(context, slot))
                .filter(|row| row.live())
            {
                source.roll = heading_for_positions(source.position, target.position) as i16;
                source.actual_speed = source.actual_speed.wrapping_sub(8);
                if source.actual_speed <= 16 {
                    source.stage_var2 = 16;
                }
            } else {
                source.stage_var2 = 16;
            }
        }
        // sub1D5D0 has no callback for0/11/15/other selectors.
        _ => {}
    }
}

pub(crate) fn bind_stage(
    source: &mut BeeCreatureRuntime,
    index: u8,
    runtime: &mut dyn BeeStageRuntime,
) {
    if index != 0
        && let Some(descriptor) = runtime.bind(index)
    {
        source.stage_var1 = index;
        source.stage_var2 = descriptor.kind;
        source.stage_relation = if descriptor.kind == 6 {
            descriptor.scalar
        } else {
            0
        };
        source.stage_target =
            (descriptor.kind == 1 || descriptor.kind == 2).then_some(descriptor.point);
        source.action = 23;
    } else {
        source.stage_var1 = 0;
        source.stage_var2 = 0;
        source.stage_relation = 0;
        source.stage_target = None;
        source.action = 17;
    }
}

/// sub12500 on the frozen living-root list, before callbacks/byte39 activation.
pub(crate) fn stage_prepass(
    source: &mut BeeCreatureRuntime,
    runtime: &mut dyn BeeStageRuntime,
    actors: &[BeeActor],
) {
    let action = source.action & 7;
    if matches!(action, 4 | 5) {
        return;
    }
    if matches!(source.stage_var2, 13 | 14 | 16 | 17) {
        if !matches!(action, 2 | 6) {
            source.action = 23;
        }
        return;
    }
    if (source.stage_var2 == 10 && !matches!(action, 2 | 6))
        || (source.stage_var2 == 15 && action != 0)
    {
        bind_stage(source, source.stage_var1, runtime);
        return;
    }
    let Some(descriptor) = runtime.descriptor(source.stage_var1) else {
        return;
    };
    let near = |distance: u16| {
        source
            .position
            .x
            .wrapping_sub(descriptor.point.x)
            .cast_signed()
            .unsigned_abs()
            <= distance
            && source
                .position
                .y
                .wrapping_sub(descriptor.point.y)
                .cast_signed()
                .unsigned_abs()
                <= distance
    };
    let ended = match descriptor.kind {
        1 => near(2048),
        3..=5 | 8 | 9 => {
            if descriptor.flags & 4 != 0 {
                if descriptor.kind == 3 {
                    source.stage_relation = 0;
                }
                descriptor.kind != 3 || !matches!(action, 2 | 6)
            } else {
                if descriptor.flags & 2 != 0
                    && matches!(descriptor.kind, 3..=5)
                    && actors
                        .iter()
                        .find(|row| row.slot == source.stage_relation)
                        .is_some_and(|row| !row.live())
                {
                    source.stage_relation = 0;
                }
                descriptor.kind == 9
                    && descriptor.flags & 2 == 0
                    && (descriptor.point.x != 0 || descriptor.point.y != 0)
                    && near(3072)
            }
        }
        6 => {
            source.stage_relation = source.stage_relation.wrapping_sub(1);
            source.stage_relation == 0
        }
        7 => descriptor.flags & 0x18 != 0,
        _ => false,
    };
    if ended {
        if descriptor.kind == 7 {
            runtime.rearm_model_relations();
        }
        bind_stage(source, descriptor.next, runtime);
    }
}

/// SP sub68C70 activation; this is separate from both StageVar and callbacks.
pub(crate) fn proximity_prepass(source: &mut BeeCreatureRuntime, player: Position) -> Option<u8> {
    if source.life < 0 {
        source.source_byte_39 = 250;
        source.proximity_delay = 0;
        return None;
    }
    if source.source_byte_39 != 0 {
        let before = source.source_byte_39;
        source.source_byte_39 = source.source_byte_39.wrapping_sub(1);
        return Some(before as u8);
    }
    if source.control_flags & 1 != 0 {
        return None;
    }
    if source.proximity_delay != 0 {
        source.proximity_delay -= 1;
        return None;
    }
    source.proximity_delay = 0;
    if square_distance(source.position, player) < 0x0240_0000 {
        source.source_byte_39 = 16;
        return Some(18);
    }
    None
}

#[cfg(test)]
mod tests;
