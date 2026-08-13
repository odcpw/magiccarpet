//! Complete reachable class-5/model-3 constructor and action family.

use super::super::*;

const MODEL3_DESCRIPTOR_INDEX: u16 = 74;
const MODEL3_DESCRIPTOR_FLAGS: u8 = 8;
const MODEL3_DESCRIPTOR_CADENCE: u8 = 30;
const MODEL3_TARGET_RANGE: u32 = 0x1400;
const MODEL3_TARGET_HALF_ANGLE: i16 = 0x02aa;
const MODEL3_CHILD_COUNT: usize = 16;
const MODEL3_CHILD_PARTICLES: [u16; MODEL3_CHILD_COUNT] = [
    89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104,
];
const MODEL3_CHILD_SPEEDS: [i16; MODEL3_CHILD_COUNT] = [
    184, 203, 227, 246, 258, 258, 246, 233, 209, 196, 184, 160, 135, 110, 86, 73,
];
const MODEL3_CHILD_ROTATIONS: [i16; MODEL3_CHILD_COUNT] = [
    150, 165, 185, 200, 210, 210, 200, 190, 170, 160, 150, 130, 110, 90, 70, 60,
];

fn model3_distance_squared_2d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

impl Level2EconomyState {
    /// Reproduce `sub_4B6F0`'s ordered child allocation.  The caller owns the
    /// transaction: a missing slot or a map-link failure returns `None`, and
    /// the root allocator restores the pre-constructor snapshot.
    pub(super) fn allocate_model3_chain(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
    ) -> Option<Level2AuthoredClass5Model3State> {
        source.source_byte_39 = MODEL3_DESCRIPTOR_CADENCE
            .wrapping_sub(source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE)
            .wrapping_add(4);
        let mut previous_slot = 0;
        let mut first_slot = 0;
        for child_index in 0..MODEL3_CHILD_COUNT {
            let child_slot = self.allocate_slot()?;
            let initial_rng = self.new_event_rng(child_slot);
            let entity_rng = level2_next_rng16(initial_rng);
            let speed = MODEL3_CHILD_SPEEDS[child_index];
            let rotation_speed = MODEL3_CHILD_ROTATIONS[child_index];
            let axis = speed * 65 / 100;
            let fov = rotation_speed * 65 / 100;
            let word_0x36 = if child_index == 0 {
                axis * 125 / 100
            } else {
                axis
            };
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: child_slot,
                    active: true,
                    x: source.position.x,
                    y: source.position.y,
                })
                .is_err()
            {
                self.release_slot(child_slot, Some(entity_rng));
                return None;
            }
            let child = Level2AuthoredClass5Model3Child {
                source_entity_index: child_slot,
                entity_rng,
                parent_entity_index: source.source_entity_index,
                word_0x32: previous_slot,
                word_0x34: 0,
                word_0x36,
                word_0x24: 0,
                pending_word_0x62: 0,
                word_0x26: 0,
                word_0x96: 0,
                damage_dword_0x5e_94: 0,
                dword_0x10: source.dword_0x10,
                minimum_speed: source.minimum_speed,
                maximum_speed: source.maximum_speed,
                actual_speed: source.actual_speed,
                max_life: source.max_life,
                life: source.life,
                mana: source.max_mana >> 5,
                control_flags: 8,
                model: source.model,
                action: 232,
                source_byte_3e: u8::try_from(child_index).expect("model-3 child index fits"),
                byte_0x3b: 0,
                byte_0x43: 10,
                byte_0x44: 0,
                byte_0x46: 0,
                word_0x2c: 0,
                particle_index: MODEL3_CHILD_PARTICLES[child_index],
                yaw: rotation_speed / 2,
                pitch: axis,
                roll: axis,
                fov,
                shift_rot_x: 0,
                shift_rot_y: 0,
                position: source.position,
                phase: 0,
                drawing_enabled: true,
                active: true,
            };
            if previous_slot == 0 {
                first_slot = child_slot;
            } else if let Some(previous) = self
                .authored_class5_model3_children
                .iter_mut()
                .find(|child| child.source_entity_index == previous_slot)
            {
                previous.word_0x34 = child_slot;
            }
            self.source_rng.insert(child_slot, entity_rng);
            self.authored_class5_model3_children.push(child);
            previous_slot = child_slot;
        }
        Some(Level2AuthoredClass5Model3State {
            source_entity_index: source.source_entity_index,
            damage_dword_0x5e_94: 0,
            word_0x32: 0,
            word_0x34: first_slot,
            word_0x36: 96,
            control_flags: 8,
            byte_0x43: 10,
            byte_0x44: 0,
            word_0x2c: 0,
            descriptor_index: MODEL3_DESCRIPTOR_INDEX,
            last_child_root_phase: u8::MAX,
        })
    }

    #[inline]
    fn model3_state_index(&self, source_slot: u16) -> Option<usize> {
        self.authored_class5_model3
            .iter()
            .position(|state| state.source_entity_index == source_slot)
    }

    fn model3_child_slots(&self, first_slot: u16) -> Vec<u16> {
        let mut slots = Vec::with_capacity(MODEL3_CHILD_COUNT);
        let mut next = first_slot;
        while next != 0 && slots.len() < MODEL3_CHILD_COUNT {
            slots.push(next);
            next = self
                .authored_class5_model3_children
                .iter()
                .find(|child| child.source_entity_index == next)
                .map_or(0, |child| child.word_0x34);
        }
        slots
    }

    /// Advance one model-3 root through actions 25 and 216..220, then visit
    /// the linked child chain in source order.
    pub(super) fn step_authored_class5_model3(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        target: Option<EntitySnapshot>,
    ) {
        let Some(state_index) = self.model3_state_index(source.source_entity_index) else {
            return;
        };
        let mut model3 = self.authored_class5_model3[state_index];
        let mut run_terrain_resnap = true;
        let mut mailbox_fallback = false;
        // Action 25 owns the source `sub_1BF90(a1, 24)` prepass.  The
        // action-216..220 mailbox path is a separate authored callback and
        // must retain its existing recovery semantics.
        if matches!(source.action, 216..=220) && source.pending_word_0x62 != 0 {
            let pending = source.pending_word_0x62;
            source.word_0x26 = pending;
            source.pending_word_0x62 = 0;
            model3.damage_dword_0x5e_94 = 0;
            if source.byte_0x3b == 0 {
                source.action = 220;
                source.word_0x24 = pending;
                run_terrain_resnap = false;
                mailbox_fallback = true;
            } else {
                source.word_0x96_150 = pending;
                source.life = 1_000_000;
                if source.action == 217 {
                    source.action = 218;
                }
            }
        }

        if !mailbox_fallback {
            match source.action {
                25 => self.step_model3_action25(source, &mut model3, target),
                26 => self.step_model3_action26(source, &mut model3, target),
                27 => self.step_model3_action27(source, &mut model3),
                28 => self.step_model3_action28(source, &model3),
                30 => self.step_model3_action30(source, &mut model3),
                216 => self.step_model3_action216(source, &mut model3),
                217 => self.step_model3_action217(source, &mut model3),
                218 => self.step_model3_action218(source, &mut model3, target),
                219 => {
                    source.action = 217;
                    self.step_model3_action217(source, &mut model3);
                }
                220 => self.step_model3_action220(source, &model3),
                _ => {}
            }
        }
        if run_terrain_resnap {
            source.position.z = self.terrain.ground_height(source.position);
        }
        model3.last_child_root_phase = source.phase;
        self.authored_class5_model3[state_index] = model3;
        self.step_model3_children(source, &mut model3);
        self.authored_class5_model3[state_index] = model3;
        self.source_rng
            .insert(source.source_entity_index, source.entity_rng);
    }

    fn model3_source_prepass(
        &self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) -> u8 {
        let mut state = 0_u8;
        if source.pending_word_0x62 != 0 {
            source.life = source.life.wrapping_sub(model3.damage_dword_0x5e_94);
            source.word_0x26 = source.pending_word_0x62;
            source.pending_word_0x62 = 0;
            model3.damage_dword_0x5e_94 = 0;
            state = 1;
        } else {
            source.word_0x26 = 0;
        }

        for child_slot in self.model3_child_slots(model3.word_0x34) {
            let Some(child) = self
                .authored_class5_model3_children
                .iter()
                .find(|child| child.active && child.source_entity_index == child_slot)
            else {
                continue;
            };
            if child.life < source.life {
                source.life = child.life;
                source.word_0x26 = child.word_0x26;
                state = 1;
                break;
            }
        }

        if source.life < 0 {
            source.word_0x24 = source.word_0x26;
            state = 2;
        }
        state
    }

    fn step_model3_action25(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        target: Option<EntitySnapshot>,
    ) {
        // This is the reachable `sub_1BF90(source, 24)` body.  In
        // particular, action 25 is the acquisition/pre-pass callback; it is
        // not the action-218 target-loss wrapper.
        let state = self.model3_source_prepass(source, model3);
        if state == 2 {
            source.action = 28;
            return;
        }
        if state == 1 {
            source.word_0x96_150 = source.word_0x26;
            source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                30
            } else {
                26
            };
            return;
        }

        // The source moves before its cadence gate, then consumes exactly two
        // LCG draws and masks the signed roll update to eleven bits.
        let _ = self.model3_move_root(source, model3, true);
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }
        let first = level2_next_rng16(source.entity_rng);
        let second = level2_next_rng16(first);
        source.entity_rng = second;
        let direction = 2 * (i32::from(first % 0x009d) / 79) - 1;
        let delta = (i32::from(second & 0x00ff) + 85) * direction;
        source.roll = ((i32::from(source.roll) + delta) & 0x07ff) as i16;

        // The outer world currently supplies one lossless class-3 projection
        // here. Its validity/range/FOV checks are the source predicate; the
        // multi-row source-chain projection is completed by the caller's
        // nearest-candidate plumbing.
        let target = if source.source_byte_39 != 0 {
            target
                .filter(|target| {
                    target.active
                        && target.class == 3
                        && target.life >= 0
                        && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
                })
                .filter(|target| {
                    let distance = model3_distance_squared_2d(source.position, target.position);
                    distance <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                        && angle_distance(
                            i32::from(source.yaw),
                            heading_for_positions(source.position, target.position),
                        ) < MODEL3_TARGET_HALF_ANGLE
                })
        } else {
            None
        };
        if let Some(target) = target {
            source.word_0x96_150 = target.index;
            source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                30
            } else {
                26
            };
        } else if MODEL3_DESCRIPTOR_FLAGS & 4 == 0 {
            if let Some(peer) = self.model3_select_root_peer(source) {
                model3.word_0x32 = peer;
                source.action = 27;
            }
        }
    }

    fn step_model3_action26(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        target: Option<EntitySnapshot>,
    ) {
        // Action 26 is the source `sub_1F990` hit callback.  Its helper owns
        // the same damage/child prepass as action 25, but it consumes no
        // acquisition RNG of its own.
        match self.model3_source_prepass(source, model3) {
            2 => {
                source.action = 28;
                return;
            }
            1 => {
                source.word_0x96_150 = source.word_0x26;
                source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                    30
                } else {
                    26
                };
                source.position.z = self.terrain.ground_height(source.position);
                return;
            }
            _ => {}
        }

        let _ = self.model3_move_root(source, model3, true);
        let Some(target) = target.filter(|target| {
            target.active
                && target.class == 3
                && target.life >= 0
                && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
        }) else {
            source.action = 25;
            return;
        };
        if level2_wrapping_source_distance_3d(source.position, target.position)
            >= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
        {
            source.action = 25;
            return;
        }

        // `sub_1CC20` is the source hit helper.  The native-world boundary
        // applies its existing player-feedback carrier after this callback;
        // this event is the source sound-8 side effect of a successful hit.
        self.events
            .push(Level2EconomyEvent::AuthoredClass5Model3Sound {
                source_entity_index: source.source_entity_index,
                sound_id: 8,
            });
    }

    fn step_model3_action27(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) {
        // Action 27 is `sub_1F9E0 -> sub_1C560(a1, 24)`: retain the
        // same-model relation while the peer remains live, then hand off to
        // the source action selected by that peer.
        match self.model3_source_prepass(source, model3) {
            2 => {
                source.action = 28;
                return;
            }
            1 => {
                source.word_0x96_150 = source.word_0x26;
                source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                    30
                } else {
                    26
                };
                return;
            }
            _ => {}
        }
        let peer_slot = model3.word_0x32;
        if peer_slot == 0 {
            source.action = 25;
            return;
        }
        let peer = self
            .authored_class5
            .iter()
            .find(|peer| peer.active && peer.source_entity_index == peer_slot)
            .map(|peer| {
                (
                    peer.position,
                    peer.life,
                    self.authored_class5_model3
                        .iter()
                        .find(|state| state.source_entity_index == peer_slot)
                        .map_or(0, |state| state.control_flags),
                    peer.model,
                    peer.action,
                    peer.word_0x96_150,
                    self.authored_class5_model3
                        .iter()
                        .find(|state| state.source_entity_index == peer_slot)
                        .map_or(0, |state| state.word_0x32),
                )
            })
            .or_else(|| {
                self.authored_class5_model3_children
                    .iter()
                    .find(|peer| peer.active && peer.source_entity_index == peer_slot)
                    .map(|peer| {
                        (
                            peer.position,
                            peer.life,
                            peer.control_flags,
                            peer.model,
                            peer.action,
                            peer.word_0x96,
                            peer.word_0x32,
                        )
                    })
            });
        let Some((
            peer_position,
            peer_life,
            peer_flags,
            peer_model,
            peer_action,
            peer_target,
            peer_peer,
        )) = peer
        else {
            model3.word_0x32 = 0;
            source.action = 25;
            return;
        };
        if peer_life < 0 || peer_flags & ENTITY_DRAW_DISABLED_BIT != 0 || peer_model != source.model
        {
            model3.word_0x32 = 0;
            source.action = 25;
            return;
        }

        let _ = self.model3_move_root(source, model3, true);
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }
        match peer_action.wrapping_sub(24) {
            0 | 1 => {
                source.roll = i16::try_from(heading_for_positions(source.position, peer_position))
                    .expect("model-3 peer heading fits");
            }
            2 => {
                source.word_0x96_150 = peer_target;
                model3.word_0x32 = 0;
                source.action = 26;
            }
            3 => model3.word_0x32 = peer_peer,
            6 => {
                source.word_0x96_150 = peer_target;
                model3.word_0x32 = 0;
                source.action = 30;
            }
            _ => {
                model3.word_0x32 = 0;
                source.action = 25;
            }
        }
    }

    fn step_model3_action28(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &Level2AuthoredClass5Model3State,
    ) {
        let mut transferred = source.word_0x24;
        for child_slot in self.model3_child_slots(model3.word_0x34) {
            if let Some(child) = self
                .authored_class5_model3_children
                .iter_mut()
                .find(|child| child.active && child.source_entity_index == child_slot)
            {
                child.action = 29;
                if child.word_0x24 != 0 {
                    transferred = child.word_0x24;
                }
            }
        }
        source.word_0x24 = transferred;
        source.action = 29;
    }

    fn step_model3_action30(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) {
        // `sub_1FA50` hands the entity to `sub_1D5D0`.  The model-3
        // constructor carries the default stage value, whose source switch
        // is a no-op; retain the callback boundary and ordinary cursor
        // bookkeeping without inventing a Level-2-only stage family.
        let _ = (source, model3);
    }

    /// Dispatch the post-acquisition action family through the same source
    /// cursor bookkeeping used by the shared class-5 entry point.  The
    /// native-world caller owns target projection; this seam keeps actions
    /// 26/27/28/30 on the model-3 carrier without changing other models.
    pub(crate) fn step_authored_class5_model3_slot_with_target(
        &mut self,
        source_slot: u16,
        target: Option<EntitySnapshot>,
    ) -> bool {
        let Some(source_index) = self
            .authored_class5
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        if source.model != 3 {
            return false;
        }
        self.step_authored_class5_model3(&mut source, target);
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.authored_class5[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    fn step_model3_action216(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) {
        let previous = source.dword_0x10;
        source.dword_0x10 = previous.wrapping_add(1);
        match previous {
            0 => {
                source.life = 1_000_000;
                model3.word_0x36 = 0;
            }
            3 | 15 => self.model3_clear_child_word1_and_set_byte43(model3),
            6 | 12 => self.model3_clear_child_word1_and_set_byte2(model3),
            9 => {
                let first = level2_next_rng16(source.entity_rng);
                let x_offset = i16::try_from((first & 7) + 8).expect("model-3 offset fits");
                let second = level2_next_rng16(first);
                let y_offset = i16::try_from((second & 7) + 8).expect("model-3 offset fits");
                source.entity_rng = second;
                source.position.x = source
                    .position
                    .x
                    .wrapping_add(u16::try_from(i32::from(x_offset) << 8).unwrap_or_default());
                source.position.y = source
                    .position
                    .y
                    .wrapping_add(u16::try_from(i32::from(y_offset) << 8).unwrap_or_default());
                source.position.z = self.terrain.ground_height(source.position);
                self.events
                    .push(Level2EconomyEvent::AuthoredClass5Model3Sound {
                        source_entity_index: source.source_entity_index,
                        sound_id: 22,
                    });
            }
            18 => {
                source.action = 218;
                source.particle_index = 337;
                model3.word_0x36 = 0;
                model3.byte_0x43 = 1;
            }
            _ => {}
        }
    }

    fn step_model3_action217(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) {
        let _ = self.model3_move_root(source, model3, true);
        if source.source_byte_3e & 0x3f == 0 {
            let random = level2_next_rng16(source.entity_rng);
            source.entity_rng = random;
            let delta = i32::from(random % 0x1c7) + i32::from(source.roll) - 227;
            source.roll = (delta & 0x07ff) as i16;
        }
    }

    fn step_model3_action218(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        target: Option<EntitySnapshot>,
    ) {
        let move_result = self.model3_move_root(source, model3, source.byte_0x46_70 == 0);
        if move_result == 4 {
            source.action = 216;
            source.dword_0x10 = 0;
            return;
        }
        let target_valid = target.as_ref().is_some_and(|target| {
            target.active
                && target.life >= 0
                && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
        });
        if source.source_byte_3e & 3 == 0 && model3.byte_0x43 == 0 && target_valid {
            let heading = heading_for_positions(
                source.position,
                target.as_ref().expect("validated target").position,
            );
            source.roll = i16::try_from(heading).expect("model-3 target heading fits");
        }
        if source.source_byte_3e & 0x1f == 0 {
            let target_lost = !target_valid || {
                let target = target.as_ref().expect("validated target");
                let heading = heading_for_positions(source.position, target.position);
                if angle_distance(i32::from(source.yaw), heading) <= MODEL3_TARGET_HALF_ANGLE {
                    source.byte_0x46_70 = 1;
                    source.particle_index = 337;
                } else {
                    source.byte_0x46_70 = 0;
                    source.particle_index = 315;
                }
                crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                    source.position,
                    target.position,
                )) >= MODEL3_TARGET_RANGE
            };
            if target_lost {
                source.action = 217;
                source.particle_index = 315;
                source.word_0x96_150 = 0;
                source.byte_0x46_70 = 0;
            }
        }
    }

    fn step_model3_action220(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &Level2AuthoredClass5Model3State,
    ) {
        source.life = -1;
        source.action = 221;
        source.drawing_enabled = false;
        for child_slot in self.model3_child_slots(model3.word_0x34) {
            if let Some(child) = self
                .authored_class5_model3_children
                .iter_mut()
                .find(|child| child.source_entity_index == child_slot)
            {
                child.action = 221;
                child.drawing_enabled = false;
            }
        }
    }

    fn model3_move_root(
        &self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        should_move: bool,
    ) -> u8 {
        if model3.control_flags & 0x0010_0000 != 0 {
            model3.control_flags &= !0x0010_0000;
        }
        if should_move && source.actual_speed != 0 {
            let previous = source.position;
            crate::move_entity(&mut source.position, source.yaw, 0, source.actual_speed);
            source.position.z = self.terrain.ground_height(source.position);
            if previous.x >> 8 == source.position.x >> 8
                && previous.y >> 8 == source.position.y >> 8
            {
                return 1;
            }
        }
        source.position.z = self.terrain.ground_height(source.position);
        2
    }

    fn model3_clear_child_word1_and_set_byte43(
        &mut self,
        model3: &Level2AuthoredClass5Model3State,
    ) {
        for child_slot in self.model3_child_slots(model3.word_0x34) {
            if let Some(child) = self
                .authored_class5_model3_children
                .iter_mut()
                .find(|child| child.source_entity_index == child_slot)
            {
                child.control_flags &= !0x0000_0180;
                child.byte_0x43 |= 1;
            }
        }
    }

    fn model3_clear_child_word1_and_set_byte2(&mut self, model3: &Level2AuthoredClass5Model3State) {
        for child_slot in self.model3_child_slots(model3.word_0x34) {
            if let Some(child) = self
                .authored_class5_model3_children
                .iter_mut()
                .find(|child| child.source_entity_index == child_slot)
            {
                child.control_flags &= !0x0000_0180;
                child.control_flags |= 0x0080_0000;
            }
        }
    }

    fn model3_child_target(&self, target_slot: u16) -> Option<(Position, i32, u32)> {
        if let Some(source) = self
            .authored_class5
            .iter()
            .find(|source| source.active && source.source_entity_index == target_slot)
        {
            return Some((source.position, source.life, 0));
        }
        self.authored_class5_model3_children
            .iter()
            .find(|child| child.active && child.source_entity_index == target_slot)
            .map(|child| (child.position, child.life, child.control_flags))
    }

    fn model3_select_peer(&self, child: &Level2AuthoredClass5Model3Child) -> Option<u16> {
        let mut candidates = self
            .authored_class5
            .iter()
            .filter(|source| source.active && source.model == 3)
            .map(|source| {
                (
                    source.source_entity_index,
                    source.position,
                    source.life,
                    0_u32,
                )
            })
            .chain(
                self.authored_class5_model3_children
                    .iter()
                    .filter(|peer| {
                        peer.active && peer.source_entity_index != child.source_entity_index
                    })
                    .map(|peer| {
                        (
                            peer.source_entity_index,
                            peer.position,
                            peer.life,
                            peer.control_flags,
                        )
                    }),
            )
            .filter(|(slot, _, life, control_flags)| {
                *slot != child.parent_entity_index
                    && *life >= 0
                    && *control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|(slot, ..)| *slot);
        candidates
            .into_iter()
            .filter_map(|(slot, position, _, _)| {
                let distance = crate::legacy_sqrt(level2_wrapping_source_distance_3d(
                    child.position,
                    position,
                ));
                let heading = heading_for_positions(child.position, position);
                (distance < MODEL3_TARGET_RANGE
                    && angle_distance(i32::from(child.yaw), heading) <= MODEL3_TARGET_HALF_ANGLE)
                    .then_some((slot, distance))
            })
            .min_by_key(|(slot, distance)| (*distance, *slot))
            .map(|(slot, _)| slot)
    }

    fn model3_select_root_peer(&self, source: &Level2AuthoredClass5Source) -> Option<u16> {
        let roots = self
            .authored_class5
            .iter()
            .filter(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.model == source.model
                    && peer.source_entity_index != source.source_entity_index
                    && !matches!(peer.action, 0xb4 | 0xe8 | 0xea)
                    && self
                        .authored_class5_model3
                        .iter()
                        .find(|state| state.source_entity_index == peer.source_entity_index)
                        .is_some_and(|state| state.word_0x32 == 0)
            })
            .map(|peer| (peer.source_entity_index, peer.position));
        let children = self
            .authored_class5_model3_children
            .iter()
            .filter(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.model == source.model
                    && peer.source_entity_index != source.source_entity_index
                    && peer.word_0x32 == 0
                    && !matches!(peer.action, 0xb4 | 0xe8 | 0xea)
            })
            .map(|peer| (peer.source_entity_index, peer.position));
        roots
            .chain(children)
            .filter_map(|(slot, position)| {
                let distance = model3_distance_squared_2d(source.position, position);
                (distance <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                    && angle_distance(
                        i32::from(source.yaw),
                        heading_for_positions(source.position, position),
                    ) < MODEL3_TARGET_HALF_ANGLE)
                    .then_some((slot, distance))
            })
            .min_by_key(|(_, distance)| *distance)
            .map(|(slot, _)| slot)
    }

    fn apply_model3_child_mailbox(
        &mut self,
        root: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        child: &mut Level2AuthoredClass5Model3Child,
    ) {
        let damage = child.damage_dword_0x5e_94.clamp(0, 76);
        if child.pending_word_0x62 != 0 {
            root.pending_word_0x62 = child.pending_word_0x62;
            model3.damage_dword_0x5e_94 = damage;
            child.word_0x26 = child.pending_word_0x62;
            child.pending_word_0x62 = 0;
        }
        if damage != 0 {
            child.life = child.life.wrapping_sub(damage);
            child.damage_dword_0x5e_94 = 0;
            if child.life < 0 {
                child.byte_0x46 = 6;
            }
            self.events.push(Level2EconomyEvent::DamageApplied {
                target_entity_index: child.source_entity_index,
                attacker_entity_index: root.source_entity_index,
                damage,
                lethal: child.life < 0,
            });
        }
    }

    fn step_model3_children(
        &mut self,
        root: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) {
        let child_slots = self.model3_child_slots(model3.word_0x34);
        for child_slot in child_slots {
            let Some(child_index) = self
                .authored_class5_model3_children
                .iter()
                .position(|child| child.active && child.source_entity_index == child_slot)
            else {
                continue;
            };
            let mut child = self.authored_class5_model3_children[child_index];
            if child.action == 232 {
                child.action = 233;
            }
            if child.byte_0x46 <= 5 {
                let random = level2_next_rng16(child.entity_rng);
                child.entity_rng = random;
                child.byte_0x43 = u8::try_from(random % 20).expect("model-3 child roll fits");
                self.apply_model3_child_mailbox(root, model3, &mut child);
            }
            match child.byte_0x46 {
                0 => {
                    child.word_0x96 = 0;
                    child.byte_0x46 = 1;
                    child.word_0x2c = 0;
                    child.word_0x36 = 0;
                    child.minimum_speed = 16;
                }
                1 => {
                    if root.source_byte_39 != 0 && child.source_byte_3e & 7 == 0 {
                        if child.byte_0x43 > 4 {
                            child.byte_0x46 = 4;
                        } else if let Some(peer) = self.model3_select_peer(&child) {
                            child.byte_0x46 = 2;
                            child.word_0x96 = peer;
                        }
                        if child.byte_0x43 & 1 == 0 {
                            let random = level2_next_rng16(child.entity_rng);
                            child.entity_rng = random;
                            let offset = i16::try_from(random % 0x02aa).expect("yaw roll fits");
                            child.yaw = (root.yaw + offset - MODEL3_TARGET_HALF_ANGLE) & 0x07ff;
                        }
                    }
                }
                2 => {
                    child.byte_0x46 = 3;
                    child.byte_0x44 = 0;
                    child.word_0x2c = 2;
                    child.minimum_speed = 16;
                }
                3 => {
                    let target = (child.word_0x96 != 0)
                        .then(|| self.model3_child_target(child.word_0x96))
                        .flatten()
                        .filter(|(_, life, flags)| {
                            *life >= 0 && *flags & ENTITY_DRAW_DISABLED_BIT == 0
                        });
                    if let Some((target_position, _, _)) = target {
                        if child.byte_0x44 < 1 && child.actual_speed == 0 {
                            child.byte_0x44 = 1;
                            child.word_0x2c = 1;
                            child.minimum_speed = 16;
                            child.yaw = i16::try_from(heading_for_positions(
                                child.position,
                                target_position,
                            ))
                            .expect("child target heading fits");
                            child.pitch = 0;
                            child.roll = child.yaw.wrapping_sub(root.yaw);
                            child.fov = child.pitch.wrapping_sub(root.pitch);
                        } else if child.byte_0x44 <= 1 && child.actual_speed == 192 {
                            child.word_0x2c = 3;
                            child.byte_0x44 = 3;
                            child.dword_0x10 = 4;
                        } else if child.byte_0x44 == 3 {
                            child.dword_0x10 = child.dword_0x10.wrapping_sub(1);
                            if child.dword_0x10 == 0 {
                                child.byte_0x46 = 0;
                                child.dword_0x10 = 1;
                            }
                        }
                    } else {
                        child.byte_0x46 = 0;
                    }
                }
                4 => {
                    child.byte_0x46 = 5;
                    child.byte_0x44 = 0;
                    child.word_0x2c = 2;
                    child.minimum_speed = 16;
                }
                5 => {
                    if child.byte_0x44 == 0 && child.actual_speed == 0 {
                        child.byte_0x44 = 1;
                        child.word_0x2c = 1;
                        child.minimum_speed = -16;
                        child.roll = root.roll;
                        child.fov = root.fov;
                        child.yaw = child.roll.wrapping_add(root.yaw);
                        child.pitch = child.fov.wrapping_add(root.pitch);
                    }
                    if child.byte_0x44 == 1 && child.actual_speed == -192 {
                        child.byte_0x44 = 2;
                        child.dword_0x10 = 2;
                    }
                    if child.byte_0x44 == 2 {
                        child.dword_0x10 = child.dword_0x10.wrapping_sub(1);
                        if child.dword_0x10 == 0 {
                            child.word_0x2c = 4;
                            child.byte_0x44 = 6;
                            child.dword_0x10 = 1;
                        }
                    }
                    if child.byte_0x44 == 6 {
                        self.events
                            .push(Level2EconomyEvent::AuthoredClass5Model3Sound {
                                source_entity_index: root.source_entity_index,
                                sound_id: 17,
                            });
                        child.dword_0x10 = child.dword_0x10.wrapping_add(1);
                        if child.dword_0x10 >= 4 {
                            child.byte_0x44 = 5;
                        }
                    }
                }
                6 | 7 => {
                    child.dword_0x10 = 0;
                    child.word_0x2c = 2;
                    child.byte_0x46 = 7;
                    child.byte_0x44 = 0;
                    child.minimum_speed = 80;
                }
                _ => {}
            }
            if child.actual_speed != 0 && matches!(child.byte_0x46, 2 | 3 | 5 | 7) {
                crate::move_entity(&mut child.position, child.yaw, 0, child.actual_speed);
            }
            child.position.z = self.terrain.ground_height(child.position);
            child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
            child.phase = child.phase.wrapping_add(1);
            self.source_rng.insert(child_slot, child.entity_rng);
            self.authored_class5_model3_children[child_index] = child;
        }
    }

    /// Advance a child when it is reached directly by the shared dynamic
    /// arena. The root normally visits the chain first; this guard keeps the
    /// cursor idempotent if a child is exposed by a later runagain scan.
    pub(crate) fn step_authored_class5_model3_child_slot(&mut self, source_slot: u16) -> bool {
        let Some(child) = self
            .authored_class5_model3_children
            .iter()
            .find(|child| child.active && child.source_entity_index == source_slot)
            .copied()
        else {
            return false;
        };
        let Some(root) = self
            .authored_class5
            .iter()
            .find(|source| source.active && source.source_entity_index == child.parent_entity_index)
            .copied()
        else {
            return false;
        };
        let Some(root_index) = self
            .authored_class5
            .iter()
            .position(|source| source.source_entity_index == root.source_entity_index)
        else {
            return false;
        };
        let Some(state_index) = self.model3_state_index(root.source_entity_index) else {
            return false;
        };
        let model3_state = self.authored_class5_model3[state_index];
        if root.phase == model3_state.last_child_root_phase.wrapping_add(1) {
            // The root callback has already walked this chain during the
            // current outer pass.  The child remains a live cursor row, but
            // must not consume its RNG or movement a second time.
            return true;
        }
        let mut root = root;
        let mut model3 = model3_state;
        self.step_model3_children(&mut root, &mut model3);
        self.authored_class5_model3[state_index] = model3;
        self.authored_class5[root_index] = root;
        self.authored_class5_model3_children
            .iter()
            .any(|child| child.active && child.source_entity_index == source_slot)
    }
}
