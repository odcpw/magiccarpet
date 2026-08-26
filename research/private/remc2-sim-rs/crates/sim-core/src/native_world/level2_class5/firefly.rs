//! Authored class-5/model-19 Firefly construction, patrol, roaming, and charge.
//!
//! The ordinary undamaged actions 159, 153, and 155 and action-154's orbit and
//! charge branch are implemented here. Fireball emission, charge contact,
//! Firefly damage, death conversion, and reclaim remain fail-closed until
//! their source lifecycles are implemented.

use super::super::*;
use crate::goat::GoatPlayerTarget;

pub(crate) const FIREFLY_MODEL: u8 = 19;
pub(crate) const FIREFLY_ROAM_ACTION: u8 = 153;
pub(crate) const FIREFLY_ATTACK_ACTION: u8 = 154;
pub(crate) const FIREFLY_LINKED_ACTION: u8 = 155;
pub(crate) const FIREFLY_STAGE_ACTION: u8 = 159;
pub(crate) const FIREFLY_MINIMUM_SPEED: i16 = 76;
pub(crate) const FIREFLY_MAXIMUM_SPEED: i16 = 8;
pub(crate) const FIREFLY_LIFE: i32 = 600;
pub(crate) const FIREFLY_MANA: i32 = 300;
pub(crate) const FIREFLY_SUBSPELL: u16 = 300;
pub(crate) const FIREFLY_DESCRIPTOR: u16 = 88;
pub(crate) const FIREFLY_PARTICLE: u16 = 287;
pub(crate) const FIREFLY_SHIFT_X: i16 = 85;
pub(crate) const FIREFLY_SHIFT_Y: i16 = 51;
const FIREFLY_DESCRIPTOR_CADENCE: u8 = 35;
const FIREFLY_TURN_LIMIT: i16 = 113;
const FIREFLY_GROUND_CLEARANCE: i16 = 51;
const FIREFLY_DESCENT: i16 = -8;
const FIREFLY_MAX_TILE_SLOPE: i32 = 20;
const FIREFLY_STAGE_REACH: i32 = 2_048;
const FIREFLY_SEPARATION: i32 = 85;
const FIREFLY_LINKED_SEPARATION: i32 = 256;
const FIREFLY_RANGE: u16 = 5_120;
const FIREFLY_VIEW_HALF_ANGLE: i16 = 682;
const FIREFLY_ORBIT_RADIUS: i16 = 2_048;
const FIREFLY_ORBIT_REACHED_DISTANCE: u32 = 1_280;
const FIREFLY_CHARGE_DURATION: i32 = 24;
const FIREFLY_CHARGE_CONTACT_RANGE: u32 = 768;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireflyAttackTarget {
    pub(crate) entity_index: u16,
    pub(crate) position: Position,
    pub(crate) yaw: i16,
    pub(crate) live: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FireflyAttackStep {
    pub(crate) live: bool,
    pub(crate) sound_id: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireflyStageRoute {
    pub(crate) stage_var1: u8,
    pub(crate) stage_var2: u8,
    pub(crate) target: Position,
}

impl Level2EconomyState {
    pub(crate) fn allocate_authored_firefly(
        &mut self,
        authored_index: u16,
        position: Position,
        route: FireflyStageRoute,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let entity_rng = level2_next_rng16(initial_rng);
        let source_byte_3e = self.class5_model19_constructor_phase;
        self.class5_model19_constructor_phase = source_byte_3e.wrapping_add(1);
        let heading = i16::try_from(entity_rng & 0x07ff)
            .expect("Firefly heading fits i16")
            .wrapping_sub(1);
        let mut position = position;
        position.z = terrain.ground_height(position);
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
        // `NewEvent_4A050` zeroes the raw player/source-owner relation. A
        // reclaimed slot may still have a prior carrier entry in this typed
        // arena, so clear it before publishing the new Firefly row.
        self.source_owner.insert(source_entity_index, 0);
        let source = Level2AuthoredClass5Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model: FIREFLY_MODEL,
            action: FIREFLY_STAGE_ACTION,
            minimum_speed: FIREFLY_MINIMUM_SPEED,
            maximum_speed: FIREFLY_MAXIMUM_SPEED,
            actual_speed: FIREFLY_MINIMUM_SPEED,
            max_life: FIREFLY_LIFE,
            life: FIREFLY_LIFE,
            mana: FIREFLY_MANA,
            max_mana: 0,
            subspell_index: FIREFLY_SUBSPELL,
            dword_0x10: i32::from(source_entity_index % 100),
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: FIREFLY_PARTICLE,
            source_byte_3e,
            source_byte_39: FIREFLY_DESCRIPTOR_CADENCE
                .wrapping_sub(source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE)
                .wrapping_add(4),
            yaw: heading,
            pitch: heading,
            roll: heading,
            fov: 0,
            shift_rot_x: FIREFLY_SHIFT_X,
            shift_rot_y: FIREFLY_SHIFT_Y,
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_class5.push(source);
        self.authored_fireflies.push(AuthoredFireflyRuntime {
            source_entity_index,
            linked_entity_index: 0,
            control_flags: 8,
            byte_0x38: 1,
            xtype: 3,
            descriptor_index: FIREFLY_DESCRIPTOR,
            stage_var1: route.stage_var1,
            stage_var2: route.stage_var2,
            stage_target: route.target,
            movement_blocked: false,
        });
        Some(source)
    }

    pub(crate) fn advance_authored_firefly_stage_rows(&mut self) {
        let reached = self
            .authored_fireflies
            .iter()
            .filter_map(|state| {
                let source = self.authored_class5.iter().find(|source| {
                    source.active
                        && source.source_entity_index == state.source_entity_index
                        && source.model == FIREFLY_MODEL
                        && source.action == FIREFLY_STAGE_ACTION
                })?;
                let dx = i32::from(
                    state
                        .stage_target
                        .x
                        .wrapping_sub(source.position.x)
                        .cast_signed(),
                )
                .abs();
                let dy = i32::from(
                    state
                        .stage_target
                        .y
                        .wrapping_sub(source.position.y)
                        .cast_signed(),
                )
                .abs();
                (state.stage_var1 != 0
                    && state.stage_var2 == 1
                    && dx <= FIREFLY_STAGE_REACH
                    && dy <= FIREFLY_STAGE_REACH)
                    .then_some(source.source_entity_index)
            })
            .collect::<Vec<_>>();
        for source_slot in reached {
            if let Some(source) = self
                .authored_class5
                .iter_mut()
                .find(|source| source.source_entity_index == source_slot)
            {
                source.action = FIREFLY_ROAM_ACTION;
            }
            if let Some(state) = self
                .authored_fireflies
                .iter_mut()
                .find(|state| state.source_entity_index == source_slot)
            {
                state.stage_var1 = 0;
                state.stage_var2 = 0;
            }
        }
    }

    pub(crate) fn step_authored_firefly_stage_slot(
        &mut self,
        source_slot: u16,
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_STAGE_ACTION
        }) else {
            return false;
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        step_firefly_motion(&mut source, &mut state, terrain);

        if source.source_byte_3e & 7 == 0 {
            if !state.movement_blocked {
                source.roll = i16::try_from(crate::heading_for_positions(
                    source.position,
                    state.stage_target,
                ))
                .expect("Firefly stage heading fits i16");
                if source.source_byte_3e & 0x3f == 0 {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let direction = source.entity_rng;
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let sign = if direction % 157 < 79 { -1 } else { 1 };
                    let delta = i16::try_from((source.entity_rng & 0x00ff) + 85)
                        .expect("Firefly wander delta fits i16");
                    source.roll = source.roll.wrapping_add(sign * delta) & 0x07ff;
                }
            }
            if let Some(peer) = frozen_model19_slots.iter().find_map(|peer_slot| {
                self.authored_class5.iter().find(|peer| {
                    peer.active
                        && peer.life >= 0
                        && peer.model == FIREFLY_MODEL
                        && peer.source_entity_index == *peer_slot
                        && peer.source_entity_index != source_slot
                        && i32::from(
                            peer.position
                                .x
                                .wrapping_sub(source.position.x)
                                .cast_signed(),
                        )
                        .abs()
                            < FIREFLY_SEPARATION
                        && i32::from(
                            peer.position
                                .y
                                .wrapping_sub(source.position.y)
                                .cast_signed(),
                        )
                        .abs()
                            < FIREFLY_SEPARATION
                })
            }) {
                source.roll =
                    i16::try_from(crate::heading_for_positions(peer.position, source.position))
                        .expect("Firefly separation heading fits i16");
            }
        }

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(crate) fn step_authored_firefly_roam_slot(
        &mut self,
        source_slot: u16,
        players: &[GoatPlayerTarget],
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_ROAM_ACTION
        }) else {
            return false;
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        step_firefly_motion(&mut source, &mut state, terrain);

        if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0 {
            apply_firefly_wander(&mut source);
            if source.source_byte_39 != 0 {
                let range_squared = u32::from(FIREFLY_RANGE).pow(2);
                if let Some(target) = players
                    .iter()
                    .filter(|target| target.control_flags & 0x20 == 0)
                    .filter_map(|target| {
                        let distance =
                            squared_wrapping_xy_distance(source.position, target.position);
                        let heading =
                            crate::heading_for_positions(source.position, target.position);
                        (distance <= range_squared
                            && crate::angle_distance(i32::from(source.yaw), heading)
                                < FIREFLY_VIEW_HALF_ANGLE)
                            .then_some((distance, target))
                    })
                    .min_by_key(|(distance, _)| *distance)
                    .map(|(_, target)| target)
                {
                    source.word_0x96_150 = target.entity_index;
                    source.byte_0x46_70 = 0;
                    state.linked_entity_index = 0;
                    source.action = 154;
                } else if let Some(peer_slot) = nearest_unlinked_firefly(
                    source,
                    frozen_model19_slots,
                    &self.authored_class5,
                    &self.authored_fireflies,
                ) {
                    state.linked_entity_index = peer_slot;
                    source.action = FIREFLY_LINKED_ACTION;
                }
            }
        }

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(crate) fn step_authored_firefly_linked_slot(
        &mut self,
        source_slot: u16,
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_LINKED_ACTION
        }) else {
            return false;
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        if state.linked_entity_index == 0 {
            source.action = FIREFLY_ROAM_ACTION;
            self.authored_class5[source_index] = source;
            return true;
        }
        let peer = self.authored_class5.iter().copied().find(|peer| {
            peer.source_entity_index == state.linked_entity_index
                && peer.active
                && peer.drawing_enabled
                && peer.life >= 0
                && peer.model == FIREFLY_MODEL
        });
        step_firefly_motion(&mut source, &mut state, terrain);
        if let Some(peer) = peer {
            if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0 {
                match peer.action.wrapping_sub(152) {
                    0 | 1 => {}
                    2 => {
                        source.word_0x96_150 = peer.word_0x96_150;
                        source.byte_0x46_70 = 0;
                        state.linked_entity_index = 0;
                        source.action = 154;
                    }
                    3 => {
                        state.linked_entity_index = self
                            .authored_fireflies
                            .iter()
                            .find(|state| state.source_entity_index == peer.source_entity_index)
                            .map_or(0, |state| state.linked_entity_index);
                    }
                    _ => leave_firefly_link(&mut source, &mut state),
                }
                if source.action == FIREFLY_LINKED_ACTION {
                    if let Some(linked) = self.authored_class5.iter().find(|candidate| {
                        candidate.source_entity_index == state.linked_entity_index
                    }) {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            source.position,
                            linked.position,
                        ))
                        .expect("Firefly linked heading fits i16");
                        apply_firefly_separation(
                            &mut source,
                            frozen_model19_slots,
                            &self.authored_class5,
                            FIREFLY_LINKED_SEPARATION,
                        );
                        source.actual_speed = peer.actual_speed.wrapping_add(peer.maximum_speed);
                    } else {
                        leave_firefly_link(&mut source, &mut state);
                    }
                }
            }
        } else {
            leave_firefly_link(&mut source, &mut state);
        }

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Undamaged `HitFirebug_25610` orbit and charge branch for a live
    /// class-5/model-19/action-154 row. Substates 4/5 are the separate
    /// Firefly-fireball producer and stay fail-closed until that projectile's
    /// distinct class-9 lifecycle is represented by the shared arena.
    pub(crate) fn step_authored_firefly_charge_slot(
        &mut self,
        source_slot: u16,
        target: Option<FireflyAttackTarget>,
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> std::result::Result<FireflyAttackStep, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_ATTACK_ACTION
        }) else {
            return Ok(FireflyAttackStep::default());
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return Err(format!(
                "Firefly source slot {source_slot} has no runtime sidecar"
            ));
        };

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        step_firefly_motion(&mut source, &mut state, terrain);

        let Some(target) = target.filter(|target| {
            target.live && target.entity_index != 0 && target.entity_index == source.word_0x96_150
        }) else {
            // Source LABEL_92 resets only action and speed. The target word,
            // attack substate, and Firefly peer word are deliberately left
            // untouched here.
            source.action = FIREFLY_ROAM_ACTION;
            source.actual_speed = FIREFLY_MINIMUM_SPEED;
            let live = self.commit_authored_firefly_step(source_index, state_index, source, state);
            return Ok(FireflyAttackStep {
                live,
                ..FireflyAttackStep::default()
            });
        };

        let mut sound_id = None;
        loop {
            match source.byte_0x46_70 {
                0 => {
                    source.actual_speed = FIREFLY_MINIMUM_SPEED;
                    source.byte_0x46_70 = 1;
                }
                1 => {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let orbit_yaw = target.yaw.wrapping_sub(256).wrapping_add(
                        i16::try_from((u32::from(source.entity_rng % 90) << 11) / 360)
                            .expect("Firefly orbit delta fits i16"),
                    ) & 0x07ff;
                    let mut orbit_point = target.position;
                    crate::move_entity(&mut orbit_point, orbit_yaw, 0, FIREFLY_ORBIT_RADIUS);
                    if firefly_distance_3d(source.position, orbit_point)
                        <= FIREFLY_ORBIT_REACHED_DISTANCE
                    {
                        source.byte_0x46_70 = 2;
                    } else {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            source.position,
                            orbit_point,
                        ))
                        .expect("Firefly orbit heading fits i16");
                        if source.source_byte_3e & 3 == 0 {
                            apply_firefly_separation(
                                &mut source,
                                frozen_model19_slots,
                                &self.authored_class5,
                                FIREFLY_SEPARATION,
                            );
                        }
                    }
                    break;
                }
                2 => {
                    source.actual_speed = FIREFLY_MAXIMUM_SPEED;
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    source.dword_0x10 =
                        i32::from(target.position.z) + i32::from(source.entity_rng & 0x03ff);
                    source.byte_0x46_70 = 3;
                }
                3 => {
                    source.roll = i16::try_from(crate::heading_for_positions(
                        source.position,
                        target.position,
                    ))
                    .expect("Firefly target heading fits i16");
                    if source.source_byte_3e & 3 != 0 {
                        break;
                    }
                    apply_firefly_separation(
                        &mut source,
                        frozen_model19_slots,
                        &self.authored_class5,
                        FIREFLY_SEPARATION,
                    );
                    let mut ahead = target.position;
                    crate::move_entity(&mut ahead, target.yaw, 0, FIREFLY_ORBIT_RADIUS);
                    if firefly_distance_3d(source.position, ahead) > FIREFLY_ORBIT_REACHED_DISTANCE
                    {
                        source.byte_0x46_70 = 0;
                        break;
                    }
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let maneuver = source.entity_rng % 287;
                    if maneuver & 0x3f == 0 {
                        source.byte_0x46_70 = 6;
                    }
                    if maneuver & 0x1f == 0 {
                        source.byte_0x46_70 = 7;
                    }
                    if maneuver == 0 {
                        source.byte_0x46_70 = 4;
                    }
                    if maneuver & 3 == 0 {
                        source.position.z = source.position.z.wrapping_add(
                            if i32::from(source.position.z) <= source.dword_0x10 {
                                64
                            } else {
                                -64
                            },
                        );
                    }
                    break;
                }
                4 | 5 => {
                    return Err(format!(
                        "Firefly source slot {source_slot} reached the unimplemented fireball branch substate {}",
                        source.byte_0x46_70
                    ));
                }
                6 => {
                    source.action = FIREFLY_ROAM_ACTION;
                    source.actual_speed = FIREFLY_MINIMUM_SPEED;
                    break;
                }
                7 => {
                    source.actual_speed = FIREFLY_MINIMUM_SPEED.saturating_mul(3);
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    sound_id = Some(43 + (source.entity_rng & 1));
                    source.dword_0x10 = FIREFLY_CHARGE_DURATION;
                    source.byte_0x46_70 = 8;
                }
                8 | 9 => {
                    source.dword_0x10 = source.dword_0x10.wrapping_sub(1);
                    if source.dword_0x10 == 0 {
                        source.byte_0x46_70 = 0;
                        break;
                    }
                    if source.source_byte_3e & 3 == 0 {
                        if source.dword_0x10 > 16 {
                            source.roll = i16::try_from(crate::heading_for_positions(
                                source.position,
                                target.position,
                            ))
                            .expect("Firefly charge heading fits i16");
                        }
                        apply_firefly_separation(
                            &mut source,
                            frozen_model19_slots,
                            &self.authored_class5,
                            FIREFLY_SEPARATION,
                        );
                    }
                    let altitude_delta = i32::from(target.position.z)
                        .wrapping_sub(i32::from(source.position.z))
                        .clamp(-64, 64);
                    source.position.z = source.position.z.wrapping_add(
                        i16::try_from(altitude_delta)
                            .expect("clamped Firefly altitude delta fits i16"),
                    );
                    if source.byte_0x46_70 == 8
                        && firefly_distance_3d(source.position, target.position)
                            < FIREFLY_CHARGE_CONTACT_RANGE
                    {
                        return Err(format!(
                            "Firefly source slot {source_slot} reached the unimplemented charge-contact path"
                        ));
                    }
                    if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0
                        && firefly_distance_3d(source.position, target.position)
                            >= u32::from(FIREFLY_RANGE)
                    {
                        source.byte_0x46_70 = 6;
                    }
                    break;
                }
                substate => {
                    return Err(format!(
                        "Firefly source slot {source_slot} reached unsupported attack substate {substate}"
                    ));
                }
            }
        }

        let live = self.commit_authored_firefly_step(source_index, state_index, source, state);
        Ok(FireflyAttackStep { live, sound_id })
    }

    fn commit_authored_firefly_step(
        &mut self,
        source_index: usize,
        state_index: usize,
        source: Level2AuthoredClass5Source,
        state: AuthoredFireflyRuntime,
    ) -> bool {
        self.source_rng
            .insert(source.source_entity_index, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source.source_entity_index,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Stage only the geometry and cadence prerequisite for the real-package
    /// action-153 -> action-155 production test. Construction, PreDispatch
    /// arrival, numeric dispatch, callback ordering, and epilogues remain on
    /// the ordinary campaign path; this does not claim natural travel time.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_peer_roam_prerequisite_for_test(
        &mut self,
        roam_slot: u16,
        peer_slot: u16,
    ) -> bool {
        let Some(stage_target) = self
            .authored_fireflies
            .iter()
            .find(|state| state.source_entity_index == roam_slot)
            .map(|state| state.stage_target)
        else {
            return false;
        };
        if !self
            .authored_fireflies
            .iter()
            .any(|state| state.source_entity_index == peer_slot)
        {
            return false;
        }

        for state in &mut self.authored_fireflies {
            state.linked_entity_index = 0;
            state.movement_blocked = false;
        }
        let mut remote_ordinal = 0_u16;
        for source in self
            .authored_class5
            .iter_mut()
            .filter(|source| source.model == FIREFLY_MODEL)
        {
            source.actual_speed = FIREFLY_MINIMUM_SPEED;
            source.yaw = 0;
            source.roll = 0;
            match source.source_entity_index {
                slot if slot == roam_slot => {
                    source.action = FIREFLY_STAGE_ACTION;
                    source.source_byte_3e = 70;
                    source.position = stage_target;
                }
                slot if slot == peer_slot => {
                    source.action = FIREFLY_ROAM_ACTION;
                    source.source_byte_3e = 1;
                    source.position = Position {
                        x: stage_target.x.wrapping_add(2_048),
                        ..stage_target
                    };
                }
                _ => {
                    source.action = FIREFLY_STAGE_ACTION;
                    remote_ordinal = remote_ordinal.wrapping_add(1);
                    source.position = Position {
                        y: stage_target
                            .y
                            .wrapping_add(10_000_u16.wrapping_add(remote_ordinal * 1_000)),
                        ..stage_target
                    };
                }
            }
        }
        let seeds = self
            .authored_class5
            .iter()
            .filter(|source| source.model == FIREFLY_MODEL)
            .map(|source| RenderableMapSeed {
                entity_index: source.source_entity_index,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .collect::<Vec<_>>();
        seeds
            .into_iter()
            .all(|seed| self.map_chains.sync_entity(seed).is_ok())
    }

    /// Stage only elapsed source cadence for the action-155 boundary test.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_phase_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        phase: u8,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_LINKED_ACTION
        }) else {
            return false;
        };
        source.source_byte_3e = phase;
        true
    }

    /// Stage only geometry, cadence, and private RNG for the real-package
    /// action-153 player acquisition -> action-154 charge test. The ordinary
    /// scheduler still owns target acquisition, every attack callback, sound,
    /// charge-contact refusal and phase epilogues.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_charge_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        player_position: Position,
        player_yaw: i16,
    ) -> bool {
        if !self.authored_class5.iter().any(|source| {
            source.source_entity_index == source_slot && source.model == FIREFLY_MODEL
        }) || !self
            .authored_fireflies
            .iter()
            .any(|state| state.source_entity_index == source_slot)
        {
            return false;
        }

        let mut position = player_position;
        crate::move_entity(&mut position, player_yaw, 0, FIREFLY_ORBIT_RADIUS);
        let toward_player = player_yaw.wrapping_add(1_024) & 0x07ff;
        let mut remote_ordinal = 0_u16;
        let mut seeds = Vec::new();
        for source in self
            .authored_class5
            .iter_mut()
            .filter(|source| source.model == FIREFLY_MODEL)
        {
            if source.source_entity_index == source_slot {
                source.action = FIREFLY_ROAM_ACTION;
                source.actual_speed = FIREFLY_MINIMUM_SPEED;
                source.entity_rng = 739;
                source.source_byte_3e = 70;
                source.source_byte_39 = 1;
                source.word_0x96_150 = 0;
                source.byte_0x46_70 = 0;
                source.dword_0x10 = 0;
                source.position = position;
                source.yaw = toward_player;
                source.roll = toward_player;
            } else {
                remote_ordinal = remote_ordinal.wrapping_add(1);
                source.position = Position {
                    x: player_position
                        .x
                        .wrapping_add(12_000_u16.wrapping_add(remote_ordinal * 1_000)),
                    y: player_position.y.wrapping_add(12_000),
                    z: player_position.z,
                };
            }
            seeds.push(RenderableMapSeed {
                entity_index: source.source_entity_index,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            });
        }
        for state in &mut self.authored_fireflies {
            state.linked_entity_index = 0;
            state.movement_blocked = false;
            if state.source_entity_index == source_slot {
                state.stage_var1 = 0;
                state.stage_var2 = 0;
            }
        }
        self.source_rng.insert(source_slot, 739);
        seeds
            .into_iter()
            .all(|seed| self.map_chains.sync_entity(seed).is_ok())
    }
}

fn leave_firefly_link(source: &mut Level2AuthoredClass5Source, state: &mut AuthoredFireflyRuntime) {
    source.action = FIREFLY_ROAM_ACTION;
    state.linked_entity_index = 0;
}

fn step_firefly_motion(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredFireflyRuntime,
    terrain: &SessionTerrain,
) {
    let transition = crate::ground_creature_motion::step_ground_creature_motion(
        source.position,
        source.yaw,
        source.roll,
        source.actual_speed,
        terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            ground_clearance: FIREFLY_GROUND_CLEARANCE,
            descent: FIREFLY_DESCENT,
            turn_limit: FIREFLY_TURN_LIMIT,
            terrain_mask: u32::MAX,
            maximum_tile_slope: FIREFLY_MAX_TILE_SLOPE,
        },
    );
    source.position = transition.position;
    source.yaw = transition.yaw;
    state.movement_blocked = transition.movement_blocked;
    if transition.killed {
        source.life = -1;
    }
}

fn apply_firefly_wander(source: &mut Level2AuthoredClass5Source) {
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let direction = source.entity_rng;
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let sign = if direction % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((source.entity_rng & 0x00ff) + 85).expect("Firefly wander delta fits i16");
    source.roll = source.roll.wrapping_add(sign * delta) & 0x07ff;
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

fn firefly_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .cast_unsigned()
            .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
            .wrapping_add(dz.wrapping_mul(dz).cast_unsigned()),
    )
}

fn nearest_unlinked_firefly(
    source: Level2AuthoredClass5Source,
    frozen_model19_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    fireflies: &[AuthoredFireflyRuntime],
) -> Option<u16> {
    let range_squared = u32::from(FIREFLY_RANGE).pow(2);
    frozen_model19_slots
        .iter()
        .filter_map(|slot| {
            let peer = roots.iter().find(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.source_entity_index == *slot
                    && peer.source_entity_index != source.source_entity_index
                    && fireflies
                        .iter()
                        .find(|state| state.source_entity_index == *slot)
                        .is_some_and(|state| state.linked_entity_index == 0)
            })?;
            let distance = squared_wrapping_xy_distance(source.position, peer.position);
            let heading = crate::heading_for_positions(source.position, peer.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < FIREFLY_VIEW_HALF_ANGLE)
                .then_some((distance, *slot))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, slot)| slot)
}

fn apply_firefly_separation(
    source: &mut Level2AuthoredClass5Source,
    frozen_model19_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    separation: i32,
) {
    if let Some(peer) = frozen_model19_slots.iter().find_map(|peer_slot| {
        roots.iter().find(|peer| {
            peer.active
                && peer.life >= 0
                && peer.source_entity_index == *peer_slot
                && peer.source_entity_index != source.source_entity_index
                && i32::from(
                    peer.position
                        .x
                        .wrapping_sub(source.position.x)
                        .cast_signed(),
                )
                .abs()
                    < separation
                && i32::from(
                    peer.position
                        .y
                        .wrapping_sub(source.position.y)
                        .cast_signed(),
                )
                .abs()
                    < separation
        })
    }) {
        source.roll = i16::try_from(crate::heading_for_positions(peer.position, source.position))
            .expect("Firefly separation heading fits i16");
    }
}
