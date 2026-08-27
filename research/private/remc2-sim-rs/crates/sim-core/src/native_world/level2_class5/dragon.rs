//! Authored class-5/model-0 Dragon construction, body following, damage
//! propagation, roaming, and player attack.
//!
//! Dragons use the shared class-5 root and NewEvent arena. This module owns
//! only the model-0 constructor sidecar, its sixteen action-232 body rows,
//! and the source-shaped undamaged action-1 callback reached by Level 3.

use super::super::*;
use crate::goat::GoatPlayerTarget;

pub(crate) const DRAGON_MODEL: u8 = 0;
pub(crate) const DRAGON_ROAM_ACTION: u8 = 1;
pub(crate) const DRAGON_ATTACK_ACTION: u8 = 2;
pub(crate) const DRAGON_PART_ACTION: u8 = 232;
pub(crate) const DRAGON_PART_COUNT: usize = 16;
pub(crate) const DRAGON_DESCRIPTOR: u16 = 71;
pub(crate) const DRAGON_ROOT_PARTICLE: u16 = 40;
pub(crate) const DRAGON_MINIMUM_SPEED: i16 = 80;
pub(crate) const DRAGON_MAXIMUM_SPEED: i16 = 16;
pub(crate) const DRAGON_ACTUAL_SPEED: i16 = 30;
pub(crate) const DRAGON_LIFE: i32 = 4_000;
pub(crate) const DRAGON_MAX_MANA: i32 = 4_500;
pub(crate) const DRAGON_INITIAL_MANA: i32 = 2_250;
pub(crate) const DRAGON_SEGMENT_MANA: i32 = DRAGON_MAX_MANA >> 5;
pub(crate) const DRAGON_SUBSPELL_INDEX: u16 = 100;
const DRAGON_DESCRIPTOR_CADENCE: u8 = 30;
const DRAGON_TURN_LIMIT: i16 = 34;
const DRAGON_GROUND_CLEARANCE: i16 = 0;
const DRAGON_DESCENT: i16 = -4;
const DRAGON_MAX_TILE_SLOPE: i32 = 30;
const DRAGON_RANGE: u16 = 5_120;
const DRAGON_VIEW_HALF_ANGLE: i16 = 170;
const DRAGON_RENDER_SPEED: i16 = 342;
const DRAGON_RENDER_ROTATION: i16 = 280;
const DRAGON_FIREBALL_PAYLOAD: u16 = 500;
const DRAGON_PART_SPEEDS: [i16; DRAGON_PART_COUNT] = [
    196, 215, 241, 261, 274, 274, 261, 248, 222, 209, 196, 170, 143, 117, 91, 78,
];
const DRAGON_PART_ROTATIONS: [i16; DRAGON_PART_COUNT] = [
    150, 165, 185, 200, 210, 210, 200, 190, 170, 160, 150, 130, 110, 90, 70, 60,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DragonAttackStep {
    pub(crate) live: bool,
    pub(crate) fireball: Option<DragonFireballLaunch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DragonFireballLaunch {
    pub(crate) owner_entity_index: u16,
    pub(crate) target_entity_index: u16,
    pub(crate) position: Position,
    pub(crate) target_position: Position,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) payload: u16,
}

impl Level2EconomyState {
    /// Reproduce `sub_4B240`: one Dragon root followed by sixteen copied body
    /// rows, then map-link the root after the body chain. The adapter keeps
    /// the constructor transactional so arena exhaustion cannot publish a
    /// partial typed chain.
    pub(crate) fn allocate_authored_dragon(
        &mut self,
        authored_index: u16,
        mut position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        let before = self.clone();
        position.z = terrain.ground_height(position);
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = level2_next_rng16(self.new_event_rng(source_entity_index));
        let heading = i16::try_from(entity_rng & 0x07ff)
            .expect("Dragon heading fits i16")
            .wrapping_sub(1);
        let source_byte_3e = self.class5_model0_constructor_phase;
        self.class5_model0_constructor_phase = source_byte_3e.wrapping_add(1);
        let mut source = Level2AuthoredClass5Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model: DRAGON_MODEL,
            action: DRAGON_ROAM_ACTION,
            minimum_speed: DRAGON_MINIMUM_SPEED,
            maximum_speed: DRAGON_MAXIMUM_SPEED,
            actual_speed: DRAGON_ACTUAL_SPEED,
            max_life: DRAGON_LIFE,
            life: DRAGON_LIFE,
            mana: DRAGON_INITIAL_MANA,
            max_mana: DRAGON_MAX_MANA,
            subspell_index: DRAGON_SUBSPELL_INDEX,
            dword_0x10: i32::from(source_entity_index % 100),
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: DRAGON_ROOT_PARTICLE,
            source_byte_3e,
            source_byte_39: DRAGON_DESCRIPTOR_CADENCE
                .wrapping_sub(source_byte_3e % DRAGON_DESCRIPTOR_CADENCE)
                .wrapping_add(4),
            yaw: heading,
            pitch: heading,
            roll: heading,
            fov: 0,
            shift_rot_x: DRAGON_RENDER_SPEED / 2,
            shift_rot_y: DRAGON_RENDER_SPEED / 2,
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };

        let mut previous_entity_index = source_entity_index;
        let mut first_part = 0;
        for part_index in 0..DRAGON_PART_COUNT {
            let Some(part_slot) = self.allocate_slot() else {
                *self = before;
                return None;
            };
            let speed = DRAGON_PART_SPEEDS[part_index];
            let rotation = DRAGON_PART_ROTATIONS[part_index];
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: part_slot,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .is_err()
            {
                *self = before;
                return None;
            }
            let part = AuthoredDragonPart {
                source_entity_index: part_slot,
                entity_id: source_entity_index,
                entity_rng,
                // The body row is a raw root copy. `sub_4B240` uses
                // word_0x32/word_0x34 for the chain and leaves the copied
                // parentId at the root's NewEvent default of zero.
                parent_entity_index: 0,
                previous_entity_index,
                next_entity_index: 0,
                word_0x36: speed / 2,
                pending_word_0x62: 0,
                pending_damage: 0,
                word_0x26: 0,
                model: DRAGON_MODEL,
                action: DRAGON_PART_ACTION,
                source_byte_3e: u8::try_from(part_index).expect("Dragon part index fits u8"),
                source_byte_39: source.source_byte_39,
                max_life: DRAGON_LIFE,
                life: DRAGON_LIFE,
                // `sub_4B240` copies the root before replacing the root's
                // mana with max-mana/32. The first segment therefore keeps
                // the initial half-mana value; later segments copy 140.
                mana: source.mana,
                subspell_index: source.subspell_index,
                particle_index: u16::try_from(part_index)
                    .expect("Dragon part index fits u16")
                    .wrapping_add(19),
                yaw: heading,
                pitch: heading,
                roll: heading,
                fov: 0,
                render_yaw: rotation / 2,
                render_pitch: speed / 2,
                render_roll: speed / 2,
                render_fov: rotation / 2,
                position,
                drawing_enabled: true,
                active: true,
            };
            if first_part == 0 {
                first_part = part_slot;
            }
            if previous_entity_index != source_entity_index
                && let Some(previous) = self
                    .authored_dragon_parts
                    .iter_mut()
                    .find(|previous| previous.source_entity_index == previous_entity_index)
            {
                previous.next_entity_index = part_slot;
            }
            self.source_rng.insert(part_slot, entity_rng);
            self.source_owner.insert(part_slot, 0);
            self.authored_dragon_parts.push(part);
            source.mana = DRAGON_SEGMENT_MANA;
            previous_entity_index = part_slot;
        }
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
            *self = before;
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_owner.insert(source_entity_index, 0);
        self.authored_class5.push(source);
        self.authored_class5_runtime
            .push(Level2AuthoredClass5RuntimeState {
                source_entity_index,
                byte_0x38: 1,
                xtype: 3,
                descriptor_index: DRAGON_DESCRIPTOR,
                pending_damage: 0,
                linked_head: first_part,
            });
        self.authored_dragons.push(AuthoredDragonRuntime {
            source_entity_index,
            linked_entity_index: 0,
            word_0x36: 96,
            control_flags: 8,
            font_type_index: 0,
            render_yaw: DRAGON_RENDER_ROTATION / 2,
            render_pitch: DRAGON_RENDER_SPEED / 2,
            render_roll: DRAGON_RENDER_SPEED / 2,
            render_fov: DRAGON_RENDER_ROTATION / 2,
            movement_blocked: false,
        });
        Some(source)
    }

    /// Run the undamaged `sub_1EF40` action-1 callback: ordinary creature
    /// motion/roam, the initially inert font tail, then Dragon vertical
    /// motion. Damage and successor actions remain visibly unsupported until
    /// their own package-reached callbacks are implemented.
    pub(crate) fn step_authored_dragon_roam_slot(
        &mut self,
        source_slot: u16,
        players: &[GoatPlayerTarget],
        frozen_model0_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> std::result::Result<bool, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
                && source.action == DRAGON_ROAM_ACTION
        }) else {
            return Ok(false);
        };
        let Some(state_index) = self
            .authored_dragons
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return Err(format!(
                "Dragon source slot {source_slot} has no runtime sidecar"
            ));
        };
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|runtime| runtime.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no class-5 runtime"))?;

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_dragons[state_index];
        let mut runtime = self.authored_class5_runtime[runtime_index];
        if !consume_dragon_damage(
            &mut source,
            &mut runtime,
            &self.authored_dragon_parts,
            terrain,
        ) {
            step_dragon_motion(&mut source, &mut state, terrain);

            if source.source_byte_3e % DRAGON_DESCRIPTOR_CADENCE == 0 {
                apply_dragon_wander(&mut source);
                if source.source_byte_39 != 0 {
                    if let Some(target) = nearest_dragon_player(source, players) {
                        source.word_0x96_150 = target;
                        source.action = 2;
                    } else if let Some(peer) = nearest_unlinked_dragon(
                        source,
                        frozen_model0_slots,
                        &self.authored_class5,
                        &self.authored_dragons,
                    ) {
                        state.linked_entity_index = peer;
                        source.action = 3;
                    }
                }
            }
        }

        step_dragon_tails(&mut source, state, terrain);

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_class5_runtime[runtime_index] = runtime;
        self.authored_dragons[state_index] = state;
        Ok(self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok())
    }

    /// Run undamaged `sub_1EF70 -> sub_1C310(..., sub_1CC20)`. The callback
    /// owns movement, target validation, aiming, range/cadence, and the launch
    /// intent. The campaign adapter owns the shared NewEvent allocation and
    /// publishes sound 8 only when that allocation succeeds.
    pub(crate) fn step_authored_dragon_attack_slot(
        &mut self,
        source_slot: u16,
        target: Option<GoatPlayerTarget>,
        frozen_model0_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> std::result::Result<DragonAttackStep, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
                && source.action == DRAGON_ATTACK_ACTION
        }) else {
            return Ok(DragonAttackStep::default());
        };
        let state_index = self
            .authored_dragons
            .iter()
            .position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no runtime sidecar"))?;
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|runtime| runtime.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no class-5 runtime"))?;

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_dragons[state_index];
        let mut fireball = None;
        let mut runtime = self.authored_class5_runtime[runtime_index];
        if !consume_dragon_damage(
            &mut source,
            &mut runtime,
            &self.authored_dragon_parts,
            terrain,
        ) {
            step_dragon_motion(&mut source, &mut state, terrain);

            let target = target.filter(|target| {
                target.entity_index == source.word_0x96_150
                    && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            });
            if let Some(target) = target {
                if source.source_byte_3e & 3 == 0 {
                    source.roll = i16::try_from(crate::heading_for_positions(
                        source.position,
                        target.position,
                    ))
                    .expect("Dragon target heading fits i16");
                    if let Some(peer) =
                        first_close_dragon(source, frozen_model0_slots, &self.authored_class5)
                    {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            peer.position,
                            source.position,
                        ))
                        .expect("Dragon separation heading fits i16");
                    }
                }
                if source.source_byte_3e % DRAGON_DESCRIPTOR_CADENCE == 0 {
                    if distance_3d_wrapped(source.position, target.position)
                        >= u32::from(DRAGON_RANGE)
                    {
                        source.action = DRAGON_ROAM_ACTION;
                    } else {
                        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
                            source.position,
                            target.position,
                        );
                        let mut launch_position = source.position;
                        launch_position.z =
                            launch_position.z.wrapping_add(DRAGON_RENDER_ROTATION / 2);
                        fireball = Some(DragonFireballLaunch {
                            owner_entity_index: source.source_entity_index,
                            target_entity_index: target.entity_index,
                            position: launch_position,
                            target_position: target.position,
                            yaw,
                            pitch,
                            payload: DRAGON_FIREBALL_PAYLOAD,
                        });
                    }
                }
            } else {
                source.action = DRAGON_ROAM_ACTION;
            }
        }

        step_dragon_tails(&mut source, state, terrain);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_class5_runtime[runtime_index] = runtime;
        self.authored_dragons[state_index] = state;
        let live = self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok();
        Ok(DragonAttackStep {
            live,
            fireball: if live { fireball } else { None },
        })
    }

    #[cfg(test)]
    pub(crate) fn stage_authored_dragon_attack_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        position: Position,
        phase: u8,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
        }) else {
            return false;
        };
        source.position = position;
        source.source_byte_3e = phase;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_ok()
    }

    /// Submit one `sub_10C80(a2 = 0)` mailbox write to a live Dragon root or
    /// body segment. The target consumes this mailbox only on its own numeric
    /// source visit.
    pub(crate) fn submit_authored_dragon_damage(
        &mut self,
        target_slot: u16,
        damage: i32,
        attacker: u16,
    ) -> bool {
        if damage <= 0 || attacker == 0 || !self.map_chains.is_linked(target_slot) {
            return false;
        }
        if let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.drawing_enabled
                && source.life >= 0
                && source.source_entity_index == target_slot
                && source.source_entity_index != attacker
                && source.model == DRAGON_MODEL
        }) {
            let Some(runtime) = self
                .authored_class5_runtime
                .iter_mut()
                .find(|runtime| runtime.source_entity_index == target_slot)
            else {
                return false;
            };
            let source = &mut self.authored_class5[source_index];
            runtime.pending_damage = if source.pending_word_0x62 == 0 {
                damage
            } else {
                runtime.pending_damage.wrapping_add(damage)
            };
            source.pending_word_0x62 = attacker;
            return true;
        }
        let Some(part) = self.authored_dragon_parts.iter_mut().find(|part| {
            part.active
                && part.drawing_enabled
                && part.life >= 0
                && part.source_entity_index == target_slot
                && part.source_entity_index != attacker
                && part.model == DRAGON_MODEL
        }) else {
            return false;
        };
        part.pending_damage = if part.pending_word_0x62 == 0 {
            damage
        } else {
            part.pending_damage.wrapping_add(damage)
        };
        part.pending_word_0x62 = attacker;
        true
    }

    /// Run the action-232 Dragon-body callback. The segment follows its
    /// `word_0x32` predecessor first, then consumes its private damage
    /// mailbox. The scheduler remains the sole byte-0x3E epilogue owner.
    pub(crate) fn step_authored_dragon_part_slot(&mut self, source_slot: u16) -> bool {
        let Some(part_index) = self.authored_dragon_parts.iter().position(|part| {
            part.active
                && part.source_entity_index == source_slot
                && part.model == DRAGON_MODEL
                && part.action == DRAGON_PART_ACTION
        }) else {
            return false;
        };
        let previous_slot = self.authored_dragon_parts[part_index].previous_entity_index;
        let previous_position = self
            .authored_class5
            .iter()
            .find(|root| {
                root.active
                    && root.model == DRAGON_MODEL
                    && root.source_entity_index == previous_slot
            })
            .map(|root| root.position)
            .or_else(|| {
                self.authored_dragon_parts
                    .iter()
                    .find(|part| {
                        part.active
                            && part.model == DRAGON_MODEL
                            && part.source_entity_index == previous_slot
                    })
                    .map(|part| part.position)
            });

        let part = &mut self.authored_dragon_parts[part_index];
        let Some(previous_position) = previous_position else {
            part.drawing_enabled = false;
            part.active = false;
            return true;
        };
        if part.source_byte_39 != 0 {
            let (yaw, pitch) =
                crate::level1_mission::projectile_guidance_angles(part.position, previous_position);
            part.yaw = yaw;
            part.pitch = pitch;
            let mut position = previous_position;
            crate::move_entity(&mut position, yaw, pitch, part.word_0x36.wrapping_neg());
            part.position = position;
            if part.pending_word_0x62 != 0 {
                part.life = part.life.wrapping_sub(part.pending_damage);
                part.word_0x26 = part.pending_word_0x62;
                part.pending_word_0x62 = 0;
                part.pending_damage = 0;
            } else {
                part.word_0x26 = 0;
            }
        } else if part.source_byte_3e & 3 == 0 {
            part.position = previous_position;
        }
        self.source_rng.insert(source_slot, part.entity_rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: part.drawing_enabled,
                x: part.position.x,
                y: part.position.y,
            })
            .is_ok()
    }

    #[cfg(test)]
    pub(crate) fn stage_authored_dragon_part_position_for_test(
        &mut self,
        source_slot: u16,
        position: Position,
    ) -> bool {
        let Some(part) = self
            .authored_dragon_parts
            .iter_mut()
            .find(|part| part.active && part.source_entity_index == source_slot)
        else {
            return false;
        };
        part.position = position;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_ok()
    }
}

fn step_dragon_motion(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredDragonRuntime,
    terrain: &SessionTerrain,
) {
    let transition = crate::ground_creature_motion::step_ground_creature_motion(
        source.position,
        source.yaw,
        source.roll,
        source.actual_speed,
        terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            ground_clearance: DRAGON_GROUND_CLEARANCE,
            descent: DRAGON_DESCENT,
            turn_limit: DRAGON_TURN_LIMIT,
            terrain_mask: u32::MAX,
            maximum_tile_slope: DRAGON_MAX_TILE_SLOPE,
        },
    );
    source.position = transition.position;
    source.yaw = transition.yaw;
    state.movement_blocked = transition.movement_blocked;
    if transition.killed {
        source.life = -1;
    }
}

/// Consume the common creature mailbox/child-chain prefix shared by Dragon
/// actions 1 and 2. A damaged body segment is observed only when the root's
/// later numeric visit reaches this prefix.
fn consume_dragon_damage(
    source: &mut Level2AuthoredClass5Source,
    runtime: &mut Level2AuthoredClass5RuntimeState,
    parts: &[AuthoredDragonPart],
    terrain: &SessionTerrain,
) -> bool {
    let mut damaged = false;
    if source.pending_word_0x62 != 0 {
        source.life = source.life.wrapping_sub(runtime.pending_damage);
        source.word_0x26 = source.pending_word_0x62;
        source.pending_word_0x62 = 0;
        runtime.pending_damage = 0;
        damaged = true;
    } else {
        source.word_0x26 = 0;
    }

    let mut child_slot = runtime.linked_head;
    while child_slot != 0 {
        let Some(child) = parts
            .iter()
            .find(|part| part.source_entity_index == child_slot)
        else {
            break;
        };
        if child.life < source.life {
            source.life = child.life;
            source.word_0x26 = child.word_0x26;
            damaged = true;
            break;
        }
        child_slot = child.next_entity_index;
    }

    if !damaged {
        return false;
    }
    if source.life < 0 {
        source.word_0x24 = source.word_0x26;
        source.action = 4;
    } else {
        source.word_0x96_150 = source.word_0x26;
        source.action = DRAGON_ATTACK_ACTION;
        let ground = terrain.ground_height(source.position);
        if source.position.z > ground {
            source.position.z = source.position.z.wrapping_sub(4);
        }
        if source.position.z <= ground {
            source.position.z = ground;
        }
    }
    true
}

fn step_dragon_tails(
    source: &mut Level2AuthoredClass5Source,
    state: AuthoredDragonRuntime,
    terrain: &SessionTerrain,
) {
    // `sub_1F0C0` is inert while the constructor-owned font counter is 0.
    debug_assert_eq!(state.font_type_index, 0);
    source.position.z = source.position.z.wrapping_add(source.dword_0x10 as i16);
    source.dword_0x10 = source.dword_0x10.wrapping_sub(5);
    let ground = terrain.ground_height(source.position);
    if source.position.z < ground.wrapping_add(256) {
        source.dword_0x10 = 150;
    } else if let Some(ceiling) = terrain.ceiling_height(source.position)
        && source.position.z > ceiling.wrapping_sub(256)
    {
        source.dword_0x10 = -150;
    }
}

fn first_close_dragon(
    source: Level2AuthoredClass5Source,
    frozen_model0_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
) -> Option<Level2AuthoredClass5Source> {
    frozen_model0_slots
        .iter()
        .find_map(|slot| {
            roots.iter().find(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.source_entity_index == *slot
                    && peer.source_entity_index != source.source_entity_index
                    && peer.model == DRAGON_MODEL
                    && i32::from(
                        peer.position
                            .x
                            .wrapping_sub(source.position.x)
                            .cast_signed(),
                    )
                    .abs()
                        < i32::from(DRAGON_RENDER_SPEED / 2)
                    && i32::from(
                        peer.position
                            .y
                            .wrapping_sub(source.position.y)
                            .cast_signed(),
                    )
                    .abs()
                        < i32::from(DRAGON_RENDER_SPEED / 2)
            })
        })
        .copied()
}

fn distance_3d_wrapped(left: Position, right: Position) -> u32 {
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

fn apply_dragon_wander(source: &mut Level2AuthoredClass5Source) {
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let direction = source.entity_rng;
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let sign = if direction % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((source.entity_rng & 0x00ff) + 85).expect("Dragon wander delta fits i16");
    source.roll = source.roll.wrapping_add(sign * delta) & 0x07ff;
}

fn nearest_dragon_player(
    source: Level2AuthoredClass5Source,
    players: &[GoatPlayerTarget],
) -> Option<u16> {
    let range_squared = u32::from(DRAGON_RANGE).pow(2);
    players
        .iter()
        .filter(|player| player.control_flags & 0x20 == 0)
        .filter_map(|player| {
            let distance = squared_wrapping_xy_distance(source.position, player.position);
            let heading = crate::heading_for_positions(source.position, player.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < DRAGON_VIEW_HALF_ANGLE)
                .then_some((distance, player.entity_index))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, entity_index)| entity_index)
}

fn nearest_unlinked_dragon(
    source: Level2AuthoredClass5Source,
    frozen_model0_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    dragons: &[AuthoredDragonRuntime],
) -> Option<u16> {
    let range_squared = u32::from(DRAGON_RANGE).pow(2);
    frozen_model0_slots
        .iter()
        .filter_map(|slot| {
            let peer = roots.iter().find(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.source_entity_index == *slot
                    && peer.source_entity_index != source.source_entity_index
                    && peer.model == DRAGON_MODEL
                    && dragons
                        .iter()
                        .find(|state| state.source_entity_index == *slot)
                        .is_some_and(|state| state.linked_entity_index == 0)
            })?;
            let distance = squared_wrapping_xy_distance(source.position, peer.position);
            let heading = crate::heading_for_positions(source.position, peer.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < DRAGON_VIEW_HALF_ANGLE)
                .then_some((distance, *slot))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, slot)| slot)
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}
