//! Complete reachable class-5/model-3 constructor and action family.
//!
//! The Level 2 model-3 carrier is the `sub_4B6F0` root (action 25) and its
//! sixteen action-232 presentation children.  The model-27 carrier from
//! `sub_4D000` is a separate source family and is deliberately absent here.

use crate::{FlightWorld, Position, Tile};

use super::super::*;

const MODEL3_DESCRIPTOR_INDEX: u16 = 74;
const MODEL3_DESCRIPTOR_FLAGS: u8 = 1;
const MODEL3_DESCRIPTOR_CADENCE: u8 = 30;
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

// Descriptor row 74 (`str_D7BD6[74]`) is passed to `sub_580E0` by both
// `sub_1B8C0` and `sub_1EEE0`: ground offset 0 and a -64 approach step.
const MODEL3_SOURCE_SETTLE_STEP: i16 = -0x40;
const MODEL3_SOURCE_SLOPE_LIMIT: i32 = 0x14;
const MODEL3_ALLOWED_MATERIAL_MASK: u32 = 0xfff0_80fe;
const MODEL3_SOURCE_CAVE_CLEARANCE: i32 = 0;
const MODEL3_RAW_MOTION_COLLISION_BIT: u32 = 0x0004_0000;
const MODEL3_ACTION26_ARRAY_PITCH: i16 = 1;

fn model3_distance_squared_2d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .wrapping_add(dy.wrapping_mul(dy))
        .cast_unsigned()
}

fn model3_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
    let tile_x = position.x.to_be_bytes()[0];
    let tile_y = position.y.to_be_bytes()[0];
    let height = |x: u8, y: u8| {
        i32::from(world.ground_height(Position {
            x: u16::from(x) << 8,
            y: u16::from(y) << 8,
            z: 0,
        })) >> 5
    };
    let point_1 = height(tile_x, tile_y);
    let point_2 = height(tile_x.wrapping_add(1), tile_y);
    let point_3 = height(tile_x.wrapping_add(1), tile_y.wrapping_add(1));
    let point_4 = height(tile_x, tile_y.wrapping_add(1));
    let diagonal_1 = (point_1 + point_4 - point_2 - point_3).abs();
    let diagonal_2 = (point_1 + point_2 - point_4 - point_3).abs();
    diagonal_1.max(diagonal_2)
}

fn model3_sub102d0_probe_blocked<W: FlightWorld>(
    source: &Level2AuthoredClass5Source,
    candidate: Position,
    heading: i16,
    world: &W,
) -> bool {
    let maximum_extent = i32::from(source.pitch.max(source.roll)).max(0);
    let mut probe = candidate;
    let mut distance = 0;
    while distance <= maximum_extent {
        if world.material_mask(probe) & !MODEL3_ALLOWED_MATERIAL_MASK != 0 {
            return true;
        }
        if world.cave_collision_blocked(probe) {
            return true;
        }
        if let Some(ceiling) = world.ceiling_height(probe) {
            let tile = Tile {
                x: probe.x.to_be_bytes()[0],
                y: probe.y.to_be_bytes()[0],
            };
            if world.angle_flags(tile) & 8 != 0
                || i32::from(source.fov)
                    + i32::from(world.ground_height(probe))
                    + MODEL3_SOURCE_CAVE_CLEARANCE
                    > i32::from(ceiling)
            {
                return true;
            }
        }
        distance += 256;
        crate::move_entity(&mut probe, heading, 0, 256);
    }
    false
}

fn model3_candidate_blocked<W: FlightWorld>(
    source: &Level2AuthoredClass5Source,
    candidate: Position,
    heading: i16,
    world: &W,
) -> bool {
    model3_sub102d0_probe_blocked(source, candidate, heading, world)
        || model3_slope_measure(candidate, world) >= MODEL3_SOURCE_SLOPE_LIMIT
}

fn model3_source_settle_position<W: FlightWorld>(position: &mut Position, world: &W) {
    let ground = world.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(MODEL3_SOURCE_SETTLE_STEP);
    }
    if position.z <= ground {
        position.z = ground;
    }
}

fn model3_turn_delta(from: i16, to: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    let magnitude = delta.abs().min(0x22);
    i16::try_from(magnitude * delta.signum()).expect("model-3 turn delta fits i16")
}

fn model3_heading_after_turn(heading: i16, roll: i16) -> i16 {
    i16::try_from((i32::from(heading) + i32::from(model3_turn_delta(heading, roll))) & 0x07ff)
        .expect("masked model-3 heading fits i16")
}

fn model3_source_retry_headings(yaw: i16) -> [i16; 3] {
    let original = u16::from_ne_bytes(yaw.to_ne_bytes());
    let second = original.wrapping_add(341) & 0x07ff;
    let third = (original.wrapping_sub(85) & 0x00ff) | ((original.wrapping_sub(341) >> 8 & 7) << 8);
    let fourth = (original.wrapping_add(0x0400) & 0x0700) | (original & 0x00ff);
    [
        i16::try_from(second).expect("model-3 retry heading fits"),
        i16::try_from(third).expect("model-3 retry heading fits"),
        i16::try_from(fourth).expect("model-3 retry heading fits"),
    ]
}

#[derive(Clone, Copy, Debug)]
struct Model3Peer {
    source_entity_index: u16,
    position: Position,
    life: i32,
    maximum_speed: i16,
    actual_speed: i16,
    action: u8,
    word_0x96: u16,
    word_0x32: u16,
    drawing_enabled: bool,
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
                // qmemcpy copies the root's word 0x32; the constructor then
                // writes the previous linked entity into every new child.
                word_0x32: if previous_slot == 0 {
                    source.source_entity_index
                } else {
                    previous_slot
                },
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

    /// Advance one model-3 root through the reachable source callbacks. The
    /// action-232 children are presentation rows copied by the constructor;
    /// no model-27 child callback is attached to them.
    pub(super) fn step_authored_class5_model3(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        target: Option<EntitySnapshot>,
    ) {
        let Some(state_index) = self.model3_state_index(source.source_entity_index) else {
            return;
        };
        let mut model3 = self.authored_class5_model3[state_index];
        match source.action {
            25 => self.step_model3_action25(source, &mut model3, target),
            26 => self.step_model3_action26(source, &mut model3, target),
            27 => self.step_model3_action27(source, &mut model3),
            28 => self.step_model3_action28(source, &model3),
            30 => self.step_model3_action30(source, &mut model3),
            // 216..220 are model-27 actions owned by sub_4D000. A malformed
            // model-3 row is intentionally inert rather than borrowing that
            // carrier's state machine.
            _ => {}
        }
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
            // sub_1BF90 reads dword 0x5e94 but does not clear it. Reset and
            // replay therefore retain the same mailbox/hash value.
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
            self.model3_source_terrain_settle(source);
            return;
        }

        // sub_1B8C0 runs before the descriptor cadence gate.
        let _ = self.model3_step_source_movement(source, model3);
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }
        let first = level2_next_rng16(source.entity_rng);
        let second = level2_next_rng16(first);
        source.entity_rng = second;
        let direction = 2 * (i32::from(first % 0x009d) / 79) - 1;
        let delta = (i32::from(second & 0x00ff) + 85) * direction;
        source.roll = ((i32::from(source.roll) + delta) & 0x07ff) as i16;

        if source.source_byte_39 == 0 {
            return;
        }
        let target = target
            .filter(|target| {
                target.active
                    && target.class == 3
                    && target.life >= 0
                    && target.control_flags & MODEL3_RAW_SOURCE_DISABLED_BIT == 0
            })
            .filter(|target| {
                model3_distance_squared_2d(source.position, target.position)
                    <= MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                    && angle_distance(
                        i32::from(source.yaw),
                        heading_for_positions(source.position, target.position),
                    ) < MODEL3_TARGET_HALF_ANGLE
            });
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

    /// Port `sub_1F990 -> sub_1C310`. The native-world caller supplies the
    /// lossless target snapshot and applies the established player-feedback
    /// carrier when the sound-8 hit event is emitted.
    fn step_model3_action26(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
        target: Option<EntitySnapshot>,
    ) {
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
                self.model3_source_terrain_settle(source);
                return;
            }
            _ => {}
        }

        let _ = self.model3_step_source_movement(source, model3);
        let Some(target) = target.filter(|target| {
            target.active
                && target.life >= 0
                && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
        }) else {
            source.action = 25;
            return;
        };
        if crate::legacy_sqrt(level2_wrapping_source_distance_3d(
            source.position,
            target.position,
        )) >= MODEL3_TARGET_RANGE
        {
            source.action = 25;
            return;
        }
        if source.source_byte_3e & 3 == 0 {
            source.roll = i16::try_from(heading_for_positions(source.position, target.position))
                .expect("model-3 target heading fits");
            self.model3_retarget_heading_nearby(source, MODEL3_ACTION26_ARRAY_PITCH);
        }
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }

        // sub_1CC20 may fail to allocate its temporary sphere when the source
        // arena is exhausted. Preserve that branch by making the sound and
        // feedback conditional on an available arena row.
        if !self.free_slots.is_empty() {
            self.events
                .push(Level2EconomyEvent::AuthoredClass5Model3Sound {
                    source_entity_index: source.source_entity_index,
                    sound_id: 8,
                });
        }
        source.word_0x96_150 = source.word_0x26;
        self.model3_source_terrain_settle(source);
    }

    /// Port `sub_1F9E0 -> sub_1C560`, using the model-3 root sidecar for the
    /// source word 0x32 relation and retaining source-order peer ties.
    fn step_model3_action27(
        &mut self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) {
        let peer = self.model3_peer_info(model3.word_0x32);
        let peer_valid = peer.is_some_and(|peer| {
            peer.life >= 0
                && peer.drawing_enabled
                && peer.source_entity_index != source.source_entity_index
        });
        match self.model3_source_prepass(source, model3) {
            2 => {
                if let Some(peer) = peer.filter(|_| peer_valid) {
                    self.model3_update_peer_state(peer.source_entity_index, source.word_0x26);
                }
                model3.word_0x32 = 0;
                source.action = 28;
                return;
            }
            1 => {
                if let Some(peer) = peer.filter(|_| peer_valid) {
                    self.model3_update_peer_state(peer.source_entity_index, source.word_0x26);
                }
                model3.word_0x32 = 0;
                source.word_0x96_150 = source.word_0x26;
                source.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                    30
                } else {
                    26
                };
                self.model3_source_terrain_settle(source);
                return;
            }
            _ => {}
        }
        let _ = self.model3_step_source_movement(source, model3);
        let Some(peer) = peer.filter(|_| peer_valid) else {
            model3.word_0x32 = 0;
            source.action = 25;
            return;
        };
        if source.source_byte_3e % MODEL3_DESCRIPTOR_CADENCE != 0 {
            return;
        }

        match peer.action.wrapping_sub(24) {
            0 | 1 => {
                source.roll = i16::try_from(heading_for_positions(source.position, peer.position))
                    .expect("model-3 peer heading fits");
                self.model3_retarget_heading_nearby(source, 256);
                source.actual_speed = source
                    .actual_speed
                    .wrapping_add(peer.maximum_speed)
                    .wrapping_add(peer.actual_speed);
            }
            2 => {
                source.word_0x96_150 = peer.word_0x96;
                model3.word_0x32 = 0;
                source.action = 26;
            }
            3 => {
                model3.word_0x32 = peer.word_0x32;
                let heading_target = self
                    .model3_position_for_slot(peer.word_0x32)
                    .unwrap_or(peer.position);
                source.roll = i16::try_from(heading_for_positions(source.position, heading_target))
                    .expect("model-3 chained peer heading fits");
                self.model3_retarget_heading_nearby(source, 256);
                source.actual_speed = source
                    .actual_speed
                    .wrapping_add(peer.maximum_speed)
                    .wrapping_add(peer.actual_speed);
            }
            6 => {
                source.word_0x96_150 = peer.word_0x96;
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

    /// Model-3 descriptor 74 starts with the default stage value, so the
    /// action-30 `sub_1D5D0` switch has no case and returns without mutation.
    fn step_model3_action30(
        &mut self,
        _source: &mut Level2AuthoredClass5Source,
        _model3: &mut Level2AuthoredClass5Model3State,
    ) {
    }

    fn model3_step_source_movement(
        &self,
        source: &mut Level2AuthoredClass5Source,
        model3: &mut Level2AuthoredClass5Model3State,
    ) -> u8 {
        if model3.control_flags & 8 != 0 {
            model3.control_flags &= !8;
            return 4;
        }
        let before = source.position;
        let mut predicted = before;
        model3_source_settle_position(&mut predicted, &self.terrain);
        crate::move_entity(&mut predicted, source.yaw, 0, source.actual_speed);
        let crossed_tile = before.x >> 8 != predicted.x >> 8 || before.y >> 8 != predicted.y >> 8;
        if !crossed_tile {
            source.position = predicted;
            model3.control_flags &= !MODEL3_RAW_MOTION_COLLISION_BIT;
            source.yaw = model3_heading_after_turn(source.yaw, source.roll);
            return 1;
        }

        if !model3_candidate_blocked(source, predicted, source.yaw, &self.terrain) {
            source.position = predicted;
            model3.control_flags &= !MODEL3_RAW_MOTION_COLLISION_BIT;
            source.yaw = model3_heading_after_turn(source.yaw, source.roll);
            return 2;
        }

        model3.control_flags |= MODEL3_RAW_MOTION_COLLISION_BIT;
        let original_yaw = source.yaw;
        for heading in model3_source_retry_headings(original_yaw) {
            source.yaw = heading;
            let mut candidate = before;
            model3_source_settle_position(&mut candidate, &self.terrain);
            crate::move_entity(&mut candidate, heading, 0, source.actual_speed);
            if !model3_candidate_blocked(source, candidate, heading, &self.terrain) {
                source.position = candidate;
                source.yaw = model3_heading_after_turn(heading, source.roll);
                return 3;
            }
        }

        if MODEL3_DESCRIPTOR_FLAGS & 1 != 0 {
            source.life = -1;
        }
        4
    }

    fn model3_source_terrain_settle(&self, source: &mut Level2AuthoredClass5Source) {
        model3_source_settle_position(&mut source.position, &self.terrain);
    }

    fn model3_position_for_slot(&self, source_slot: u16) -> Option<Position> {
        self.authored_class5
            .iter()
            .find(|source| {
                source.active && source.model == 3 && source.source_entity_index == source_slot
            })
            .map(|source| source.position)
            .or_else(|| {
                self.authored_class5_model3_children
                    .iter()
                    .find(|child| {
                        child.active && child.model == 3 && child.source_entity_index == source_slot
                    })
                    .map(|child| child.position)
            })
    }

    fn model3_peer_info(&self, source_slot: u16) -> Option<Model3Peer> {
        let source = self.authored_class5.iter().find(|source| {
            source.active && source.model == 3 && source.source_entity_index == source_slot
        })?;
        let state = self
            .authored_class5_model3
            .iter()
            .find(|state| state.source_entity_index == source_slot)?;
        Some(Model3Peer {
            source_entity_index: source.source_entity_index,
            position: source.position,
            life: source.life,
            maximum_speed: source.maximum_speed,
            actual_speed: source.actual_speed,
            action: source.action,
            word_0x96: source.word_0x96_150,
            word_0x32: state.word_0x32,
            drawing_enabled: source.drawing_enabled,
        })
    }

    fn model3_same_model_positions(&self, source_slot: u16) -> Vec<(u16, Position)> {
        let mut positions = self
            .authored_class5
            .iter()
            .filter(|source| source.active && source.model == 3)
            .map(|source| (source.source_entity_index, source.position))
            .collect::<Vec<_>>();
        positions.sort_unstable_by_key(|(slot, _)| *slot);
        positions.retain(|(slot, _)| *slot != source_slot);
        positions
    }

    fn model3_retarget_heading_nearby(
        &self,
        source: &mut Level2AuthoredClass5Source,
        proximity: i16,
    ) {
        for (_, position) in self.model3_same_model_positions(source.source_entity_index) {
            let dx = i32::from(position.x.wrapping_sub(source.position.x).cast_signed()).abs();
            let dy = i32::from(position.y.wrapping_sub(source.position.y).cast_signed()).abs();
            if dx < i32::from(proximity) && dy < i32::from(proximity) {
                source.roll = i16::try_from(heading_for_positions(position, source.position))
                    .expect("model-3 nearby heading fits");
                break;
            }
        }
    }

    fn model3_select_root_peer(&self, source: &Level2AuthoredClass5Source) -> Option<u16> {
        let mut nearest: Option<(u16, u32)> = None;
        let mut peers = self
            .authored_class5
            .iter()
            .filter(|peer| peer.active && peer.model == source.model)
            .collect::<Vec<_>>();
        peers.sort_unstable_by_key(|peer| peer.source_entity_index);
        for peer in peers {
            let Some(state) = self
                .authored_class5_model3
                .iter()
                .find(|state| state.source_entity_index == peer.source_entity_index)
            else {
                continue;
            };
            if peer.source_entity_index == source.source_entity_index || state.word_0x32 != 0 {
                continue;
            }
            let distance = model3_distance_squared_2d(source.position, peer.position);
            if distance > MODEL3_TARGET_RANGE * MODEL3_TARGET_RANGE
                || angle_distance(
                    i32::from(source.yaw),
                    heading_for_positions(peer.position, source.position),
                ) >= MODEL3_TARGET_HALF_ANGLE
            {
                continue;
            }
            if nearest.is_none_or(|(_, nearest_distance)| distance < nearest_distance) {
                nearest = Some((peer.source_entity_index, distance));
            }
        }
        nearest.map(|(slot, _)| slot)
    }

    fn model3_update_peer_state(&mut self, peer_slot: u16, relation: u16) {
        if let Some(peer) = self
            .authored_class5
            .iter_mut()
            .find(|peer| peer.source_entity_index == peer_slot)
        {
            peer.word_0x96_150 = relation;
            peer.action = if MODEL3_DESCRIPTOR_FLAGS & 8 != 0 {
                30
            } else {
                26
            };
        }
        if let Some(state) = self
            .authored_class5_model3
            .iter_mut()
            .find(|state| state.source_entity_index == peer_slot)
        {
            state.word_0x32 = 0;
        }
    }

    /// The sixteen constructor children remain action 232. `sub_29A90` is a
    /// model-27 action-233 callback and must not run on this carrier, whether
    /// the child is reached from the root chain or exposed by the arena.
    pub(crate) fn step_authored_class5_model3_child_slot(&mut self, source_slot: u16) -> bool {
        self.authored_class5_model3_children.iter().any(|child| {
            child.active && child.source_entity_index == source_slot && child.model == 3
        })
    }

    /// Dispatch the model-3 family through the same source cursor bookkeeping
    /// used by the shared class-5 entry point. This bridge is needed because
    /// the legacy class-5 module only selects action 25 at its older seam.
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
}
