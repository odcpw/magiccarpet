//! The reachable Level 2 class-5/model-14 Trader family.
//!
//! `AddTrader_4C0B0` and `sub_237B0` are deliberately kept apart from the
//! other class-5 rows.  The Trader constructor has a different source packet
//! (notably its model counter, descriptor, and particle row), and action 113
//! owns a real mailbox/target/building decision closure rather than the
//! creature callbacks used by models 3 and 9.

use super::super::*;

/// The group-3 Trader rows, in the order in which the Level 2 dispatch
/// artifact presents them.
pub(crate) const TRADER_AUTHORED_ROWS: [u16; 27] = [
    125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
];

pub(crate) const TRADER_CLASS: u8 = 5;
pub(crate) const TRADER_MODEL: u8 = 14;
pub(crate) const TRADER_ACTION: u8 = 113;
pub(crate) const TRADER_HIT_ACTION: u8 = 118;
pub(crate) const TRADER_PRE_KILL_ACTION: u8 = 116;
pub(crate) const TRADER_MINIMUM_SPEED: i16 = 54;
pub(crate) const TRADER_MAXIMUM_SPEED: i16 = 18;
pub(crate) const TRADER_MAX_LIFE: i32 = 1_000;
pub(crate) const TRADER_MANA: i32 = 0;
pub(crate) const TRADER_MAX_MANA: i32 = 0;
pub(crate) const TRADER_SUBSPELL_INDEX: u16 = 500;
pub(crate) const TRADER_DESCRIPTOR_INDEX: u16 = 100;
pub(crate) const TRADER_DESCRIPTOR_CADENCE: u8 = 40;
pub(crate) const TRADER_BYTE_0X38: u8 = 1;
pub(crate) const TRADER_XTYPE: u8 = 3;
pub(crate) const TRADER_SOURCE_BYTE_39: u8 = 64;
pub(crate) const TRADER_PARTICLE_INDEX: u16 = 219;
pub(crate) const TRADER_SHIFT_ROT_X: i16 = 128;
pub(crate) const TRADER_SHIFT_ROT_Y: i16 = 128;
pub(crate) const TRADER_BUILDING_DISTANCE_MIN: u32 = 0x0e10_0000;
pub(crate) const TRADER_TARGET_RANGE: u32 = 0x0800;

#[must_use]
pub(crate) fn is_authored_trader_row(authored_index: u16) -> bool {
    TRADER_AUTHORED_ROWS.contains(&authored_index)
}

/// The exact constructor packet before it is projected into the existing
/// class-5 arena carrier.  The carrier predates this family and does not yet
/// expose `byte_0x38`, `xtype`, the descriptor pointer, or the first value
/// written to `dword_0x10`; retaining them here prevents the integration seam
/// from silently dropping source state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderConstructorPayload {
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

/// Reproduce the ordered `AddTrader_4C0B0` writes after `NewEvent_4A050`.
/// `initial_rng` is the value installed by `NewEvent`; this function performs
/// the constructor's one LCG draw and no other random operation.
#[must_use]
pub(crate) fn trader_constructor_payload(
    source_entity_index: u16,
    initial_rng: u16,
    authored_index: u16,
    source_byte_3e: u8,
    position: Position,
) -> TraderConstructorPayload {
    let entity_rng = level2_next_rng16(initial_rng);
    let angle = i16::try_from(entity_rng & 0x07ff).expect("Trader angle fits") - 1;
    TraderConstructorPayload {
        source_entity_index,
        entity_rng,
        authored_index,
        source_class: TRADER_CLASS,
        model: TRADER_MODEL,
        action: TRADER_ACTION,
        minimum_speed: TRADER_MINIMUM_SPEED,
        maximum_speed: TRADER_MAXIMUM_SPEED,
        actual_speed: TRADER_MAXIMUM_SPEED,
        max_life: TRADER_MAX_LIFE,
        life: TRADER_MAX_LIFE,
        mana: TRADER_MANA,
        max_mana: TRADER_MAX_MANA,
        subspell_index: TRADER_SUBSPELL_INDEX,
        initial_dword_0x10: i32::from(source_entity_index % 100),
        dword_0x10: 2,
        byte_0x38: TRADER_BYTE_0X38,
        descriptor_index: TRADER_DESCRIPTOR_INDEX,
        source_byte_39: TRADER_SOURCE_BYTE_39,
        xtype: TRADER_XTYPE,
        source_byte_3e,
        particle_index: TRADER_PARTICLE_INDEX,
        yaw: angle,
        pitch: angle,
        roll: angle,
        fov: 0,
        shift_rot_x: TRADER_SHIFT_ROT_X,
        shift_rot_y: TRADER_SHIFT_ROT_Y,
        position,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}

impl TraderConstructorPayload {
    /// Project the source packet into the carrier fields that existed before
    /// the Trader slice.  The full payload remains available to the caller so
    /// the serialized integration can extend that carrier without changing
    /// constructor order or values.
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
    /// Allocate one authored Trader using the real source free stack, event
    /// RNG, model-14 counter, terrain/map projection, and reset-owned source
    /// arena.  The caller owns the surrounding group transaction.
    pub(crate) fn allocate_authored_trader(
        &mut self,
        authored_index: u16,
        position: Position,
    ) -> Option<Level2AuthoredClass5Source> {
        if !is_authored_trader_row(authored_index) {
            return None;
        }
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let source_byte_3e = self.class5_model14_constructor_phase;
        self.class5_model14_constructor_phase = source_byte_3e.wrapping_add(1);

        let mut position = position;
        position.z = self.terrain.ground_height(position);
        let payload = trader_constructor_payload(
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
            self.release_slot(source_entity_index, None);
            return None;
        }
        let source = payload.into_class5_source();
        self.source_rng
            .insert(source_entity_index, source.entity_rng);
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
        Some(source)
    }

    /// Advance one live Trader through action 113 and its direct source
    /// closure.  The context is deliberately explicit: the current generic
    /// class-5 carrier does not own the Trader mailbox damage amount, linked
    /// child chain, target `dword_0x10`, or ordered building candidates.
    pub(crate) fn step_authored_trader_slot(
        &mut self,
        source_slot: u16,
        target: Option<TraderTarget>,
        pending_damage: i32,
        linked_head: u16,
        linked: &[TraderLinkedState],
        buildings: &[TraderBuildingCandidate],
    ) -> Option<TraderAction113Step> {
        let source_index = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == TRADER_MODEL
                && source.action == TRADER_ACTION
        })?;
        let mut source = self.authored_class5[source_index];
        let consumed_mailbox = source.pending_word_0x62 != 0;
        let mut context = TraderAction113Context {
            pending_damage,
            linked_head,
            linked,
            target,
            buildings,
        };
        let effects = step_trader_action113(&mut source, &mut context);
        if effects.moved {
            source.position.z = self.terrain.ground_height(source.position);
        }
        if consumed_mailbox && let Some(runtime) = self.authored_class5_runtime_mut(source_slot) {
            runtime.pending_damage = 0;
        }
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        let map_synced = self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok();
        Some(TraderAction113Step {
            effects,
            target: context.target,
            map_synced,
        })
    }
}

/// A linked class-5 child as observed by `sub_237B0`'s `word_0x34` walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderLinkedState {
    pub(crate) entity_index: u16,
    pub(crate) next_entity_index: u16,
    pub(crate) life: i32,
    pub(crate) current_attacker: u16,
    pub(crate) model: u8,
}

/// The target fields directly read by action 113.  `dword_0x10` is included
/// because the close class-10/model-45 branch mutates the target before the
/// source callback returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderTarget {
    pub(crate) entity_index: u16,
    pub(crate) class: u8,
    pub(crate) model: u8,
    pub(crate) active: bool,
    pub(crate) minimum_speed: i16,
    pub(crate) dword_0x10: i32,
    pub(crate) position: Position,
}

/// An ordered source candidate from the live building map walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderBuildingCandidate {
    pub(crate) entity_index: u16,
    pub(crate) position: Position,
    pub(crate) eligible: bool,
}

pub(crate) struct TraderAction113Context<'a> {
    pub(crate) pending_damage: i32,
    pub(crate) linked_head: u16,
    pub(crate) linked: &'a [TraderLinkedState],
    pub(crate) target: Option<TraderTarget>,
    pub(crate) buildings: &'a [TraderBuildingCandidate],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderAction113Effects {
    pub(crate) player_feedback_entity: Option<u16>,
    pub(crate) selected_target: Option<u16>,
    pub(crate) released_target: bool,
    pub(crate) moved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraderAction113Step {
    pub(crate) effects: TraderAction113Effects,
    pub(crate) target: Option<TraderTarget>,
    pub(crate) map_synced: bool,
}

/// Exact source action 113/helper closure for a model-14 Trader.
pub(crate) fn step_trader_action113(
    source: &mut Level2AuthoredClass5Source,
    context: &mut TraderAction113Context<'_>,
) -> TraderAction113Effects {
    debug_assert_eq!(source.model, TRADER_MODEL);
    debug_assert_eq!(source.action, TRADER_ACTION);

    let mut effects = TraderAction113Effects {
        player_feedback_entity: None,
        selected_target: None,
        released_target: false,
        moved: false,
    };

    let mut mailbox_state = 0_u8;
    if source.pending_word_0x62 != 0 {
        source.life = source.life.wrapping_sub(context.pending_damage);
        let pending_target = source.pending_word_0x62;
        source.pending_word_0x62 = 0;
        source.word_0x26 = pending_target;
        mailbox_state = 1;
    } else {
        source.word_0x26 = 0;
    }

    let mut linked_entity_index = context.linked_head;
    while linked_entity_index != 0 {
        let Some(linked_entity) = context
            .linked
            .iter()
            .find(|linked_entity| linked_entity.entity_index == linked_entity_index)
        else {
            break;
        };
        if linked_entity.life < source.life {
            source.life = linked_entity.life;
            source.word_0x26 = linked_entity.current_attacker;
            mailbox_state = 1;
            break;
        }
        linked_entity_index = linked_entity.next_entity_index;
    }

    if source.life < 0 {
        source.word_0x24 = source.word_0x26;
        mailbox_state = 2;
    }

    if mailbox_state >= 1 {
        if mailbox_state == 1 {
            source.word_0x96_150 = source.word_0x26;
            source.action = TRADER_HIT_ACTION;
            effects.selected_target = Some(source.word_0x26);
            if context
                .target
                .is_some_and(|target| target.entity_index == source.word_0x26 && target.model <= 1)
                || context.linked.iter().any(|linked_entity| {
                    linked_entity.entity_index == source.word_0x26 && linked_entity.model <= 1
                })
            {
                effects.player_feedback_entity = Some(source.word_0x26);
            }
        } else {
            source.action = TRADER_PRE_KILL_ACTION;
        }
        if source.action == TRADER_HIT_ACTION {
            source.actual_speed = source.minimum_speed;
        }
        return effects;
    }

    // `sub_1B8C0` is the common source movement helper reached before the
    // descriptor cadence gate.  The Level 2 terrain seam supplies its
    // ground result; the integer movement primitive preserves wrapping.
    crate::move_entity(&mut source.position, source.yaw, 0, source.actual_speed);
    effects.moved = true;

    if !source
        .source_byte_3e
        .is_multiple_of(TRADER_DESCRIPTOR_CADENCE)
    {
        return effects;
    }

    if source.word_0x96_150 != 0 {
        if let Some(mut target) = context
            .target
            .filter(|target| target.entity_index == source.word_0x96_150)
        {
            if target.active && target.class == 10 && target.model == 45 {
                let distance = level2_trader_distance_3d(source.position, target.position);
                if distance > TRADER_TARGET_RANGE {
                    source.roll =
                        i16::try_from(heading_for_positions(source.position, target.position))
                            .expect("Trader target heading fits i16");
                    context.target = Some(target);
                    return effects;
                }
                if i32::from(target.minimum_speed) > target.dword_0x10 {
                    source.dword_0x10 = 1;
                    source.action = TRADER_PRE_KILL_ACTION;
                    target.dword_0x10 = target.dword_0x10.wrapping_add(1);
                    context.target = Some(target);
                    return effects;
                }
            }
        }
        source.word_0x96_150 = 0;
        effects.released_target = true;
        source.actual_speed = source.maximum_speed;
        return effects;
    }

    let first_rng = level2_next_rng16(source.entity_rng);
    source.entity_rng = first_rng;
    let second_rng = level2_next_rng16(source.entity_rng);
    source.entity_rng = second_rng;
    let direction = 2 * (i32::from(first_rng % 0x009d) / 79) - 1;
    let turn = (i32::from(second_rng & 0x00ff) + 85) * direction;
    source.roll = i16::try_from((i32::from(source.roll).wrapping_add(turn)) & 0x07ff)
        .expect("Trader roll fits 11 bits");

    let mut nearest = u32::MAX;
    let mut nearest_entity = None;
    for candidate in context
        .buildings
        .iter()
        .filter(|candidate| candidate.eligible)
    {
        let dx = i32::from(i16::from_le_bytes(
            candidate
                .position
                .x
                .wrapping_sub(source.position.x)
                .to_le_bytes(),
        ));
        let dy = i32::from(i16::from_le_bytes(
            candidate
                .position
                .y
                .wrapping_sub(source.position.y)
                .to_le_bytes(),
        ));
        let distance = dx
            .wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .cast_unsigned();
        if distance < nearest && distance > TRADER_BUILDING_DISTANCE_MIN {
            nearest = distance;
            nearest_entity = Some(candidate.entity_index);
        }
    }
    if let Some(entity_index) = nearest_entity {
        source.word_0x96_150 = entity_index;
        source.actual_speed = source.maximum_speed.wrapping_add(12);
        effects.selected_target = Some(entity_index);
    }
    effects
}

fn level2_trader_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .wrapping_add(dy.wrapping_mul(dy))
            .wrapping_add(dz.wrapping_mul(dz))
            .cast_unsigned(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::level_two_fixture_path;
    use super::*;

    fn trader_position() -> Position {
        Position {
            x: 0x2800,
            y: 0x3100,
            z: 0,
        }
    }

    fn empty_context<'a>() -> TraderAction113Context<'a> {
        TraderAction113Context {
            pending_damage: 0,
            linked_head: 0,
            linked: &[],
            target: None,
            buildings: &[],
        }
    }

    #[test]
    fn trader_rows_are_the_complete_group3_source_order() {
        assert_eq!(TRADER_AUTHORED_ROWS.len(), 27);
        assert_eq!(TRADER_AUTHORED_ROWS[0], 125);
        assert_eq!(TRADER_AUTHORED_ROWS[26], 151);
        assert!(
            TRADER_AUTHORED_ROWS
                .windows(2)
                .all(|rows| rows[1] == rows[0].wrapping_add(1))
        );
        assert!(
            TRADER_AUTHORED_ROWS
                .iter()
                .copied()
                .all(is_authored_trader_row)
        );
        assert!(!is_authored_trader_row(124));
        assert!(!is_authored_trader_row(152));
    }

    #[test]
    fn trader_constructor_payload_is_source_exact_and_rng_discriminating() {
        let slot = 147_u16;
        let initial_rng = 0x1357_u16;
        let entity_rng = initial_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let angle = i16::try_from(entity_rng & 0x07ff).expect("angle fits") - 1;
        let payload = trader_constructor_payload(slot, initial_rng, 125, 3, trader_position());

        assert_eq!(payload.entity_rng, entity_rng);
        assert_eq!(payload.initial_dword_0x10, 47);
        assert_eq!(payload.dword_0x10, 2);
        assert_eq!(payload.source_class, 5);
        assert_eq!(payload.model, 14);
        assert_eq!(payload.action, 113);
        assert_eq!(payload.minimum_speed, 54);
        assert_eq!(payload.maximum_speed, 18);
        assert_eq!(payload.actual_speed, 18);
        assert_eq!(payload.max_life, 1_000);
        assert_eq!(payload.life, 1_000);
        assert_eq!(payload.mana, 0);
        assert_eq!(payload.max_mana, 0);
        assert_eq!(payload.subspell_index, 500);
        assert_eq!(payload.byte_0x38, 1);
        assert_eq!(payload.descriptor_index, 100);
        assert_eq!(payload.source_byte_39, 64);
        assert_eq!(payload.xtype, 3);
        assert_eq!(payload.source_byte_3e, 3);
        assert_eq!(payload.particle_index, 219);
        assert_eq!(
            (payload.yaw, payload.pitch, payload.roll),
            (angle, angle, angle)
        );
        assert_eq!(payload.fov, 0);
        assert_eq!((payload.shift_rot_x, payload.shift_rot_y), (128, 128));
        assert!(payload.active && payload.drawing_enabled);
    }

    #[test]
    fn level_two_trader_group3_constructor_and_action113_are_exact() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Trader model-local session");
        let baseline_hash = session.campaign_state_hash();

        let mut rows = Vec::new();
        for authored_index in TRADER_AUTHORED_ROWS {
            rows.push(
                session
                    .economy_mut()
                    .allocate_authored_trader(authored_index, trader_position())
                    .expect("every authored Trader row allocates"),
            );
        }
        assert_eq!(
            rows.iter()
                .map(|source| source.authored_index)
                .collect::<Vec<_>>(),
            TRADER_AUTHORED_ROWS
        );
        assert!(rows.iter().all(|source| {
            source.model == TRADER_MODEL
                && source.action == TRADER_ACTION
                && source.minimum_speed == TRADER_MINIMUM_SPEED
                && source.maximum_speed == TRADER_MAXIMUM_SPEED
                && source.actual_speed == TRADER_MAXIMUM_SPEED
                && source.max_life == TRADER_MAX_LIFE
                && source.life == TRADER_MAX_LIFE
                && source.mana == TRADER_MANA
                && source.max_mana == TRADER_MAX_MANA
                && source.subspell_index == TRADER_SUBSPELL_INDEX
                && source.source_byte_39 == TRADER_SOURCE_BYTE_39
                && source.particle_index == TRADER_PARTICLE_INDEX
                && source.shift_rot_x == TRADER_SHIFT_ROT_X
                && source.shift_rot_y == TRADER_SHIFT_ROT_Y
                && source.active
                && source.drawing_enabled
        }));
        assert_eq!(session.economy().authored_class5_runtime().len(), 27);
        assert!(
            session
                .economy()
                .authored_class5_runtime()
                .iter()
                .all(|state| {
                    state.byte_0x38 == TRADER_BYTE_0X38
                        && state.xtype == TRADER_XTYPE
                        && state.descriptor_index == TRADER_DESCRIPTOR_INDEX
                        && state.pending_damage == 0
                        && state.linked_head == 0
                })
        );

        let no_slot_phase = session.economy().class5_model14_constructor_phase;
        let no_slot_rng = session.economy().source_rng.clone();
        session.economy_mut().free_slots.clear();
        assert!(
            session
                .economy_mut()
                .allocate_authored_trader(151, trader_position())
                .is_none()
        );
        assert!(session.economy().authored_class5().len() >= 27);
        assert!(session.economy().free_slots.is_empty());
        assert_eq!(
            session.economy().class5_model14_constructor_phase,
            no_slot_phase
        );
        assert_eq!(session.economy().source_rng, no_slot_rng);

        let mut source = rows[0];
        let source_before = source;
        let first_rng = source_before
            .entity_rng
            .wrapping_mul(9_377)
            .wrapping_add(9_439);
        let second_rng = first_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let expected_roll = i16::try_from(
            (i32::from(source_before.roll)
                + (i32::from(second_rng & 0xff) + 85)
                    * (2 * (i32::from(first_rng % 0x009d) / 79) - 1))
                & 0x07ff,
        )
        .expect("roll fits");
        let mut context = empty_context();
        let effects = step_trader_action113(&mut source, &mut context);
        assert!(effects.moved);
        assert_eq!(source.action, TRADER_ACTION);
        assert_eq!(source.word_0x96_150, 0);
        assert_eq!(source.actual_speed, TRADER_MAXIMUM_SPEED);
        assert_eq!(source.entity_rng, second_rng);
        assert_eq!(source.roll, expected_roll);
        assert!(effects.selected_target.is_none());
        assert!(!effects.released_target);

        let mut replay = source_before;
        let mut replay_context = empty_context();
        let replay_effects = step_trader_action113(&mut replay, &mut replay_context);
        assert_eq!(source, replay);
        assert_eq!(effects, replay_effects);

        let attacker = TraderTarget {
            entity_index: 777,
            class: 3,
            model: 0,
            active: true,
            minimum_speed: 0,
            dword_0x10: 0,
            position: source_before.position,
        };
        let mut hit_source = source_before;
        hit_source.pending_word_0x62 = attacker.entity_index;
        let mut hit_context = TraderAction113Context {
            pending_damage: 100,
            linked_head: 0,
            linked: &[],
            target: Some(attacker),
            buildings: &[],
        };
        let hit_effects = step_trader_action113(&mut hit_source, &mut hit_context);
        assert_eq!(hit_source.life, 900);
        assert_eq!(hit_source.pending_word_0x62, 0);
        assert_eq!(hit_source.word_0x26, attacker.entity_index);
        assert_eq!(hit_source.word_0x96_150, attacker.entity_index);
        assert_eq!(hit_source.action, TRADER_HIT_ACTION);
        assert_eq!(hit_source.actual_speed, TRADER_MINIMUM_SPEED);
        assert_eq!(
            hit_effects.player_feedback_entity,
            Some(attacker.entity_index)
        );

        // Bind the mailbox effect through the real dynamic-arena dispatcher:
        // the Trader consumes its sidecar damage and the existing Nyphur
        // authority receives the source player-feedback word.
        let trader_slot = rows[0].source_entity_index;
        let player_slot = session.nyphur_dispatcher.common_upkeep(0).source_slot;
        session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == trader_slot)
            .expect("production Trader row")
            .pending_word_0x62 = player_slot;
        session
            .economy_mut()
            .authored_class5_runtime_mut(trader_slot)
            .expect("production Trader sidecar")
            .pending_damage = 100;
        assert!(session.step_level2_dynamic_arena(
            0,
            trader_slot,
            Level2DynamicArenaKind::AuthoredClass5,
        ));
        let production_after = session
            .economy()
            .authored_class5()
            .iter()
            .find(|candidate| candidate.source_entity_index == trader_slot)
            .copied()
            .expect("production Trader remains live after contact");
        assert_eq!(production_after.life, 900);
        assert_eq!(production_after.action, TRADER_HIT_ACTION);
        assert_eq!(production_after.word_0x96_150, player_slot);
        assert_eq!(
            session
                .economy()
                .authored_class5_runtime()
                .iter()
                .find(|state| state.source_entity_index == trader_slot)
                .expect("production Trader sidecar remains keyed")
                .pending_damage,
            0
        );
        assert_eq!(session.nyphur_dispatcher.common_upkeep(0).word_0x248, 200);

        session.reset();
        assert_eq!(session.campaign_state_hash(), baseline_hash);
    }
}
