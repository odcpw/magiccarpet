//! Reachable Level 2 class-5/model-4 Archer family.
//!
//! The pinned source calls this family `AddArchers_4BA10` and dispatches its
//! live root through actions 33, 34, and 35.  Action 34's `sub_1CCE0`
//! callback allocates a class-9/model-13 child.  The enclosing economy owns
//! the shared source arena, hash, reset, and dispatch carrier; this file owns
//! the source-exact action packet and typed constructor helpers used by it.

use super::super::*;

/// Group-20 authored Archer rows, in owner package/source order.
pub(crate) const ARCHER_AUTHORED_ROWS: [u16; 4] = [232, 233, 234, 235];

pub(crate) const ARCHER_SOURCE_CLASS: u8 = 5;
pub(crate) const ARCHER_MODEL: u8 = 4;
pub(crate) const ARCHER_ACTION: u8 = 33;
pub(crate) const ARCHER_ACTION_ATTACK: u8 = 34;
pub(crate) const ARCHER_ACTION_LINK: u8 = 35;
pub(crate) const ARCHER_ACTION_HIT: u8 = 36;
pub(crate) const ARCHER_ACTION_KILL: u8 = 37;
pub(crate) const ARCHER_ACTION_ATTACK_VISUAL: u8 = 38;
pub(crate) const ARCHER_ACTION_IDLE_VISUAL: u8 = 39;

pub(crate) const ARCHER_MINIMUM_SPEED: i16 = 30;
pub(crate) const ARCHER_MAXIMUM_SPEED: i16 = 0;
pub(crate) const ARCHER_MAX_LIFE: i32 = 1_000;
pub(crate) const ARCHER_MANA: i32 = 500;
pub(crate) const ARCHER_MAX_MANA: i32 = 0;
pub(crate) const ARCHER_SUBSPELL_INDEX: u16 = 500;
pub(crate) const ARCHER_DESCRIPTOR_INDEX: u16 = 75;
pub(crate) const ARCHER_DESCRIPTOR_CADENCE: u8 = 30;
pub(crate) const ARCHER_DESCRIPTOR_RANGE: u32 = 0x1400;
pub(crate) const ARCHER_DESCRIPTOR_HALF_ANGLE: i16 = 0x0200;
pub(crate) const ARCHER_PARTICLE_INDEX: u16 = 0;
pub(crate) const ARCHER_SHIFT_ROT_X: i16 = 128;
pub(crate) const ARCHER_SHIFT_ROT_Y: i16 = 256;
pub(crate) const ARCHER_POSE_YAW: i16 = 125;
pub(crate) const ARCHER_POSE_PITCH: i16 = 66;
pub(crate) const ARCHER_POSE_ROLL: i16 = 66;
pub(crate) const ARCHER_POSE_FOV: i16 = 125;

pub(crate) const ARCHER_CHILD_SOURCE_CLASS: u8 = 9;
pub(crate) const ARCHER_CHILD_MODEL: u8 = 13;
pub(crate) const ARCHER_CHILD_ACTION: u8 = 13;
pub(crate) const ARCHER_CHILD_SPEED: i16 = 384;
pub(crate) const ARCHER_CHILD_MAX_LIFE: i16 = 5120 / ARCHER_CHILD_SPEED;
pub(crate) const ARCHER_CHILD_SUBSPELL_INDEX: u16 = 250;
pub(crate) const ARCHER_CHILD_PARTICLE_INDEX: u16 = 195;
pub(crate) const ARCHER_CHILD_PARTICLE_FOV: i16 = 60;

#[must_use]
pub(crate) fn is_authored_archer_row(authored_index: u16) -> bool {
    ARCHER_AUTHORED_ROWS.contains(&authored_index)
}

/// The complete `AddArchers_4BA10` source packet before projection into the
/// pre-existing class-5 root carrier.  `source_byte_39` is not decorative:
/// descriptor row 75 uses it as the first cadence countdown, so dropping it
/// changes the first target/child callback after construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherConstructorPayload {
    pub(crate) source_entity_index: u16,
    pub(crate) entity_rng: u16,
    pub(crate) authored_index: u16,
    pub(crate) source_class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) minimum_speed: i16,
    pub(crate) maximum_speed: i16,
    pub(crate) actual_speed: i16,
    pub(crate) max_life: i32,
    pub(crate) life: i32,
    pub(crate) mana: i32,
    pub(crate) max_mana: i32,
    pub(crate) subspell_index: u16,
    pub(crate) initial_dword_0x10: i32,
    pub(crate) dword_0x10: i32,
    pub(crate) byte_0x38: u8,
    pub(crate) descriptor_index: u16,
    pub(crate) source_byte_39: u8,
    pub(crate) xtype: u8,
    pub(crate) source_byte_3e: u8,
    pub(crate) particle_index: u16,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) roll: i16,
    pub(crate) fov: i16,
    pub(crate) shift_rot_x: i16,
    pub(crate) shift_rot_y: i16,
    pub(crate) position: Position,
    pub(crate) phase: u8,
    pub(crate) drawing_enabled: bool,
    pub(crate) active: bool,
}

/// Reproduce the ordered writes in `AddArchers_4BA10` after `NewEvent_4A050`.
/// `initial_rng` is the value installed by `NewEvent`; the constructor's one
/// LCG draw is retained even though `SetEntityIndexAndRot(0)` overwrites the
/// random orientation with particle row 0's fixed pose.
#[must_use]
pub(crate) fn archer_constructor_payload(
    source_entity_index: u16,
    initial_rng: u16,
    authored_index: u16,
    source_byte_3e: u8,
    position: Position,
) -> ArcherConstructorPayload {
    let entity_rng = level2_next_rng16(initial_rng);
    let source_byte_39 = ARCHER_DESCRIPTOR_CADENCE
        .wrapping_sub(source_byte_3e % ARCHER_DESCRIPTOR_CADENCE)
        .wrapping_add(4);
    ArcherConstructorPayload {
        source_entity_index,
        entity_rng,
        authored_index,
        source_class: ARCHER_SOURCE_CLASS,
        model: ARCHER_MODEL,
        action: ARCHER_ACTION,
        minimum_speed: ARCHER_MINIMUM_SPEED,
        maximum_speed: ARCHER_MAXIMUM_SPEED,
        actual_speed: ARCHER_MINIMUM_SPEED,
        max_life: ARCHER_MAX_LIFE,
        life: ARCHER_MAX_LIFE,
        mana: ARCHER_MANA,
        max_mana: ARCHER_MAX_MANA,
        subspell_index: ARCHER_SUBSPELL_INDEX,
        initial_dword_0x10: i32::from(source_entity_index % 100),
        dword_0x10: i32::from(source_entity_index % 100),
        byte_0x38: 1,
        descriptor_index: ARCHER_DESCRIPTOR_INDEX,
        source_byte_39,
        xtype: 3,
        source_byte_3e,
        particle_index: ARCHER_PARTICLE_INDEX,
        yaw: ARCHER_POSE_YAW,
        pitch: ARCHER_POSE_PITCH,
        roll: ARCHER_POSE_ROLL,
        fov: ARCHER_POSE_FOV,
        shift_rot_x: ARCHER_SHIFT_ROT_X,
        shift_rot_y: ARCHER_SHIFT_ROT_Y,
        position,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}

impl ArcherConstructorPayload {
    /// Project the packet into the existing class-5 root carrier.  The
    /// runtime sidecar still carries byte-0x38, xtype, and descriptor index;
    /// those are installed by `allocate_authored_archer` below.
    #[must_use]
    fn into_class5_source(self) -> Level2AuthoredClass5Source {
        Level2AuthoredClass5Source {
            source_entity_index: self.source_entity_index,
            entity_rng: self.entity_rng,
            authored_index: self.authored_index,
            model: self.model,
            action: self.action,
            minimum_speed: self.minimum_speed,
            maximum_speed: self.maximum_speed,
            actual_speed: self.actual_speed,
            max_life: self.max_life,
            life: self.life,
            mana: self.mana,
            max_mana: self.max_mana,
            subspell_index: self.subspell_index,
            dword_0x10: self.dword_0x10,
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: self.particle_index,
            source_byte_3e: self.source_byte_3e,
            source_byte_39: self.source_byte_39,
            yaw: self.yaw,
            pitch: self.pitch,
            roll: self.roll,
            fov: self.fov,
            shift_rot_x: self.shift_rot_x,
            shift_rot_y: self.shift_rot_y,
            position: self.position,
            phase: self.phase,
            drawing_enabled: self.drawing_enabled,
            active: self.active,
        }
    }
}

impl Level2EconomyState {
    fn commit_class9_model13_child(
        &mut self,
        request: ArcherChildRequest,
        linked_parent_index: Option<usize>,
        castle_auxiliary_tail: bool,
    ) -> Option<ArcherChildPacket> {
        let state_before = self.clone();
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut packet = archer_child_packet(source_entity_index, entity_rng, request);
        if castle_auxiliary_tail {
            // `sub_23E60` keeps AddEvent09_0D's NewEvent subspell (100),
            // then copies the model-15 auxiliary's xtype/xsubtype (3/-1).
            // The Archer-only sub_1CCE0 tail instead installs 250 and the
            // target's class/model, so it cannot be reused unchanged.
            packet.subspell_index = 100;
            packet.xtype = 3;
            packet.xsubtype = u8::MAX;
        }
        let position = Position {
            x: request.parent_position.x,
            y: request.parent_position.y,
            z: request
                .parent_position
                .z
                .wrapping_add(request.parent_fov / 2),
        };
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            *self = state_before;
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        // NewEvent_4A050 clears the source player/owner relation before the
        // class-9/model-13 constructor writes its own fields. Keep a reused
        // arena slot from leaking the prior occupant's owner into this child.
        self.source_owner.insert(source_entity_index, 0);
        let next_entity_index = linked_parent_index
            .map(|parent_index| self.authored_class5_archer[parent_index].word_0x34)
            .unwrap_or(0);
        if let Some(parent_index) = linked_parent_index {
            self.authored_class5_archer[parent_index].word_0x34 = source_entity_index;
        }
        self.authored_class5_archer_children
            .push(Level2AuthoredClass5ArcherChild {
                source_entity_index: packet.source_entity_index,
                entity_id: packet.entity_id,
                entity_rng: packet.entity_rng,
                parent_entity_index: packet.parent_entity_index,
                parent_entity_id: packet.parent_entity_id,
                slot_generation: self.slot_generation(source_entity_index),
                word_0x32: 0,
                word_0x34: next_entity_index,
                word_0x24: 0,
                pending_word_0x62: 0,
                word_0x26: 0,
                word_0x96: packet.word_0x96,
                dword_0x10: packet.dword_0x10,
                source_class: packet.source_class,
                model: packet.model,
                action: packet.action,
                minimum_speed: packet.minimum_speed,
                maximum_speed: packet.maximum_speed,
                actual_speed: packet.actual_speed,
                max_life: packet.max_life,
                life: packet.life,
                subspell_index: packet.subspell_index,
                particle_index: packet.particle_index,
                control_flags: packet.control_flags,
                xtype: packet.xtype,
                xsubtype: packet.xsubtype,
                yaw: packet.yaw,
                pitch: packet.pitch,
                roll: packet.roll,
                fov: packet.fov,
                shift_rot_x: if castle_auxiliary_tail {
                    0
                } else {
                    ARCHER_SHIFT_ROT_X
                },
                shift_rot_y: if castle_auxiliary_tail {
                    0
                } else {
                    ARCHER_SHIFT_ROT_Y
                },
                position: packet.position,
                source_byte_3e: packet.source_byte_3e,
                phase: packet.phase,
                drawing_enabled: packet.drawing_enabled,
                active: packet.active,
            });
        Some(packet)
    }

    /// Allocate one authored Archer through the real source free stack,
    /// NewEvent RNG, model-4 constructor counter, terrain/map projection, and
    /// reset-owned class-5 source arena.
    pub(crate) fn allocate_authored_archer(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if !is_authored_archer_row(authored_index) {
            return None;
        }
        let state_before = self.clone();
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let source_byte_3e = self.class5_model4_constructor_phase;
        self.class5_model4_constructor_phase = source_byte_3e.wrapping_add(1);

        let mut position = position;
        position.z = terrain.ground_height(position);
        let payload = archer_constructor_payload(
            source_entity_index,
            initial_rng,
            authored_index,
            source_byte_3e,
            position,
        );
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_entity_index,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_err()
        {
            *self = state_before;
            return None;
        }
        let source = payload.into_class5_source();
        self.source_rng
            .insert(source_entity_index, source.entity_rng);
        // NewEvent_4A050 memsets playerEntityIndex to zero. The Archer
        // constructor does not replace it, so a reused source slot must not
        // retain the previous occupant's owner relation.
        self.source_owner.insert(source_entity_index, 0);
        self.authored_class5.push(source);
        self.authored_class5_runtime
            .push(Level2AuthoredClass5RuntimeState {
                source_entity_index,
                byte_0x38: payload.byte_0x38,
                xtype: payload.xtype,
                descriptor_index: payload.descriptor_index,
                pending_damage: 0,
                linked_head: 0,
            });
        self.authored_class5_archer
            .push(Level2AuthoredClass5ArcherState {
                source_entity_index,
                slot_generation: self.slot_generation(source_entity_index),
                word_0x32: 0,
                word_0x34: 0,
                descriptor_index: payload.descriptor_index,
                byte_0x38: payload.byte_0x38,
                xtype: payload.xtype,
                pending_damage: 0,
                last_child_root_phase: 0,
            });
        Some(source)
    }
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
    pub(crate) pending_damage: i32,
    pub(crate) linked_head: u16,
    pub(crate) linked: &'a [ArcherLinkedState],
    pub(crate) target: Option<ArcherTarget>,
    pub(crate) entities: &'a [ArcherTarget],
    pub(crate) fallback_entities: &'a [ArcherTarget],
    pub(crate) peers: &'a [ArcherTarget],
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
) -> Option<ArcherTarget> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            candidate.entity_index != source.source_entity_index
                && archer_target_is_live(*candidate)
                && archer_distance_squared_2d(source.position, candidate.position) <= radius_squared
                && (!require_cone || archer_within_cone(source, *candidate))
        })
        .min_by_key(|candidate| archer_distance_squared_2d(source.position, candidate.position))
}

fn archer_attack_visual(
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

fn archer_idle_visual(source: &mut Level2AuthoredClass5Source) -> ArcherVisualUpdate {
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
    if source.pending_word_0x62 != 0 {
        source.life = source.life.wrapping_sub(context.pending_damage);
        source.word_0x26 = source.pending_word_0x62;
        source.pending_word_0x62 = 0;
        context.pending_damage = 0;
        state = 1;
        consumed_pending_damage = true;
    } else {
        source.word_0x26 = 0;
    }
    let mut linked_entity_index = context.linked_head;
    while linked_entity_index != 0 {
        let Some(linked) = context
            .linked
            .iter()
            .find(|linked| linked.entity_index == linked_entity_index)
        else {
            break;
        };
        if linked.life < source.life {
            source.life = linked.life;
            source.word_0x26 = linked.current_attacker;
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
        effects.visual = Some(archer_attack_visual(
            source,
            context
                .target
                .filter(|target| target.entity_index == source.word_0x26),
        ));
        return effects;
    }
    if mailbox_state == 2 {
        source.action = ARCHER_ACTION_HIT;
        return effects;
    }

    crate::move_entity(
        &mut source.position,
        source.yaw,
        source.pitch,
        source.actual_speed,
    );
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
        let preferred = archer_nearest_candidate(source, context.entities, radius_squared, true)
            .filter(|candidate| {
                (candidate.model == 0 || candidate.model == 1) && candidate.word_0x248 != 0
            });
        let fallback = preferred.or_else(|| {
            archer_nearest_candidate(source, context.fallback_entities, radius_squared, false)
        });
        if let Some(target) = fallback {
            if !(target.class == 10 && target.model == 45) {
                source.word_0x96_150 = target.entity_index;
                source.action = ARCHER_ACTION_ATTACK;
                effects.selected_target = Some(target.entity_index);
                return effects;
            }
        }
        if let Some(peer) = archer_nearest_candidate(source, context.peers, radius_squared, true) {
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
    } else if mailbox_state == 2 {
        source.action = ARCHER_ACTION_HIT;
        return effects;
    } else {
        crate::move_entity(
            &mut source.position,
            source.yaw,
            source.pitch,
            source.actual_speed,
        );
        effects.moved = true;
        let Some(target) = context
            .target
            .filter(|target| target.entity_index == source.word_0x96_150)
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
                peer.entity_index != source.source_entity_index
                    && peer.class == ARCHER_SOURCE_CLASS
                    && peer.model == ARCHER_MODEL
                    && archer_target_is_live(*peer)
                    && peer.position.x.abs_diff(source.position.x) < 128
                    && peer.position.y.abs_diff(source.position.y) < 128
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
                parent_entity_id: source.source_entity_index,
                parent_word_0x96: source.word_0x96_150,
                parent_roll: source.roll,
                parent_position: source.position,
                parent_fov: source.fov,
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
    effects
}

/// Port `sub_1FFE0`/`sub_1C560` for the model-local fields.  The linked
/// relation lives in the caller's explicit `word_0x32` sidecar.
pub(crate) fn step_archer_action35(
    source: &mut Level2AuthoredClass5Source,
    word_0x32: &mut u16,
    context: &mut ArcherActionContext<'_>,
) -> ArcherActionEffects {
    debug_assert_eq!(source.model, ARCHER_MODEL);
    debug_assert_eq!(source.action, ARCHER_ACTION_LINK);
    let mut effects = ArcherActionEffects::default();
    if *word_0x32 == 0 {
        source.action = ARCHER_ACTION;
        return effects;
    }
    let linked_target = context
        .entities
        .iter()
        .copied()
        .find(|target| target.entity_index == *word_0x32);
    let Some(target) = linked_target.filter(|target| {
        archer_target_is_live(*target)
            && target.class == ARCHER_SOURCE_CLASS
            && target.model == ARCHER_MODEL
    }) else {
        *word_0x32 = 0;
        source.action = ARCHER_ACTION;
        return effects;
    };

    let (mailbox_state, consumed_pending_damage) = archer_consume_mailbox(source, context);
    effects.consumed_pending_damage = consumed_pending_damage;
    if mailbox_state == 1 {
        source.word_0x96_150 = source.word_0x26;
        *word_0x32 = 0;
        source.action = ARCHER_ACTION_ATTACK;
        effects.selected_target = Some(source.word_0x26);
        effects.visual = Some(archer_attack_visual(
            source,
            context
                .target
                .filter(|target| target.entity_index == source.word_0x26),
        ));
        return effects;
    }
    if mailbox_state == 2 {
        source.action = ARCHER_ACTION_HIT;
        return effects;
    }

    crate::move_entity(
        &mut source.position,
        source.yaw,
        source.pitch,
        source.actual_speed,
    );
    effects.moved = true;
    if source.source_byte_3e & 3 == 0 {
        source.roll = i16::try_from(heading_for_positions(source.position, target.position))
            .expect("Archer peer heading fits i16");
        for peer in context.peers.iter().copied().filter(|peer| {
            peer.entity_index != source.source_entity_index
                && peer.class == ARCHER_SOURCE_CLASS
                && peer.model == ARCHER_MODEL
                && archer_target_is_live(*peer)
                && peer.position.x.abs_diff(source.position.x) < 256
                && peer.position.y.abs_diff(source.position.y) < 256
        }) {
            source.roll = i16::try_from(heading_for_positions(peer.position, source.position))
                .expect("Archer peer heading fits i16");
        }
    }
    if !source
        .source_byte_3e
        .is_multiple_of(ARCHER_DESCRIPTOR_CADENCE)
    {
        return effects;
    }
    match target.action {
        ARCHER_ACTION => {}
        ARCHER_ACTION_ATTACK => {
            source.word_0x96_150 = target.word_0x96_150;
            *word_0x32 = 0;
            source.action = ARCHER_ACTION_ATTACK;
            effects.visual = Some(archer_attack_visual(source, Some(target)));
            return effects;
        }
        ARCHER_ACTION_LINK => {
            *word_0x32 = target.word_0x32;
            let heading_target = context
                .entities
                .iter()
                .copied()
                .find(|candidate| candidate.entity_index == *word_0x32)
                .filter(|candidate| archer_target_is_live(*candidate))
                .unwrap_or(target);
            source.roll = i16::try_from(heading_for_positions(
                heading_target.position,
                source.position,
            ))
            .expect("Archer peer heading fits i16");
            for peer in context.peers.iter().copied().filter(|peer| {
                peer.entity_index != source.source_entity_index
                    && peer.class == ARCHER_SOURCE_CLASS
                    && peer.model == ARCHER_MODEL
                    && archer_target_is_live(*peer)
                    && peer.position.x.abs_diff(source.position.x) < 256
                    && peer.position.y.abs_diff(source.position.y) < 256
            }) {
                source.roll = i16::try_from(heading_for_positions(peer.position, source.position))
                    .expect("Archer peer heading fits i16");
            }
        }
        ARCHER_ACTION_ATTACK_VISUAL => {
            source.word_0x96_150 = target.word_0x96_150;
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
    source.actual_speed = target.maximum_speed.wrapping_add(target.actual_speed);
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
        control_flags: 0,
        word_0x96: request.parent_word_0x96,
        xtype: request.target_class,
        xsubtype: request.target_model,
        yaw,
        pitch,
        roll: request.parent_roll,
        fov: ARCHER_CHILD_PARTICLE_FOV,
        position,
        // NewEvent seeds byte_0x3E from the fixed arena slot before the
        // ordinary callback epilogue advances it.
        source_byte_3e: child_source_entity_index as u8,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}

impl Level2EconomyState {
    /// Allocate the child source slot and map relation without inventing a
    /// second child arena.  The caller must retain the returned packet in the
    /// shared reset/hash/reclaim carrier.  Failure before the map link leaves
    /// the free stack, source RNG map, and child state unchanged.
    pub(crate) fn allocate_archer_child_packet(
        &mut self,
        request: ArcherChildRequest,
    ) -> Option<ArcherChildPacket> {
        let parent_index = self.authored_class5_archer.iter().position(|state| {
            state.source_entity_index == request.parent_entity_index
                && self.authored_class5.iter().any(|source| {
                    source.active && source.source_entity_index == state.source_entity_index
                })
        })?;
        self.commit_class9_model13_child(request, Some(parent_index), false)
    }

    /// Commit the same `IfSubtypeCallCreatingManaSphere(..., 9, 13)` product
    /// without the Archer-only word-0x34 chain writes. Castle auxiliary action
    /// 122 sets only id, aim, target, xtype/xsubtype and the raised Z position.
    pub(crate) fn allocate_castle_auxiliary_projectile_packet(
        &mut self,
        request: ArcherChildRequest,
    ) -> Option<ArcherChildPacket> {
        self.commit_class9_model13_child(request, None, true)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherStepResult {
    pub(crate) effects: ArcherActionEffects,
    pub(crate) target: Option<ArcherTarget>,
    pub(crate) map_synced: bool,
}

impl Level2EconomyState {
    /// Dispatch one retained model-4 root from the ordinary dynamic cursor.
    /// The outer reset session supplies imported entity views; all action
    /// state, child allocation, source cadence, map projection, and rollback
    /// remain in this shared economy carrier.
    pub(crate) fn step_authored_archer_slot(
        &mut self,
        source_slot: u16,
        mut context: ArcherActionContext<'_>,
        terrain: &SessionTerrain,
    ) -> Option<ArcherStepResult> {
        let source_index = self
            .authored_class5
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)?;
        let archer_index = self
            .authored_class5_archer
            .iter()
            .position(|state| state.source_entity_index == source_slot)?;
        let state_before = self.clone();
        let mut source = self.authored_class5[source_index];
        let mut archer_state = self.authored_class5_archer[archer_index];
        let mut word_0x32 = archer_state.word_0x32;
        context.linked_head = archer_state.word_0x34;

        let effects = match source.action {
            ARCHER_ACTION => step_archer_action33(&mut source, &mut word_0x32, &mut context),
            ARCHER_ACTION_ATTACK => step_archer_action34(&mut source, &mut context),
            ARCHER_ACTION_LINK => step_archer_action35(&mut source, &mut word_0x32, &mut context),
            ARCHER_ACTION_HIT => {
                step_archer_action36(&mut source);
                if source.action == ARCHER_ACTION_KILL {
                    self.prekill_archer_chain(&mut source, archer_state.word_0x34);
                }
                ArcherActionEffects::default()
            }
            ARCHER_ACTION_KILL => {
                // `KillArcher_201040` calls KillEntity only on the phase-zero
                // cadence. Visits at phases 1..7 retain the row and consume no
                // allocation. The common UpdateEntities epilogue below then
                // advances byte 0x3E exactly once.
                if source.source_byte_3e & 7 == 0 {
                    let owner_entity_index = self.source_owner(source.source_entity_index);
                    if source.mana > 0 {
                        // KillEntity_1C930 advances the dying row's RNG once
                        // before TransformEntityToManaSphere_36BA0 consumes
                        // the resulting stream value.
                        source.entity_rng = level2_next_rng16(source.entity_rng);
                    }
                    let _ = self.spawn_mana_sphere_from_death(
                        source.source_entity_index,
                        source.position,
                        source.yaw,
                        source.entity_rng,
                        source.mana,
                        owner_entity_index,
                        terrain,
                    );
                    let _ = self.spawn_death_burst_from_death_with_id(
                        source.source_entity_index,
                        source.source_entity_index,
                        source.position,
                    );
                    source.drawing_enabled = false;
                }
                ArcherActionEffects::default()
            }
            ARCHER_ACTION_ATTACK_VISUAL => ArcherActionEffects {
                visual: Some(archer_attack_visual(&mut source, context.target)),
                ..ArcherActionEffects::default()
            },
            ARCHER_ACTION_IDLE_VISUAL => ArcherActionEffects {
                visual: Some(archer_idle_visual(&mut source)),
                ..ArcherActionEffects::default()
            },
            _ => ArcherActionEffects::default(),
        };

        if let Some(request) = effects.child_request {
            let _ = self.allocate_archer_child_packet(request);
            archer_state.word_0x34 = self.authored_class5_archer[archer_index].word_0x34;
        }
        if effects.moved {
            source.position.z = terrain.ground_height(source.position);
        }
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        archer_state.word_0x32 = word_0x32;
        archer_state.pending_damage = context.pending_damage;
        self.authored_class5[source_index] = source;
        self.authored_class5_archer[archer_index] = archer_state;
        self.source_rng.insert(source_slot, source.entity_rng);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_err()
        {
            *self = state_before;
            return None;
        }
        Some(ArcherStepResult {
            effects,
            target: context.target,
            map_synced: true,
        })
    }

    fn prekill_archer_chain(&mut self, source: &mut Level2AuthoredClass5Source, chain_head: u16) {
        let mut current = chain_head;
        let mut visits = 0_usize;
        while current != 0 && visits <= self.authored_class5_archer_children.len() {
            let Some(child_index) = self
                .authored_class5_archer_children
                .iter()
                .position(|child| child.active && child.source_entity_index == current)
            else {
                break;
            };
            let child = &mut self.authored_class5_archer_children[child_index];
            child.action = ARCHER_ACTION_KILL;
            if child.word_0x24 != 0 {
                source.word_0x24 = child.word_0x24;
            }
            current = child.word_0x34;
            visits = visits.wrapping_add(1);
        }
    }

    /// Visit a retained class-9/model-13 child from the same live source
    /// cursor. Its source movement uses the constructor's signed pitch and
    /// the shared terrain/map projection. Action 37 is not a class-9 table
    /// callback and is handled by the scheduler's invalid-selector branch.
    pub(crate) fn step_authored_class5_archer_child_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(child_index) = self
            .authored_class5_archer_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return false;
        };
        let state_before = self.clone();
        let mut child = self.authored_class5_archer_children[child_index];
        if child.action == ARCHER_CHILD_ACTION {
            crate::move_entity(
                &mut child.position,
                child.yaw,
                child.pitch,
                child.actual_speed,
            );
            child.position.z = terrain.ground_height(child.position);
        }
        child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
        child.phase = child.phase.wrapping_add(1);
        self.authored_class5_archer_children[child_index] = child;
        self.source_rng.insert(source_slot, child.entity_rng);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_err()
        {
            *self = state_before;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::level_two_fixture_path;
    use super::*;

    fn archer_position() -> Position {
        Position {
            x: 0x2800,
            y: 0x3100,
            z: 0,
        }
    }

    fn empty_context<'a>() -> ArcherActionContext<'a> {
        ArcherActionContext {
            pending_damage: 0,
            linked_head: 0,
            linked: &[],
            target: None,
            entities: &[],
            fallback_entities: &[],
            peers: &[],
        }
    }

    #[test]
    fn archer_rows_are_the_complete_group20_source_order() {
        assert_eq!(ARCHER_AUTHORED_ROWS, [232, 233, 234, 235]);
        assert!(
            ARCHER_AUTHORED_ROWS
                .windows(2)
                .all(|rows| rows[1] == rows[0].wrapping_add(1))
        );
        assert!(
            ARCHER_AUTHORED_ROWS
                .iter()
                .copied()
                .all(is_authored_archer_row)
        );
        assert!(!is_authored_archer_row(231));
        assert!(!is_authored_archer_row(236));
    }

    #[test]
    fn archer_constructor_payload_preserves_pose_rng_and_descriptor_cadence() {
        let slot = 147_u16;
        let initial_rng = 0x1357_u16;
        let entity_rng = initial_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let payload = archer_constructor_payload(slot, initial_rng, 232, 3, archer_position());
        assert_eq!(payload.entity_rng, entity_rng);
        assert_eq!(payload.initial_dword_0x10, 47);
        assert_eq!(payload.dword_0x10, 47);
        assert_eq!(payload.source_class, ARCHER_SOURCE_CLASS);
        assert_eq!(payload.model, ARCHER_MODEL);
        assert_eq!(payload.action, ARCHER_ACTION);
        assert_eq!(
            (
                payload.minimum_speed,
                payload.maximum_speed,
                payload.actual_speed,
                payload.max_life,
                payload.life,
                payload.mana,
                payload.max_mana,
                payload.subspell_index,
            ),
            (30, 0, 30, 1_000, 1_000, 500, 0, 500)
        );
        assert_eq!(payload.byte_0x38, 1);
        assert_eq!(payload.descriptor_index, 75);
        assert_eq!(payload.source_byte_39, 31);
        assert_eq!(payload.xtype, 3);
        assert_eq!(payload.source_byte_3e, 3);
        assert_eq!(payload.particle_index, 0);
        assert_eq!(
            (payload.yaw, payload.pitch, payload.roll, payload.fov),
            (125, 66, 66, 125)
        );
        assert_eq!((payload.shift_rot_x, payload.shift_rot_y), (128, 256));
        assert!(payload.active && payload.drawing_enabled);
    }

    #[test]
    fn archer_action_packets_and_allocator_branches() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Archer model-local session");
        let baseline_hash = session.campaign_state_hash();

        let mut rows = Vec::new();
        for authored_index in ARCHER_AUTHORED_ROWS {
            rows.push(
                session
                    .economy_mut()
                    .allocate_authored_archer(authored_index, archer_position())
                    .expect("every authored Archer row allocates"),
            );
        }
        assert_eq!(
            rows.iter()
                .map(|source| source.authored_index)
                .collect::<Vec<_>>(),
            ARCHER_AUTHORED_ROWS
        );
        assert!(rows.iter().all(|source| {
            source.model == ARCHER_MODEL
                && source.action == ARCHER_ACTION
                && source.minimum_speed == ARCHER_MINIMUM_SPEED
                && source.maximum_speed == ARCHER_MAXIMUM_SPEED
                && source.actual_speed == ARCHER_MINIMUM_SPEED
                && source.max_life == ARCHER_MAX_LIFE
                && source.life == ARCHER_MAX_LIFE
                && source.mana == ARCHER_MANA
                && source.max_mana == ARCHER_MAX_MANA
                && source.subspell_index == ARCHER_SUBSPELL_INDEX
                && source.particle_index == ARCHER_PARTICLE_INDEX
                && source.yaw == ARCHER_POSE_YAW
                && source.pitch == ARCHER_POSE_PITCH
                && source.roll == ARCHER_POSE_ROLL
                && source.fov == ARCHER_POSE_FOV
                && source.shift_rot_x == ARCHER_SHIFT_ROT_X
                && source.shift_rot_y == ARCHER_SHIFT_ROT_Y
                && source.active
                && source.drawing_enabled
        }));
        assert_eq!(session.economy().authored_class5_runtime().len(), 4);
        assert!(
            session
                .economy()
                .authored_class5_runtime()
                .iter()
                .all(|state| {
                    state.byte_0x38 == 1
                        && state.xtype == 3
                        && state.descriptor_index == ARCHER_DESCRIPTOR_INDEX
                        && state.pending_damage == 0
                        && state.linked_head == 0
                })
        );

        let no_slot_phase = session.economy().class5_model4_constructor_phase;
        let no_slot_rng = session.economy().source_rng.clone();
        session.economy_mut().free_slots.clear();
        assert!(
            session
                .economy_mut()
                .allocate_authored_archer(235, archer_position())
                .is_none()
        );
        assert_eq!(
            session.economy().class5_model4_constructor_phase,
            no_slot_phase
        );
        assert_eq!(session.economy().source_rng, no_slot_rng);

        let mut source = rows[0];
        let source_before = source;
        source.source_byte_3e = 1;
        let mut word_0x32 = 0;
        let mut context = empty_context();
        let effects = step_archer_action33(&mut source, &mut word_0x32, &mut context);
        assert!(effects.moved);
        assert_eq!(source.action, ARCHER_ACTION);
        assert_eq!(source.entity_rng, source_before.entity_rng);
        assert_eq!(source.roll, source_before.roll);
        assert_eq!(word_0x32, 0);

        let target = ArcherTarget {
            entity_index: 777,
            class: 3,
            model: 0,
            action: 0,
            active: true,
            life: 1_000,
            control_flags: 0,
            minimum_speed: 0,
            maximum_speed: 0,
            actual_speed: 0,
            dword_0x10: 0,
            word_0x248: 0,
            word_0x32: 0,
            word_0x96_150: 0,
            position: source.position,
        };
        let mut attack_source = source_before;
        attack_source.action = ARCHER_ACTION_ATTACK;
        attack_source.source_byte_3e = ARCHER_DESCRIPTOR_CADENCE;
        attack_source.word_0x96_150 = target.entity_index;
        let mut attack_context = ArcherActionContext {
            pending_damage: 0,
            linked_head: 0,
            linked: &[],
            target: Some(target),
            entities: &[],
            fallback_entities: &[],
            peers: &[],
        };
        let attack_effects = step_archer_action34(&mut attack_source, &mut attack_context);
        let request = attack_effects
            .child_request
            .expect("action-34 cadence emits Archer child request");
        assert_eq!(request.target_entity_index, target.entity_index);
        assert_eq!(request.target_class, target.class);
        assert_eq!(request.target_model, target.model);
        assert_eq!(
            attack_effects.target_feedback_entity,
            Some(target.entity_index)
        );

        let child = archer_child_packet(900, 0x2468, request);
        assert_eq!(child.entity_id, attack_source.source_entity_index);
        assert_eq!(child.parent_entity_index, attack_source.source_entity_index);
        assert_eq!(child.entity_rng, 0x2468);
        assert_eq!(
            (
                child.source_class,
                child.model,
                child.action,
                child.minimum_speed,
                child.actual_speed,
                child.max_life,
                child.life,
                child.subspell_index,
                child.particle_index,
            ),
            (9, 13, 13, 384, 384, 13, 13, 250, 195)
        );
        assert_eq!(child.word_0x96, request.parent_word_0x96);
        assert_eq!((child.xtype, child.xsubtype), (target.class, target.model));
        assert_eq!(child.fov, ARCHER_CHILD_PARTICLE_FOV);
        assert_eq!(
            child.position.z,
            attack_source.position.z + attack_source.fov / 2
        );
        assert!(child.active && child.drawing_enabled);

        // A full source arena must refuse the child before touching source
        // RNG/map state. The shared carrier remains the integration owner.
        let child_no_slot_rng = session.economy().source_rng.clone();
        assert!(
            session
                .economy_mut()
                .allocate_archer_child_packet(request)
                .is_none()
        );
        assert_eq!(session.economy().source_rng, child_no_slot_rng);

        session.reset();
        assert!(session.economy().authored_class5().is_empty());
        assert_eq!(session.campaign_state_hash(), baseline_hash);
    }
}
