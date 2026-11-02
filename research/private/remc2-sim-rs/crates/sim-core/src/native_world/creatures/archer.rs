//! Archer state and source action kernels, borrowed through a narrow context.
//!
//! The MC2 adapter supplies frozen target memberships, terrain and allocation.
//! This owner changes only the Archer row, its own state and explicit effects;
//! it never borrows a session, scheduler, or economy.

use crate::combat::actors::{consume_damage, inherit_linked_life};
use crate::level_definition::ARCHER_CLASS_PROPERTIES;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, Level2AuthoredClass5Source, SessionTerrain, angle_distance,
    heading_for_positions, level2_next_rng16, level2_wrapping_source_distance_3d,
};
use crate::{FlightWorld, Position};

pub(crate) const ARCHER_SOURCE_CLASS: u8 = 5;
pub(crate) const ARCHER_MODEL: u8 = 4;
pub(crate) const ARCHER_ACTION_STATIONARY: u8 = 32;
pub(crate) const ARCHER_ACTION: u8 = 33;
pub(crate) const ARCHER_ACTION_ATTACK: u8 = 34;
pub(crate) const ARCHER_ACTION_LINK: u8 = 35;
pub(crate) const ARCHER_ACTION_HIT: u8 = 36;
pub(crate) const ARCHER_ACTION_KILL: u8 = 37;
pub(crate) const ARCHER_ACTION_ATTACK_VISUAL: u8 = 38;
pub(crate) const ARCHER_ACTION_IDLE_VISUAL: u8 = 39;

pub(crate) const ARCHER_MINIMUM_SPEED: i16 = ARCHER_CLASS_PROPERTIES.minimum_speed;
pub(crate) const ARCHER_MAXIMUM_SPEED: i16 = ARCHER_CLASS_PROPERTIES.maximum_speed;
pub(crate) const ARCHER_MAX_LIFE: i32 = 1_000;
pub(crate) const ARCHER_MANA: i32 = 500;
pub(crate) const ARCHER_MAX_MANA: i32 = 0;
pub(crate) const ARCHER_SUBSPELL_INDEX: u16 = ARCHER_CLASS_PROPERTIES.attack_damage;
pub(crate) const ARCHER_DESCRIPTOR_INDEX: u16 = 75;
pub(crate) const ARCHER_DESCRIPTOR_CADENCE: u8 = ARCHER_CLASS_PROPERTIES.scan_period;
pub(crate) const ARCHER_DESCRIPTOR_RANGE: u32 = ARCHER_CLASS_PROPERTIES.target_range;
pub(crate) const ARCHER_DESCRIPTOR_HALF_ANGLE: i16 = ARCHER_CLASS_PROPERTIES.target_half_angle;
pub(crate) const ARCHER_PARTICLE_INDEX: u16 = 0;
pub(crate) const ARCHER_SHIFT_ROT_X: i16 = ARCHER_CLASS_PROPERTIES.collision_half_width;
pub(crate) const ARCHER_SHIFT_ROT_Y: i16 = ARCHER_CLASS_PROPERTIES.collision_half_height;
// Particle-0 collision extent, not the Archer's body orientation.
pub(crate) const ARCHER_COLLISION_VERTICAL_OFFSET: i16 = 125;

pub(crate) const ARCHER_CHILD_SOURCE_CLASS: u8 = 9;
pub(crate) const ARCHER_CHILD_MODEL: u8 = 13;
pub(crate) const ARCHER_CHILD_ACTION: u8 = 13;
pub(crate) const ARCHER_CHILD_SPEED: i16 = 384;
pub(crate) const ARCHER_CHILD_MAX_LIFE: i16 = 5120 / ARCHER_CHILD_SPEED;
pub(crate) const ARCHER_CHILD_SUBSPELL_INDEX: u16 = ARCHER_CLASS_PROPERTIES.arrow_damage;
pub(crate) const ARCHER_CHILD_PARTICLE_INDEX: u16 = 195;
pub(crate) const ARCHER_CHILD_BODY_FOV: i16 = 0;

/// The model-4-only words owned by `AddArchers_4BA10`.  The root remains in
/// `authored_class5`; this typed sidecar supplies the linked-peer/head words,
/// constructor generation, and child-cursor guard without manufacturing a
/// second root arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5ArcherState {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub origin: ArcherOrigin,
    pub slot_generation: u32,
    pub word_0x32: u16,
    pub word_0x34: u16,
    pub descriptor_index: u16,
    pub byte_0x38: u8,
    pub xtype: u8,
    pub pending_damage: i32,
    pub last_child_root_phase: u8,
    pub control_flags: u32,
    pub xsubtype: u8,
    pub stage: ArcherStageState,
    pub proximity_delay: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcherOrigin {
    InitialPackage,
    AuthoredRecord(u16),
    HouseSpawn(u16),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArcherStageState {
    pub index: u8,
    pub mode: u8,
    pub relation: u16,
    pub parent: u16,
    pub remaining: i16,
    pub duration: i16,
}

/// Human-player Type_str_164 fields read/written by the Archer family.
/// AI players keep these fields in their existing common-upkeep owner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArcherPlayerFeedback {
    pub target_ticks: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherStageDescriptor {
    pub kind: u8,
    pub flags: u8,
    pub next: u8,
    pub point: Position,
    pub actor: u16,
    pub model: u8,
    pub scalar: u16,
}

/// The adapter borrows the ONE live StageVar table. `bind` advances that
/// table's source counter and applies thinning, rather than keeping a copy.
pub(crate) trait ArcherStageRuntime {
    fn descriptor(&self, index: u8) -> Option<ArcherStageDescriptor>;
    fn bind(&mut self, index: u8) -> Option<ArcherStageDescriptor>;
    fn rearm_model_relations(&mut self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherRelation {
    pub slot: u16,
    pub target: ArcherTarget,
    pub source_id: u16,
    pub parent: u16,
    pub stage_index: u8,
    pub stage_mode: u8,
    pub stage_relation: u16,
    pub attacker: u16,
    pub yaw: i16,
    pub roll: i16,
    pub half_width: i16,
    pub frozen_class5_member: bool,
}

impl Level2AuthoredClass5ArcherState {
    pub(crate) fn hash_source_state(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        hasher.update(self.source_entity_index.to_le_bytes());
        hasher.update(self.entity_id.to_le_bytes());
        let (origin, record) = match self.origin {
            ArcherOrigin::InitialPackage => (0, 0),
            ArcherOrigin::AuthoredRecord(record) => (1, record),
            ArcherOrigin::HouseSpawn(house) => (2, house),
        };
        hasher.update([origin]);
        hasher.update(record.to_le_bytes());
        hasher.update(self.slot_generation.to_le_bytes());
        hasher.update(self.word_0x32.to_le_bytes());
        hasher.update(self.word_0x34.to_le_bytes());
        hasher.update(self.descriptor_index.to_le_bytes());
        hasher.update([
            self.byte_0x38,
            self.xtype,
            self.xsubtype,
            self.last_child_root_phase,
        ]);
        hasher.update(self.pending_damage.to_le_bytes());
        hasher.update(self.control_flags.to_le_bytes());
        hasher.update([self.stage.index, self.stage.mode]);
        hasher.update(self.stage.relation.to_le_bytes());
        hasher.update(self.stage.parent.to_le_bytes());
        hasher.update(self.stage.remaining.to_le_bytes());
        hasher.update(self.stage.duration.to_le_bytes());
        hasher.update([self.proximity_delay]);
    }
}

/// Retained class-9/model-13 child emitted by Archer `sub_1CCE0`.  This is a
/// real shared source-arena row: its parent, source RNG, slot generation,
/// chain words, signed rotations, map position, cursor phase, and lifecycle
/// are all reset/hash/reclaim inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level2AuthoredClass5ArcherChild {
    pub source_entity_index: u16,
    pub entity_id: u16,
    pub entity_rng: u16,
    pub parent_entity_index: u16,
    pub parent_entity_id: u16,
    pub slot_generation: u32,
    pub word_0x32: u16,
    pub word_0x34: u16,
    pub word_0x24: u16,
    pub pending_word_0x62: u16,
    pub word_0x26: u16,
    pub word_0x96: u16,
    pub dword_0x10: i32,
    pub source_class: u8,
    pub model: u8,
    pub action: u8,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub actual_speed: i16,
    pub max_life: i16,
    pub life: i16,
    pub subspell_index: u16,
    pub particle_index: u16,
    pub collision_half_width: i16,
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
    pub control_flags: u32,
    pub xtype: u8,
    pub xsubtype: u8,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub fov: i16,
    pub shift_rot_x: i16,
    pub shift_rot_y: i16,
    pub position: Position,
    pub source_byte_3e: u8,
    pub phase: u8,
    pub drawing_enabled: bool,
    pub active: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArrowCollisionTarget {
    pub target: crate::native_world::level2_dynamic_economy::Level2FireballCollisionTarget,
    pub control_flags: u32,
    pub mana: i32,
}

pub(crate) struct ArrowCollisionFrame {
    pub targets: Vec<ArrowCollisionTarget>,
    pub owner_class_model: Option<(u8, u8)>,
}

/// A linked child observed by the `word_0x34` walk in `sub_1FAA0` and
/// `sub_1C310`.  The shared runtime already owns the head index; this row is
/// the narrow model-local view needed by the Archer action helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherLinkedState {
    pub(crate) entity_index: u16,
    pub(crate) next_entity_index: u16,
    pub(crate) life: i32,
    pub(crate) current_attacker: u16,
}

/// Target/candidate fields read by the Archer's source callbacks.  The
/// enclosing Level 2 session maps its imported entity rows into this narrow
/// view and applies the returned target mutations to the authoritative arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherTarget {
    pub(crate) entity_index: u16,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) active: bool,
    pub(crate) life: i32,
    pub(crate) control_flags: u32,
    pub(crate) minimum_speed: i16,
    pub(crate) maximum_speed: i16,
    pub(crate) actual_speed: i16,
    pub(crate) dword_0x10: i32,
    pub(crate) word_0x248: u16,
    pub(crate) word_0x32: u16,
    pub(crate) word_0x96_150: u16,
    pub(crate) position: Position,
}

/// The model-local residual context.  `word_0x32` is the one Archer root
/// field absent from `Level2AuthoredClass5RuntimeState`; callers pass it by
/// mutable reference rather than this module creating a duplicate arena.
pub(crate) struct ArcherActionContext<'a> {
    pub(crate) terrain: &'a SessionTerrain,
    pub(crate) control_flags: u32,
    pub(crate) pending_damage: i32,
    pub(crate) linked_head: u16,
    pub(crate) linked: &'a [ArcherLinkedState],
    pub(crate) target: Option<ArcherTarget>,
    pub(crate) entities: &'a [ArcherTarget],
    pub(crate) fallback_entities: &'a [ArcherTarget],
    pub(crate) peers: &'a [ArcherTarget],
    pub(crate) relations: &'a [ArcherRelation],
    pub(crate) stage: ArcherStageState,
    pub(crate) stage_runtime: Option<&'a mut dyn ArcherStageRuntime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherVisualUpdate {
    pub(crate) actual_speed: i16,
    pub(crate) particle_index: u16,
    pub(crate) shift_rot_x: i16,
    pub(crate) shift_rot_y: i16,
    pub(crate) xtype: u8,
    pub(crate) xsubtype: u8,
}

/// A source-local request emitted by `sub_1CCE0`.  It is intentionally a
/// request, not a second child vector: the shared economy must allocate and
/// retain the returned packet in its canonical child arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherChildRequest {
    pub(crate) parent_entity_index: u16,
    pub(crate) parent_entity_id: u16,
    pub(crate) parent_word_0x96: u16,
    pub(crate) parent_roll: i16,
    pub(crate) parent_position: Position,
    pub(crate) parent_fov: i16,
    pub(crate) target_entity_index: u16,
    pub(crate) target_class: u8,
    pub(crate) target_model: u8,
    pub(crate) target_position: Position,
}

/// Exact post-constructor class-9/model-13 child state from
/// `IfSubtypeCallCreatingManaSphere(..., 9, 13)` and `sub_1CCE0`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherChildPacket {
    pub(crate) source_entity_index: u16,
    pub(crate) entity_id: u16,
    pub(crate) entity_rng: u16,
    pub(crate) parent_entity_index: u16,
    pub(crate) parent_entity_id: u16,
    pub(crate) source_class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) minimum_speed: i16,
    pub(crate) maximum_speed: i16,
    pub(crate) actual_speed: i16,
    pub(crate) max_life: i16,
    pub(crate) life: i16,
    pub(crate) subspell_index: u16,
    pub(crate) particle_index: u16,
    pub(crate) dword_0x10: i32,
    pub(crate) control_flags: u32,
    pub(crate) word_0x96: u16,
    pub(crate) xtype: u8,
    pub(crate) xsubtype: u8,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) roll: i16,
    pub(crate) fov: i16,
    pub(crate) position: Position,
    pub(crate) source_byte_3e: u8,
    pub(crate) phase: u8,
    pub(crate) drawing_enabled: bool,
    pub(crate) active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherActionEffects {
    pub(crate) moved: bool,
    pub(crate) consumed_pending_damage: bool,
    pub(crate) selected_target: Option<u16>,
    pub(crate) released_target: bool,
    pub(crate) target_dword_0x10_incremented: bool,
    pub(crate) target_dword_0x10_entity: Option<u16>,
    pub(crate) target_feedback_entity: Option<u16>,
    pub(crate) child_request: Option<ArcherChildRequest>,
    pub(crate) visual: Option<ArcherVisualUpdate>,
    pub(crate) peer_update: Option<ArcherTarget>,
    pub(crate) expire_effect: bool,
    pub(crate) summon_expire_effect: bool,
    pub(crate) experience_award: Option<(u16, u8)>,
    pub(crate) population_decrement: bool,
    pub(crate) player_alert_entity: Option<u16>,
}

impl Default for ArcherActionEffects {
    fn default() -> Self {
        Self {
            moved: false,
            consumed_pending_damage: false,
            selected_target: None,
            released_target: false,
            target_dword_0x10_incremented: false,
            target_dword_0x10_entity: None,
            target_feedback_entity: None,
            child_request: None,
            visual: None,
            peer_update: None,
            expire_effect: false,
            summon_expire_effect: false,
            experience_award: None,
            population_decrement: false,
            player_alert_entity: None,
        }
    }
}

fn archer_target_is_live(target: ArcherTarget) -> bool {
    target.active && target.life >= 0 && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
}

fn archer_distance_squared_2d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

// sub_1DBF0 promotes the X subtraction to int but truncates Y to int16_t.
// Its guard scan therefore differs from sub_1FAA0 at the world boundary.
fn archer_guard_distance_squared(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x) - i32::from(left.x);
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32
}

/// Single-player `sub_68BF0 -> sub_68C70`, after StageVar prepass and before
/// action callbacks. The returned countdown is propagated to the body chain.
pub(crate) fn advance_archer_proximity(
    source: &mut Level2AuthoredClass5Source,
    state: &mut Level2AuthoredClass5ArcherState,
    player: Position,
) -> Option<u8> {
    if source.life < 0 {
        source.source_byte_39 = 250;
        state.proximity_delay = 0;
        return None;
    }
    if source.source_byte_39 != 0 {
        let prior = source.source_byte_39;
        source.source_byte_39 = prior.wrapping_sub(1);
        return Some(prior);
    }
    if state.control_flags & 1 != 0 {
        return None;
    }
    if state.proximity_delay != 0 {
        state.proximity_delay -= 1;
        return None;
    }
    state.proximity_delay = 0;
    if archer_distance_squared_2d(source.position, player) < 0x0240_0000 {
        source.source_byte_39 = 16;
        Some(18)
    } else {
        None
    }
}

fn archer_within_cone(source: &Level2AuthoredClass5Source, target: ArcherTarget) -> bool {
    let heading = heading_for_positions(source.position, target.position);
    i32::from(angle_distance(i32::from(source.yaw), heading))
        < i32::from(ARCHER_DESCRIPTOR_HALF_ANGLE)
}

fn archer_nearest_candidate(
    source: &Level2AuthoredClass5Source,
    candidates: &[ArcherTarget],
    radius_squared: u32,
    require_cone: bool,
    eligible: impl Fn(ArcherTarget) -> bool,
) -> Option<ArcherTarget> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            candidate.entity_index != source.source_entity_index
                && eligible(*candidate)
                && archer_distance_squared_2d(source.position, candidate.position) <= radius_squared
                && (!require_cone || archer_within_cone(source, *candidate))
        })
        .min_by_key(|candidate| archer_distance_squared_2d(source.position, candidate.position))
}

pub(crate) fn archer_attack_visual(
    source: &mut Level2AuthoredClass5Source,
    target: Option<ArcherTarget>,
) -> ArcherVisualUpdate {
    source.entity_rng = level2_next_rng16(source.entity_rng);
    source.actual_speed = 0;
    source.particle_index = if source.entity_rng % 0x14 <= 10 {
        206
    } else {
        1
    };
    source.shift_rot_x = ARCHER_SHIFT_ROT_X;
    source.shift_rot_y = ARCHER_SHIFT_ROT_Y;
    let (xtype, xsubtype) = target.map_or((3, u8::MAX), |target| (target.class, target.model));
    ArcherVisualUpdate {
        actual_speed: source.actual_speed,
        particle_index: source.particle_index,
        shift_rot_x: source.shift_rot_x,
        shift_rot_y: source.shift_rot_y,
        xtype,
        xsubtype,
    }
}

pub(crate) fn archer_idle_visual(source: &mut Level2AuthoredClass5Source) -> ArcherVisualUpdate {
    source.actual_speed = source.minimum_speed;
    source.particle_index = ARCHER_PARTICLE_INDEX;
    source.shift_rot_x = ARCHER_SHIFT_ROT_X;
    source.shift_rot_y = ARCHER_SHIFT_ROT_Y;
    ArcherVisualUpdate {
        actual_speed: source.actual_speed,
        particle_index: source.particle_index,
        shift_rot_x: source.shift_rot_x,
        shift_rot_y: source.shift_rot_y,
        xtype: 3,
        xsubtype: u8::MAX,
    }
}

fn archer_consume_mailbox(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
) -> (u8, bool) {
    let mut state = 0;
    let mut consumed_pending_damage = false;
    if consume_damage(
        &mut source.life,
        &mut context.pending_damage,
        &mut source.pending_word_0x62,
        &mut source.word_0x26,
    )
    .is_some()
    {
        state = 1;
        consumed_pending_damage = true;
    }
    let mut linked_entity_index = context.linked_head;
    for _ in 0..=context.linked.len() {
        if linked_entity_index == 0 {
            break;
        }
        let Some(linked) = context
            .linked
            .iter()
            .find(|linked| linked.entity_index == linked_entity_index)
        else {
            break;
        };
        if inherit_linked_life(
            &mut source.life,
            &mut source.word_0x26,
            linked.life,
            linked.current_attacker,
        ) {
            state = 1;
            break;
        }
        linked_entity_index = linked.next_entity_index;
    }
    if source.life < 0 {
        source.word_0x24 = source.word_0x26;
        state = 2;
    }
    (state, consumed_pending_damage)
}

/// `sub_580E0`, with descriptor 75's clearance zero and descent -128.
fn archer_settle(source: &mut Level2AuthoredClass5Source, terrain: &SessionTerrain) {
    let ground = terrain.ground_height(source.position);
    if source.position.z > ground {
        source.position.z = source
            .position
            .z
            .wrapping_add(ARCHER_CLASS_PROPERTIES.descent);
    }
    if source.position.z <= ground {
        source.position.z = ground;
    }
}

fn archer_slope(position: Position, world: &SessionTerrain) -> i32 {
    let x = position.x.to_be_bytes()[0];
    let y = position.y.to_be_bytes()[0];
    let height = |x, y| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let a = height(x, y);
    let b = height(x.wrapping_add(1), y);
    let c = height(x.wrapping_add(1), y.wrapping_add(1));
    let d = height(x, y.wrapping_add(1));
    (a + d - b - c).abs().max((a + b - d - c).abs())
}

/// Descriptor-75 `sub_1B8C0`: horizontal movement, byte-preserving retries,
/// forward footprint/cave admission, then the source yaw-to-roll turn.
fn archer_move(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
) -> bool {
    if context.control_flags & 0x800 != 0 {
        context.control_flags &= !0x800;
        return false;
    }
    let original = source.position;
    let yaw = source.yaw.cast_unsigned();
    let third = (yaw.wrapping_sub(85) & 0xff) | (yaw.wrapping_sub(341) & 0x700);
    let fourth = (yaw.wrapping_add(1024) & 0x700) | (yaw & 0xff);
    for (attempt, heading) in [yaw & 2047, yaw.wrapping_add(341) & 2047, third, fourth]
        .into_iter()
        .enumerate()
    {
        source.yaw = heading as i16;
        source.position = original;
        archer_settle(source, context.terrain);
        let mut predicted = source.position;
        source.position = original;
        crate::move_entity(&mut predicted, source.yaw, 0, source.actual_speed);
        let crosses = (predicted.x >> 8, predicted.y >> 8) != (original.x >> 8, original.y >> 8);
        let mut blocked = false;
        if crosses || attempt != 0 {
            let mut probe = predicted;
            let mut distance = 0;
            while distance <= i32::from(source.shift_rot_x.max(0)) {
                blocked = context.terrain.material_mask(probe)
                    & !ARCHER_CLASS_PROPERTIES.terrain_mask
                    != 0
                    || context.terrain.cave_collision_blocked(probe)
                    || context
                        .terrain
                        .ceiling_height(probe)
                        .is_some_and(|ceiling| {
                            context.terrain.angle_flags(crate::Tile {
                                x: probe.x.to_be_bytes()[0],
                                y: probe.y.to_be_bytes()[0],
                            }) & 8
                                != 0
                                || i32::from(context.terrain.ground_height(probe))
                                    + i32::from(source.shift_rot_y)
                                    > i32::from(ceiling)
                        });
                if blocked {
                    break;
                }
                crate::move_entity(&mut probe, source.yaw, 0, 256);
                distance += 256;
            }
            blocked |=
                archer_slope(predicted, context.terrain) >= ARCHER_CLASS_PROPERTIES.maximum_slope;
        }
        if !blocked {
            source.position = predicted;
            source.yaw = source.yaw.wrapping_add(crate::turn_toward(
                source.yaw,
                source.roll,
                ARCHER_CLASS_PROPERTIES.turn_limit,
            )) & 2047;
            if attempt == 0 {
                context.control_flags &= !0x40000;
            }
            return false;
        }
        context.control_flags |= 0x40000;
    }
    // Descriptor75 has behavior bit1, so complete obstruction kills the row.
    source.position = original;
    source.life = -1;
    true
}

fn archer_selected_target(context: &ArcherActionContext<'_>, slot: u16) -> Option<ArcherTarget> {
    context
        .target
        .filter(|target| target.entity_index == slot)
        .or_else(|| {
            context
                .entities
                .iter()
                .chain(context.fallback_entities)
                .chain(context.peers)
                .copied()
                .find(|target| target.entity_index == slot)
        })
        .or_else(|| {
            context
                .relations
                .iter()
                .find(|row| row.slot == slot)
                .map(|row| row.target)
        })
}

// The movement helpers compare id_0x1A, unlike target acquisition's pointer
// comparison. Summoned/imported body rows can occupy distinct slots with one id.
fn archer_source_id(context: &ArcherActionContext<'_>, slot: u16) -> u16 {
    context
        .relations
        .iter()
        .find(|row| row.slot == slot)
        .map_or(slot, |row| row.source_id)
}

pub(crate) fn bind_archer_stage(
    source: &mut Level2AuthoredClass5Source,
    stage: &mut ArcherStageState,
    index: u8,
    runtime: &mut dyn ArcherStageRuntime,
) {
    if index != 0
        && let Some(descriptor) = runtime.bind(index)
    {
        stage.index = index;
        stage.mode = descriptor.kind;
        stage.relation = if descriptor.kind == 6 {
            descriptor.scalar
        } else {
            0
        };
        source.action = ARCHER_ACTION_IDLE_VISUAL;
    } else {
        stage.index = 0;
        stage.mode = 0;
        stage.relation = 0;
        source.action = ARCHER_ACTION;
    }
}

fn signed_axis_near(a: Position, b: Position, distance: u32) -> bool {
    i32::from(a.x.cast_signed()).abs_diff(i32::from(b.x.cast_signed())) <= distance
        && i32::from(a.y.cast_signed()).abs_diff(i32::from(b.y.cast_signed())) <= distance
}

fn wrapped_axis_near(a: Position, b: Position, distance: u16) -> bool {
    a.x.wrapping_sub(b.x).cast_signed().unsigned_abs() <= distance
        && a.y.wrapping_sub(b.y).cast_signed().unsigned_abs() <= distance
}

/// `sub_12500`, once for each frozen living root before ANY action callback.
pub(crate) fn prepass_archer_stage(
    source: &mut Level2AuthoredClass5Source,
    stage: &mut ArcherStageState,
    runtime: &mut dyn ArcherStageRuntime,
    targets: &[ArcherTarget],
) {
    let action = source.action & 7;
    if matches!(action, 4 | 5) {
        return;
    }
    if matches!(stage.mode, 13 | 14 | 16 | 17) {
        if !matches!(action, 2 | 6) {
            source.action = ARCHER_ACTION_IDLE_VISUAL;
        }
        return;
    }
    if stage.mode == 10 {
        if !matches!(action, 2 | 6) {
            let index = stage.index;
            bind_archer_stage(source, stage, index, runtime);
        }
        return;
    }
    if stage.mode == 15 {
        if action != 0 {
            let index = stage.index;
            bind_archer_stage(source, stage, index, runtime);
        }
        return;
    }
    let Some(descriptor) = runtime.descriptor(stage.index) else {
        return;
    };
    let ended = match descriptor.kind {
        1 => wrapped_axis_near(source.position, descriptor.point, 2048),
        3 | 4 | 5 | 8 | 9 => {
            if descriptor.flags & 4 != 0 {
                if descriptor.kind == 3 {
                    stage.relation = 0;
                }
                !matches!(action, 2 | 6) || descriptor.kind != 3
            } else {
                if descriptor.flags & 2 != 0
                    && matches!(descriptor.kind, 3..=5)
                    && targets
                        .iter()
                        .find(|row| row.entity_index == stage.relation)
                        .is_some_and(|row| {
                            row.life < 0 || row.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
                        })
                {
                    stage.relation = 0;
                }
                descriptor.kind == 9
                    && descriptor.flags & 2 == 0
                    && (descriptor.point.x != 0 || descriptor.point.y != 0)
                    && wrapped_axis_near(source.position, descriptor.point, 3072)
            }
        }
        6 => {
            stage.relation = stage.relation.wrapping_sub(1);
            stage.relation == 0
        }
        7 => descriptor.flags & 0x18 != 0,
        _ => false,
    };
    if ended {
        if descriptor.kind == 7 {
            runtime.rearm_model_relations();
        }
        bind_archer_stage(source, stage, descriptor.next, runtime);
    }
}

fn archer_jitter(source: &mut Level2AuthoredClass5Source) {
    let first = level2_next_rng16(source.entity_rng);
    source.entity_rng = level2_next_rng16(first);
    let direction = 2 * (i32::from(first % 157) / 79) - 1;
    source.roll = (i32::from(source.roll) + (i32::from(source.entity_rng & 255) + 85) * direction)
        as i16
        & 2047;
}

fn stage_target(
    source: &Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
    descriptor: ArcherStageDescriptor,
) -> Option<ArcherTarget> {
    let slot = if descriptor.flags & 2 == 0 {
        descriptor.actor
    } else {
        if context.stage.relation == 0 {
            if descriptor.flags & 1 != 0 {
                context.stage.relation = context
                    .relations
                    .iter()
                    .find(|peer| {
                        peer.frozen_class5_member
                            && peer.target.class == 5
                            && peer.target.model == ARCHER_MODEL
                            && peer.stage_index == context.stage.index
                            && peer.stage_relation != 0
                    })
                    .map_or(0, |peer| peer.stage_relation);
            }
            if context.stage.relation == 0 {
                context.stage.relation = context
                    .relations
                    .iter()
                    .filter(|row| row.frozen_class5_member)
                    .map(|row| row.target)
                    .filter(|target| target.class == 5 && target.model == descriptor.model)
                    .min_by_key(|target| {
                        archer_distance_squared_2d(source.position, target.position)
                    })
                    .map_or(0, |target| target.entity_index);
            }
        }
        context.stage.relation
    };
    archer_selected_target(context, slot)
}

fn stage_follow(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
    target: Position,
    point_mode: bool,
    target_width: Option<i16>,
) {
    if source.source_byte_3e & 7 != 0 {
        return;
    }
    if context.control_flags & 0x40000 == 0 {
        source.roll = heading_for_positions(source.position, target) as i16;
        if source.source_byte_3e & 63 == 0 {
            archer_jitter(source);
        }
    }
    if let Some(peer) = context.peers.iter().find(|peer| {
        archer_source_id(context, peer.entity_index)
            != archer_source_id(context, source.source_entity_index)
            && if point_mode {
                // 1DDA0 explicitly casts both coordinates to int16_t.
                signed_axis_near(
                    source.position,
                    peer.position,
                    source.shift_rot_x.saturating_sub(1) as u32,
                )
            } else {
                // 1D8C0/1E700 promote the unsigned coordinates before
                // subtraction; crossing 0x8000 is an ordinary short gap.
                source.position.x.abs_diff(peer.position.x) < source.shift_rot_x as u16
                    && source.position.y.abs_diff(peer.position.y) < source.shift_rot_x as u16
            }
    }) {
        source.roll = heading_for_positions(peer.position, source.position) as i16;
    }
    if let Some(width) = target_width {
        let separation = i32::from(source.shift_rot_x) + i32::from(width);
        if i32::from(source.position.x.abs_diff(target.x)) < separation
            && i32::from(source.position.y.abs_diff(target.y)) < separation
        {
            source.roll = heading_for_positions(target, source.position) as i16;
        }
    }
}

fn alliance_blink(source: &Level2AuthoredClass5Source, context: &mut ArcherActionContext<'_>) {
    let stage = context.stage;
    let phase = if stage.remaining <= stage.duration.wrapping_sub(16) && stage.remaining >= 16 {
        if source.source_byte_3e & 0x1c == 16 {
            source.source_byte_3e & 3
        } else {
            0
        }
    } else if source.source_byte_3e & 1 != 0 {
        0
    } else {
        2
    };
    context.control_flags &= !0x0600_0000;
    context.control_flags |= match phase {
        1 | 3 => 0x0200_0000,
        2 => 0x0400_0000,
        _ => 0,
    };
}

/// Attack-side Alliance filtering is `sub_1ED30`, separate from the stage-39
/// callback. Its duration advances even when it rejects the current target.
fn archer_alliance_target(
    source: &Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
    target: Option<ArcherTarget>,
) -> Option<ArcherTarget> {
    if context.stage.mode != 14 {
        return target;
    }
    alliance_blink(source, context);
    context.stage.remaining = context.stage.remaining.wrapping_sub(1);
    if context.stage.remaining <= 0 {
        return None;
    }
    let Some(parent) = archer_selected_target(context, context.stage.parent) else {
        return target;
    };
    let target = target?;
    if target.entity_index == parent.entity_index {
        return None;
    }
    let attacker = context
        .relations
        .iter()
        .find(|row| row.slot == parent.entity_index)
        .map_or(0, |row| row.attacker);
    let selected = if parent.word_0x96_150 != 0 {
        parent.word_0x96_150
    } else {
        attacker
    };
    (selected == 0 || selected == target.entity_index).then_some(target)
}

fn summoned_follow(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
    effects: &mut ArcherActionEffects,
) {
    let (hit, consumed) = archer_consume_mailbox(source, context);
    effects.consumed_pending_damage = consumed;
    if hit == 2 {
        context.stage.remaining = 1;
        return;
    }
    effects.population_decrement |= archer_move(source, context);
    effects.moved = true;
    if hit == 1 {
        if source.word_0x26 != context.stage.parent
            && archer_selected_target(context, source.word_0x26)
                .is_none_or(|row| row.class != 5 || row.model != ARCHER_MODEL)
        {
            source.word_0x96_150 = source.word_0x26;
            source.action = ARCHER_ACTION_ATTACK;
            effects.experience_award = Some((context.stage.parent, 19));
        }
    } else if let Some(target) = archer_selected_target(context, source.word_0x96_150) {
        stage_follow(source, context, target.position, false, None);
    }
}

fn step_archer_summoned_stage(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
    effects: &mut ArcherActionEffects,
) {
    let mode = context.stage.mode;
    let parent = archer_selected_target(context, context.stage.parent);
    if mode == 14 {
        context.stage.remaining = context.stage.remaining.wrapping_sub(1);
        if context.stage.remaining <= 0 || parent.is_none_or(|parent| parent.life <= 0) {
            context.stage.mode = 10;
            source.word_0x96_150 = 0;
            context.stage.parent = 0;
            context.stage.index = context.stage.relation as u8;
            context.control_flags &= !0x0600_0000;
            return;
        }
        alliance_blink(source, context);
        let parent = parent.unwrap();
        let previous = source.word_0x96_150;
        source.word_0x96_150 = context.stage.parent;
        summoned_follow(source, context, effects);
        let parent_attacker = context
            .relations
            .iter()
            .find(|row| row.slot == parent.entity_index)
            .map_or(0, |row| row.attacker);
        let inherited = if parent.word_0x96_150 != 0 {
            parent.word_0x96_150
        } else {
            parent_attacker
        };
        source.word_0x96_150 = if inherited != 0 { inherited } else { previous };
        if let Some(target) = archer_selected_target(context, source.word_0x96_150) {
            let allied_sibling = context.relations.iter().any(|row| {
                row.slot == target.entity_index
                    && row.parent == context.stage.parent
                    && row.stage_mode == 14
            });
            if allied_sibling
                || target.life <= 0
                || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
            {
                source.word_0x96_150 = 0;
            } else if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                source.position,
                target.position,
            )) < ARCHER_DESCRIPTOR_RANGE
            {
                source.action = ARCHER_ACTION_ATTACK;
                effects.experience_award = Some((context.stage.parent, 24));
            }
        }
        return;
    }
    if parent.is_none_or(|parent| !archer_target_is_live(parent)) {
        context.stage.remaining = 0;
    } else if mode == 13 {
        context.stage.remaining = context.stage.remaining.wrapping_sub(1);
    }
    if context.stage.remaining <= 0 {
        effects.expire_effect = true;
        effects.summon_expire_effect = mode == 13;
        source.drawing_enabled = false;
        return;
    }
    let parent = parent.unwrap();
    let mut target = archer_selected_target(context, source.word_0x96_150);
    if target.is_some_and(|target| {
        target.life <= 0 || target.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
    }) {
        source.word_0x96_150 = 0;
        target = None;
    }
    if target.is_none() && source.source_byte_3e & 7 == 0 {
        let parent_id = context
            .relations
            .iter()
            .find(|row| row.slot == parent.entity_index)
            .map_or(parent.entity_index, |row| row.source_id);
        target = context
            .entities
            .iter()
            .copied()
            .filter(|row| row.class == 3 && matches!(row.model, 0 | 1))
            .filter(|row| {
                context
                    .relations
                    .iter()
                    .find(|relation| relation.slot == row.entity_index)
                    .map_or(row.entity_index, |relation| relation.source_id)
                    != parent_id
            })
            .min_by_key(|row| archer_distance_squared_2d(parent.position, row.position));
        source.word_0x96_150 = target.map_or(0, |row| row.entity_index);
    }
    if let Some(target) = target {
        summoned_follow(source, context, effects);
        if source.source_byte_3e & 7 == 0
            && crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                source.position,
                target.position,
            )) < ARCHER_DESCRIPTOR_RANGE
        {
            source.action = ARCHER_ACTION_ATTACK;
        }
    } else {
        source.word_0x96_150 = context.stage.parent;
        summoned_follow(source, context, effects);
        source.word_0x96_150 = 0;
        context.stage.remaining = context.stage.remaining.wrapping_sub(4);
    }
}

/// `AddScroll05_04_20140 -> sub_1D5D0(32)`; mode zero and unhandled
/// selectors retain the source no-op (apart from the outer dword reset).
pub(crate) fn step_archer_action39(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
) -> ArcherActionEffects {
    source.dword_0x10 = 0;
    let mut effects = ArcherActionEffects::default();
    let mode = context.stage.mode;
    if mode == 10 {
        source.action = ARCHER_ACTION_ATTACK;
    } else if mode == 12 {
        if let Some(parent) = archer_selected_target(context, context.stage.parent) {
            if parent.life < 0 {
                source.drawing_enabled = false;
            }
            source.position = parent.position;
            source.position.z = source.position.z.max(0);
            if let Some(relation) = context
                .relations
                .iter()
                .find(|row| row.slot == parent.entity_index)
            {
                source.roll = relation.roll;
                source.yaw = relation.yaw;
            }
        } else {
            source.drawing_enabled = false;
        }
    } else if mode == 17 {
        effects.population_decrement |= archer_move(source, context);
        effects.moved = true;
        if source.life < 0 {
            source.drawing_enabled = false;
        } else if let Some(target) = archer_selected_target(context, source.word_0x96_150)
            .filter(|target| archer_target_is_live(*target))
        {
            source.roll = heading_for_positions(source.position, target.position) as i16;
            source.actual_speed = source.actual_speed.wrapping_sub(8);
            if source.actual_speed <= 16 {
                context.stage.mode = 16;
            }
        } else {
            context.stage.mode = 16;
        }
    } else if matches!(mode, 13 | 14 | 16) {
        step_archer_summoned_stage(source, context, &mut effects);
    } else if matches!(mode, 1..=9) {
        let descriptor = context
            .stage_runtime
            .as_ref()
            .and_then(|runtime| runtime.descriptor(context.stage.index));
        let (hit, consumed) = archer_consume_mailbox(source, context);
        effects.consumed_pending_damage = consumed;
        if hit == 2 {
            source.action = ARCHER_ACTION_HIT;
        } else if hit == 1 {
            if archer_selected_target(context, source.word_0x26)
                .is_none_or(|target| target.class != 5 || target.model != ARCHER_MODEL)
            {
                source.word_0x96_150 = source.word_0x26;
                context.stage.mode = 10;
                source.action = ARCHER_ACTION_ATTACK;
            }
            archer_settle(source, context.terrain);
        } else {
            let directed = descriptor.is_some_and(|descriptor| {
                mode == 1
                    || (mode == 2 && !signed_axis_near(source.position, descriptor.point, 3072))
            });
            effects.population_decrement |= archer_move(source, context);
            effects.moved = true;
            if let Some(descriptor) = descriptor {
                if directed {
                    stage_follow(source, context, descriptor.point, true, None);
                } else if matches!(mode, 3..=5) && source.source_byte_3e & 7 == 0 {
                    let target = stage_target(source, context, descriptor);
                    if let Some(target) = target {
                        let width = context
                            .relations
                            .iter()
                            .find(|row| row.slot == target.entity_index)
                            .map_or(0, |row| row.half_width);
                        stage_follow(source, context, target.position, false, Some(width));
                    } else if source.source_byte_3e & 15 == 0
                        && context.control_flags & 0x40000 == 0
                    {
                        source.entity_rng = level2_next_rng16(source.entity_rng);
                        source.roll = source
                            .roll
                            .wrapping_add((source.entity_rng % 113) as i16 + 142)
                            & 2047;
                    }
                } else if source.source_byte_3e & 15 == 0 && context.control_flags & 0x40000 == 0 {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    source.roll = source
                        .roll
                        .wrapping_add((source.entity_rng % 113) as i16 + 142)
                        & 2047;
                }
            }
        }
        // 1D7C0/1D700 run this wrapper AFTER 1D8C0, including its hit and
        // lethal branches. Resolve its existing relation without acquiring
        // a new model target outside the base helper's eight-visit gate.
        if matches!(mode, 3 | 4)
            && source.source_byte_3e & 7 == 0
            && let Some(descriptor) = descriptor
        {
            let slot = if descriptor.flags & 2 != 0 {
                context.stage.relation
            } else {
                descriptor.actor
            };
            if let Some(target) = archer_selected_target(context, slot) {
                let selected = if mode == 4 {
                    archer_selected_target(context, target.word_0x96_150)
                } else {
                    Some(target)
                };
                if let Some(selected) = selected.filter(|selected| selected.entity_index != 0xae02)
                    && crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                        source.position,
                        selected.position,
                    )) <= ARCHER_DESCRIPTOR_RANGE
                {
                    source.word_0x96_150 = selected.entity_index;
                    context.stage.mode = 10;
                    source.action = ARCHER_ACTION_ATTACK;
                }
            }
        }
        // 1DBF0 similarly checks its guard scan after either base helper.
        if mode == 2
            && source.action == ARCHER_ACTION_IDLE_VISUAL
            && context.stage.mode == 2
            && source.source_byte_39 != 0
            && source
                .source_byte_3e
                .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
            && let Some(target) = context
                .entities
                .iter()
                .copied()
                .filter(|target| {
                    target.class == 3
                        && target.control_flags & 0x20 == 0
                        && archer_guard_distance_squared(source.position, target.position)
                            <= ARCHER_DESCRIPTOR_RANGE * ARCHER_DESCRIPTOR_RANGE
                        && archer_within_cone(source, *target)
                })
                .min_by_key(|target| {
                    archer_guard_distance_squared(source.position, target.position)
                })
        {
            source.word_0x96_150 = target.entity_index;
            context.stage.mode = 10;
            source.action = ARCHER_ACTION_ATTACK;
        }
    }
    if source.action == ARCHER_ACTION_ATTACK {
        effects.visual = Some(archer_attack_visual(
            source,
            archer_selected_target(context, source.word_0x96_150),
        ));
    }
    effects
}

/// `sub_1FA70 -> sub_1BD90(32)`. Descriptor75 has neither flee nor no-link
/// enabled. A stationary row consumes damage and can join an unlinked peer.
pub(crate) fn step_archer_action32(
    source: &mut Level2AuthoredClass5Source,
    word_0x32: &mut u16,
    context: &mut ArcherActionContext<'_>,
) -> ArcherActionEffects {
    let mut effects = ArcherActionEffects::default();
    let (mailbox, consumed) = archer_consume_mailbox(source, context);
    effects.consumed_pending_damage = consumed;
    if mailbox == 2 {
        source.action = ARCHER_ACTION_HIT;
    } else if mailbox == 1 {
        source.word_0x96_150 = source.word_0x26;
        source.action = ARCHER_ACTION_ATTACK;
    } else if source
        .source_byte_3e
        .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
    {
        let range = ARCHER_DESCRIPTOR_RANGE * ARCHER_DESCRIPTOR_RANGE;
        let mut best = None;
        let mut best_distance = u32::MAX;
        for peer in context.peers.iter().copied().filter(|peer| {
            peer.class == 5
                && peer.model == 4
                && peer.word_0x32 == 0
                && peer.entity_index != source.source_entity_index
        }) {
            let distance = archer_distance_squared_2d(source.position, peer.position);
            // This stationary helper tests the reverse bearing.
            let bearing = heading_for_positions(peer.position, source.position);
            if distance <= range
                && distance < best_distance
                && angle_distance(i32::from(source.yaw), bearing) < ARCHER_DESCRIPTOR_HALF_ANGLE
            {
                best = Some(peer.entity_index);
                best_distance = distance;
            }
        }
        if let Some(peer) = best {
            *word_0x32 = peer;
            source.action = ARCHER_ACTION_LINK;
        }
    }
    archer_settle(source, context.terrain);
    if source.action == ARCHER_ACTION_ATTACK {
        effects.visual = Some(archer_attack_visual(
            source,
            archer_selected_target(context, source.word_0x96_150),
        ));
    }
    effects
}

/// Port the pinned class-5/model-4/action-33 (`sub_1FAA0`) body.  The
/// `word_0x32` reference is explicit because the current shared class-5
/// runtime carrier does not yet contain it.
pub(crate) fn step_archer_action33(
    source: &mut Level2AuthoredClass5Source,
    word_0x32: &mut u16,
    context: &mut ArcherActionContext<'_>,
) -> ArcherActionEffects {
    debug_assert_eq!(source.model, ARCHER_MODEL);
    debug_assert_eq!(source.action, ARCHER_ACTION);
    let mut effects = ArcherActionEffects::default();
    source.dword_0x10 = 0;
    let (mailbox_state, consumed_pending_damage) = archer_consume_mailbox(source, context);
    effects.consumed_pending_damage = consumed_pending_damage;
    if mailbox_state == 1 {
        source.word_0x96_150 = source.word_0x26;
        source.action = ARCHER_ACTION_ATTACK;
        effects.selected_target = Some(source.word_0x26);
        archer_settle(source, context.terrain);
        effects.visual = Some(archer_attack_visual(
            source,
            archer_selected_target(context, source.word_0x26),
        ));
        return effects;
    }
    if mailbox_state == 2 {
        source.action = ARCHER_ACTION_HIT;
        return effects;
    }

    effects.population_decrement |= archer_move(source, context);
    effects.moved = true;
    if !source
        .source_byte_3e
        .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
    {
        return effects;
    }

    if source.word_0x96_150 != 0 {
        let tracked_target = context
            .target
            .filter(|target| target.entity_index == source.word_0x96_150);
        match tracked_target {
            Some(target)
                if archer_target_is_live(target) && target.class == 10 && target.model == 45 =>
            {
                if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                    source.position,
                    target.position,
                )) > 0x1000
                {
                    source.roll =
                        i16::try_from(heading_for_positions(source.position, target.position))
                            .expect("Archer target heading fits i16");
                } else {
                    source.dword_0x10 = 1;
                    source.action = ARCHER_ACTION_HIT;
                    let mut target = target;
                    target.dword_0x10 = target.dword_0x10.wrapping_add(1);
                    context.target = Some(target);
                    effects.target_dword_0x10_incremented = true;
                    effects.target_dword_0x10_entity = Some(target.entity_index);
                }
            }
            _ => {
                source.word_0x96_150 = 0;
                effects.released_target = true;
            }
        }
        return effects;
    }

    let first_rng = level2_next_rng16(source.entity_rng);
    source.entity_rng = first_rng;
    let second_rng = level2_next_rng16(source.entity_rng);
    source.entity_rng = second_rng;
    let direction = 2 * (i32::from(first_rng % 0x009d) / 79) - 1;
    let turn = (i32::from(second_rng & 0x00ff) + 85) * direction;
    source.roll = i16::try_from((i32::from(source.roll).wrapping_add(turn)) & 0x07ff)
        .expect("Archer roll fits 11 bits");

    if source
        .source_byte_3e
        .is_multiple_of(4 * ARCHER_DESCRIPTOR_CADENCE)
    {
        let radius_squared = ARCHER_DESCRIPTOR_RANGE * ARCHER_DESCRIPTOR_RANGE;
        let preferred =
            archer_nearest_candidate(source, context.entities, radius_squared, true, |target| {
                target.class == 3 && target.control_flags & 0x20 == 0
            })
            .filter(|candidate| {
                (candidate.model == 0 || candidate.model == 1) && candidate.word_0x248 != 0
            });
        let fallback = preferred.or_else(|| {
            archer_nearest_candidate(
                source,
                context.fallback_entities,
                radius_squared,
                false,
                |target| target.class == 5 && target.model == 9,
            )
        });
        if let Some(target) = fallback {
            if !(target.class == 10 && target.model == 45) {
                source.word_0x96_150 = target.entity_index;
                source.action = ARCHER_ACTION_ATTACK;
                effects.selected_target = Some(target.entity_index);
                effects.visual = Some(archer_attack_visual(source, Some(target)));
            }
            return effects;
        }
        if let Some(peer) =
            archer_nearest_candidate(source, context.peers, radius_squared, true, |target| {
                target.class == 5 && target.model == 4 && target.word_0x32 == 0
            })
        {
            *word_0x32 = peer.entity_index;
            source.action = ARCHER_ACTION_LINK;
        }
    }
    effects
}

/// Port `AddArcher0504_1FF40` and its `sub_1C310(..., 32, sub_1CCE0)` child
/// callback.  A successful in-range cadence visit emits one child request;
/// the request is later passed to `allocate_archer_child_packet` by the
/// shared arena owner.
pub(crate) fn step_archer_action34(
    source: &mut Level2AuthoredClass5Source,
    context: &mut ArcherActionContext<'_>,
) -> ArcherActionEffects {
    debug_assert_eq!(source.model, ARCHER_MODEL);
    debug_assert_eq!(source.action, ARCHER_ACTION_ATTACK);
    let mut effects = ArcherActionEffects::default();
    let (mailbox_state, consumed_pending_damage) = archer_consume_mailbox(source, context);
    effects.consumed_pending_damage = consumed_pending_damage;
    if mailbox_state == 1 {
        source.word_0x96_150 = source.word_0x26;
        effects.selected_target = Some(source.word_0x26);
        archer_settle(source, context.terrain);
    } else if mailbox_state == 2 {
        source.action = ARCHER_ACTION_HIT;
        effects.visual = Some(archer_idle_visual(source));
        return effects;
    } else {
        effects.population_decrement |= archer_move(source, context);
        effects.moved = true;
        let selected = archer_selected_target(context, source.word_0x96_150);
        let Some(target) = archer_alliance_target(source, context, selected)
            .filter(|target| archer_target_is_live(*target))
        else {
            source.action = ARCHER_ACTION;
            effects.visual = Some(archer_idle_visual(source));
            return effects;
        };
        if source.source_byte_3e & 3 == 0 {
            source.roll = i16::try_from(heading_for_positions(source.position, target.position))
                .expect("Archer target heading fits i16");
            if let Some(peer) = context.peers.iter().copied().find(|peer| {
                archer_source_id(context, peer.entity_index)
                    != archer_source_id(context, source.source_entity_index)
                    && peer.class == ARCHER_SOURCE_CLASS
                    && peer.model == ARCHER_MODEL
                    && i32::from(peer.position.x.abs_diff(source.position.x))
                        < i32::from(source.shift_rot_x)
                    && i32::from(peer.position.y.abs_diff(source.position.y))
                        < i32::from(source.shift_rot_x)
            }) {
                source.roll = i16::try_from(heading_for_positions(peer.position, source.position))
                    .expect("Archer peer heading fits i16");
            }
        }
        if source
            .source_byte_3e
            .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
        {
            if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                source.position,
                target.position,
            )) >= ARCHER_DESCRIPTOR_RANGE
            {
                source.action = ARCHER_ACTION;
                effects.visual = Some(archer_idle_visual(source));
                return effects;
            }
            let request = ArcherChildRequest {
                parent_entity_index: source.source_entity_index,
                parent_entity_id: context
                    .relations
                    .iter()
                    .find(|row| row.slot == source.source_entity_index)
                    .map_or(source.source_entity_index, |row| row.source_id),
                parent_word_0x96: source.word_0x96_150,
                parent_roll: source.roll,
                parent_position: source.position,
                parent_fov: source.shift_rot_y,
                target_entity_index: target.entity_index,
                target_class: target.class,
                target_model: target.model,
                target_position: target.position,
            };
            effects.child_request = Some(request);
            effects.target_feedback_entity = Some(target.entity_index);
            let mut target = target;
            if target.model == 0 || target.model == 1 {
                target.word_0x248 = 200;
                context.target = Some(target);
            }
        }
    }
    if source.action == ARCHER_ACTION_ATTACK
        && source
            .source_byte_3e
            .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
        && let Some(target) = archer_selected_target(context, source.word_0x96_150)
        && target.class == 3
        && matches!(target.model, 0 | 1)
    {
        effects.player_alert_entity = Some(target.entity_index);
    }
    effects
}

/// Port `sub_1FFE0`/`sub_1C560` for the model-local fields.  The linked
/// relation lives in the caller's explicit `word_0x32` sidecar.
pub(crate) fn step_archer_action35(
    source: &mut Level2AuthoredClass5Source,
    word_0x32: &mut u16,
    context: &mut ArcherActionContext<'_>,
) -> ArcherActionEffects {
    let mut effects = ArcherActionEffects::default();
    if *word_0x32 == 0 {
        source.action = ARCHER_ACTION;
        return effects;
    }
    let peer = archer_selected_target(context, *word_0x32).filter(|peer| {
        archer_target_is_live(*peer) && peer.class == 5 && peer.model == ARCHER_MODEL
    });
    let (mailbox, consumed) = archer_consume_mailbox(source, context);
    effects.consumed_pending_damage = consumed;
    if mailbox != 0 {
        if let Some(mut peer) = peer {
            peer.word_0x96_150 = source.word_0x26;
            peer.word_0x32 = 0;
            peer.action = ARCHER_ACTION_ATTACK;
            effects.peer_update = Some(peer);
        }
        *word_0x32 = 0;
        if mailbox == 2 {
            source.action = ARCHER_ACTION_HIT;
        } else {
            source.word_0x96_150 = source.word_0x26;
            source.action = ARCHER_ACTION_ATTACK;
            archer_settle(source, context.terrain);
            effects.visual = Some(archer_attack_visual(
                source,
                archer_selected_target(context, source.word_0x26),
            ));
        }
        return effects;
    }
    effects.population_decrement |= archer_move(source, context);
    effects.moved = true;
    let Some(peer) = peer else {
        *word_0x32 = 0;
        source.action = ARCHER_ACTION;
        return effects;
    };
    if !source
        .source_byte_3e
        .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
    {
        return effects;
    }
    match peer.action {
        ARCHER_ACTION_STATIONARY | ARCHER_ACTION => {}
        ARCHER_ACTION_ATTACK => {
            source.word_0x96_150 = peer.word_0x96_150;
            *word_0x32 = 0;
            source.action = ARCHER_ACTION_ATTACK;
            effects.visual = Some(archer_attack_visual(
                source,
                archer_selected_target(context, source.word_0x96_150),
            ));
            return effects;
        }
        ARCHER_ACTION_LINK => *word_0x32 = peer.word_0x32,
        ARCHER_ACTION_ATTACK_VISUAL => {
            source.word_0x96_150 = peer.word_0x96_150;
            *word_0x32 = 0;
            source.action = ARCHER_ACTION_ATTACK_VISUAL;
            return effects;
        }
        _ => {
            *word_0x32 = 0;
            source.action = ARCHER_ACTION;
            return effects;
        }
    }
    if let Some(leader) = archer_selected_target(context, *word_0x32) {
        source.roll = heading_for_positions(source.position, leader.position) as i16;
    }
    if let Some(neighbor) = context.peers.iter().find(|other| {
        archer_source_id(context, other.entity_index)
            != archer_source_id(context, source.source_entity_index)
            && i32::from(other.position.x.cast_signed())
                .abs_diff(i32::from(source.position.x.cast_signed()))
                < 256
            && i32::from(other.position.y.cast_signed())
                .abs_diff(i32::from(source.position.y.cast_signed()))
                < 256
    }) {
        source.roll = heading_for_positions(neighbor.position, source.position) as i16;
    }
    source.actual_speed = peer.maximum_speed.wrapping_add(peer.actual_speed);
    effects
}

/// Source action 36 (`HitArcher_20010`).  Pre-kill propagation to a shared
/// linked child chain remains owned by the caller because the current carrier
/// does not expose the child rows.
pub(crate) fn step_archer_action36(source: &mut Level2AuthoredClass5Source) {
    debug_assert_eq!(source.model, ARCHER_MODEL);
    debug_assert_eq!(source.action, ARCHER_ACTION_HIT);
    if source.dword_0x10 != 0 {
        source.drawing_enabled = false;
    } else {
        // `PreKillEntity(event, 32)` stores `state + 5`; class-5 dispatch
        // therefore enters KillArcher at action 37, not the unrelated action
        // 41 scroll constructor.
        source.action = ARCHER_ACTION_KILL;
    }
}

/// Build the exact class-9/model-13 child packet after `sub_1CCE0` has
/// supplied the parent/target relation.  The child constructor itself does
/// not consume an LCG draw; `child_entity_rng` is the NewEvent value for its
/// newly allocated source slot.
#[must_use]
pub(crate) fn archer_child_packet(
    child_source_entity_index: u16,
    child_entity_rng: u16,
    request: ArcherChildRequest,
) -> ArcherChildPacket {
    let yaw = i16::try_from(heading_for_positions(
        request.parent_position,
        request.target_position,
    ))
    .expect("Archer child heading fits i16");
    let dx = i32::from(
        request
            .target_position
            .x
            .wrapping_sub(request.parent_position.x)
            .cast_signed(),
    );
    let dy = i32::from(
        request
            .target_position
            .y
            .wrapping_sub(request.parent_position.y)
            .cast_signed(),
    );
    let horizontal = crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .cast_unsigned(),
    );
    let pitch = crate::pitch_for_delta(
        i32::try_from(horizontal).expect("Archer child horizontal distance fits"),
        i32::from(
            request
                .target_position
                .z
                .wrapping_sub(request.parent_position.z),
        ),
    );
    let position = Position {
        x: request.parent_position.x,
        y: request.parent_position.y,
        z: request
            .parent_position
            .z
            .wrapping_add(request.parent_fov / 2),
    };
    ArcherChildPacket {
        source_entity_index: child_source_entity_index,
        entity_id: request.parent_entity_id,
        entity_rng: child_entity_rng,
        parent_entity_index: request.parent_entity_index,
        parent_entity_id: request.parent_entity_id,
        source_class: ARCHER_CHILD_SOURCE_CLASS,
        model: ARCHER_CHILD_MODEL,
        action: ARCHER_CHILD_ACTION,
        minimum_speed: ARCHER_CHILD_SPEED,
        maximum_speed: 0,
        actual_speed: ARCHER_CHILD_SPEED,
        max_life: ARCHER_CHILD_MAX_LIFE,
        life: ARCHER_CHILD_MAX_LIFE,
        subspell_index: ARCHER_CHILD_SUBSPELL_INDEX,
        particle_index: ARCHER_CHILD_PARTICLE_INDEX,
        dword_0x10: 0,
        // AddEvent09_0D clears NewEvent's damageable bit before map linking.
        control_flags: 4,
        word_0x96: request.parent_word_0x96,
        xtype: request.target_class,
        xsubtype: request.target_model,
        yaw,
        pitch,
        // NewEvent clears body roll; neither AddEvent09_0D nor the Archer
        // sub_1CCE0 tail copies the parent's body orientation into the arrow.
        roll: 0,
        fov: ARCHER_CHILD_BODY_FOV,
        position,
        // NewEvent seeds byte_0x3E from the fixed arena slot before the
        // ordinary callback epilogue advances it.
        source_byte_3e: child_source_entity_index as u8,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}
