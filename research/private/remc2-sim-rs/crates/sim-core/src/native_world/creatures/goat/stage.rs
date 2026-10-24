//! Source StageVar helpers, borrowing only the Goat family and decoded routes.

use super::{
    DamageResult, GoatFamily, GoatPlayerTarget, GoatStepEvent, SourceEntityView, next_rng,
    relation_index, relax_to_terrain, signed_axis_distance, squared_wrapping_xy_distance,
};
use crate::native_world::{RuntimeRelation, RuntimeStageVariable};
use crate::{FlightWorld, Position, distance_3d_wrapped, heading_for_positions};

#[derive(Clone, Copy, Debug)]
pub(super) enum ExternalEffect {
    Expire { slot: u16, summoned: bool },
    OwnerFeedback { owner: u16, selector: u8 },
}

impl GoatFamily {
    fn stage_point_for(&self, index: usize, variables: &[RuntimeStageVariable]) -> Position {
        variables
            .get(usize::from(self.goats[index].stage_var_1 as u8))
            .and_then(|stage| match stage.relation {
                RuntimeRelation::AxisScalar(point) => Some(Position {
                    x: point as u16,
                    y: (point >> 16) as u16,
                    z: 0,
                }),
                _ => None,
            })
            .unwrap_or(self.stage_point)
    }

    fn stage_entity_for(
        &mut self,
        index: usize,
        variables: &[RuntimeStageVariable],
    ) -> Option<SourceEntityView> {
        let goat = &self.goats[index];
        let stage = variables.get(usize::from(goat.stage_var_1 as u8))?;
        let slot = if stage.flags as u8 & 2 == 0 {
            match stage.relation {
                RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
                _ => 0,
            }
        } else if goat.stage_target != 0 {
            goat.stage_target
        } else {
            let shared = if stage.flags as u8 & 1 != 0 {
                self.source_model_linked_list(1)
                    .into_iter()
                    .find_map(|slot| {
                        let peer = self.source_entity_view(slot)?;
                        (peer.stage_var_1 == goat.stage_var_1 && peer.stage_target != 0)
                            .then_some(peer.stage_target)
                    })
            } else {
                None
            };
            let nearest = shared
                .or_else(|| {
                    let RuntimeRelation::AxisScalar(model) = stage.relation else {
                        return None;
                    };
                    let model = u8::try_from(model).ok()?;
                    let mut nearest = None;
                    let mut distance = u32::MAX;
                    for slot in self.source_model_linked_list(model) {
                        let Some(target) = self.source_entity_view(slot) else {
                            continue;
                        };
                        let candidate =
                            squared_wrapping_xy_distance(goat.position, target.position);
                        if candidate < distance {
                            nearest = Some(slot);
                            distance = candidate;
                        }
                    }
                    nearest
                })
                .unwrap_or(0);
            self.goats[index].stage_target = nearest;
            nearest
        };
        (slot != 0).then(|| self.source_entity_view(slot)).flatten()
    }

    fn same_family_attacker(&self, index: usize, attacker: i16) -> bool {
        self.source_entity_view(attacker.cast_unsigned())
            .is_some_and(|actor| actor.class == 5 && actor.model == 1)
            || self.goats[index].entity_index == attacker.cast_unsigned()
    }

    fn stage_hit<W: FlightWorld>(&mut self, index: usize, world: &W) -> bool {
        match self.apply_action11_damage(index) {
            DamageResult::None => false,
            DamageResult::Dead => {
                self.goats[index].action = 12;
                true
            }
            DamageResult::Hit(attacker) => {
                if !self.same_family_attacker(index, attacker) {
                    self.goats[index].target_entity_index =
                        relation_index(attacker.cast_unsigned());
                    self.goats[index].stage_var_2 = 10;
                    self.goats[index].action = self.engagement_action(index);
                }
                relax_to_terrain(
                    &mut self.goats[index].position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
                true
            }
        }
    }

    fn heading_jitter(&mut self, index: usize) {
        let first = next_rng(self.goats[index].rng);
        let second = next_rng(first);
        let delta = (2 * (i32::from(first % 157) / 79) - 1) * (i32::from(second & 255) + 85);
        self.goats[index].rng = second;
        self.goats[index].roll = self.goats[index].roll.wrapping_add(delta as i16) & 0x7ff;
    }

    fn wander_heading(&mut self, index: usize) {
        if self.goats[index].phase & 15 == 0 && self.goats[index].control_flags & 0x40000 == 0 {
            let seed = next_rng(self.goats[index].rng);
            self.goats[index].rng = seed;
            self.goats[index].roll = self.goats[index]
                .roll
                .wrapping_add((seed % 113 + 142) as i16)
                & 0x7ff;
        }
    }

    /// sub_1DDA0 (point), sub_1D8C0 (entity) and sub_1E1C0 (wander).
    fn stage_movement<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        variables: &[RuntimeStageVariable],
        point: Option<Position>,
        follow: bool,
    ) {
        if self.stage_hit(index, world) {
            return;
        }
        if self.goats[index].behavior_flags & 2 != 0 && point.is_none() && !follow {
            return;
        }
        self.move_action11_source(index, world);
        if self.goats[index].phase & 7 != 0 {
            return;
        }
        let target = if let Some(point) = point {
            Some((point, 0))
        } else if follow {
            self.stage_entity_for(index, variables)
                .map(|actor| (actor.position, actor.half_width))
        } else {
            None
        };
        if let Some((position, half_width)) = target {
            if self.goats[index].control_flags & 0x40000 == 0 {
                self.goats[index].roll =
                    heading_for_positions(self.goats[index].position, position) as i16;
                if self.goats[index].phase & 63 == 0 {
                    self.heading_jitter(index);
                }
            }
            self.apply_peer_separation(index);
            if follow {
                let width =
                    i32::from(self.goats[index].collision_half_width) + i32::from(half_width);
                if signed_axis_distance(self.goats[index].position.x, position.x) < width
                    && signed_axis_distance(self.goats[index].position.y, position.y) < width
                {
                    self.goats[index].roll =
                        heading_for_positions(position, self.goats[index].position) as i16;
                }
            }
        } else {
            self.wander_heading(index);
        }
    }

    pub(super) fn step_stage<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        players: &[GoatPlayerTarget],
        variables: &[RuntimeStageVariable],
        effects: &mut Vec<ExternalEffect>,
        events: &mut Vec<GoatStepEvent>,
    ) {
        match self.goats[index].stage_var_2 {
            1 => {
                let point = self.stage_point_for(index, variables);
                self.stage_movement(index, world, variables, Some(point), false);
            }
            2 => {
                let point = self.stage_point_for(index, variables);
                let position = self.goats[index].position;
                // Source promotes the signed coordinate values before subtracting.
                let far = (i32::from(point.x as i16) - i32::from(position.x as i16)).abs() > 3072
                    || (i32::from(point.y as i16) - i32::from(position.y as i16)).abs() > 3072;
                self.stage_movement(index, world, variables, far.then_some(point), false);
                self.acquire_player_boundary(index, players, events);
            }
            3 | 4 | 5 => {
                let mode = self.goats[index].stage_var_2;
                self.stage_movement(index, world, variables, None, true);
                if mode != 5
                    && self.goats[index].phase & 7 == 0
                    && let Some(target) = self.stage_entity_for(index, variables)
                {
                    let target = if mode == 4 {
                        target
                            .target_entity_index
                            .and_then(|slot| self.source_entity_view(slot))
                    } else {
                        Some(target)
                    };
                    if let Some(target) = target
                        && distance_3d_wrapped(self.goats[index].position, target.position)
                            <= u32::from(self.config.perception_radius)
                    {
                        self.goats[index].target_entity_index = Some(target.entity_index);
                        self.goats[index].stage_var_2 = 10;
                        self.goats[index].action = self.engagement_action(index);
                    }
                }
            }
            6 | 7 | 8 | 9 => self.stage_movement(index, world, variables, None, false),
            10 => self.goats[index].action = self.engagement_action(index),
            12 => {
                let parent = self.source_entity_view(self.goats[index].parent_entity_index);
                if parent.is_none_or(|parent| !parent.active || parent.life < 0) {
                    self.goats[index].control_flags |= 0x400;
                }
                let parent = parent.unwrap_or_else(null_view);
                self.goats[index].position = Position {
                    z: parent.position.z.max(0),
                    ..parent.position
                };
                self.goats[index].yaw = parent.yaw;
                self.goats[index].roll = parent.roll;
            }
            13 | 16 => self.step_summoned(index, world, effects),
            14 => self.step_alliance(index, world, effects),
            17 => {
                self.move_action11_source(index, world);
                if self.goats[index].life < 0 {
                    self.goats[index].control_flags |= 0x400;
                    return;
                }
                let target = self.goats[index]
                    .target_entity_index
                    .and_then(|slot| self.source_entity_view(slot))
                    .filter(|target| {
                        target.active && target.life >= 0 && target.control_flags & 0x400 == 0
                    });
                if let Some(target) = target {
                    self.goats[index].roll =
                        heading_for_positions(self.goats[index].position, target.position) as i16;
                    self.goats[index].actual_speed = self.goats[index].actual_speed.wrapping_sub(8);
                    if self.goats[index].actual_speed > 16 {
                        return;
                    }
                }
                self.goats[index].stage_var_2 = 16;
            }
            // sub_1D5D0 explicitly does nothing for 0, 11, 15 and unknown modes.
            _ => {}
        }
    }

    fn target_movement<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        effects: &mut Vec<ExternalEffect>,
    ) {
        match self.apply_action11_damage(index) {
            DamageResult::Dead => self.goats[index].lifetime = 1,
            DamageResult::Hit(attacker) => {
                self.move_action11_source(index, world);
                if !self.same_family_attacker(index, attacker)
                    && self.goats[index].parent_entity_index != attacker.cast_unsigned()
                {
                    self.goats[index].target_entity_index =
                        relation_index(attacker.cast_unsigned());
                    self.goats[index].action = self.engagement_action(index);
                    effects.push(ExternalEffect::OwnerFeedback {
                        owner: self.goats[index].parent_entity_index,
                        selector: 19,
                    });
                }
            }
            DamageResult::None => {
                self.move_action11_source(index, world);
                if self.goats[index].phase & 7 == 0
                    && let Some(target) = self.goats[index]
                        .target_entity_index
                        .and_then(|slot| self.source_entity_view(slot))
                {
                    if self.goats[index].control_flags & 0x40000 == 0 {
                        self.goats[index].roll =
                            heading_for_positions(self.goats[index].position, target.position)
                                as i16;
                        if self.goats[index].phase & 63 == 0 {
                            self.heading_jitter(index);
                        }
                    }
                    self.apply_peer_separation(index);
                }
            }
        }
    }

    fn step_summoned<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        effects: &mut Vec<ExternalEffect>,
    ) {
        let parent = self
            .source_entity_view(self.goats[index].parent_entity_index)
            .filter(|parent| {
                parent.active && parent.life >= 0 && parent.control_flags & 0x400 == 0
            });
        if parent.is_none() {
            self.goats[index].lifetime = 0;
        } else if self.goats[index].stage_var_2 == 13 {
            self.goats[index].lifetime = self.goats[index].lifetime.wrapping_sub(1);
        }
        if self.goats[index].lifetime <= 0 {
            effects.push(ExternalEffect::Expire {
                slot: self.goats[index].entity_index,
                summoned: self.goats[index].stage_var_2 == 13,
            });
            self.goats[index].control_flags |= 0x400;
            return;
        }
        let mut target = self.goats[index]
            .target_entity_index
            .and_then(|slot| self.source_entity_view(slot));
        if target.is_some_and(|target| {
            !target.active || target.life <= 0 || target.control_flags & 0x400 != 0
        }) {
            self.goats[index].target_entity_index = None;
            target = None;
        }
        if target.is_none() && self.goats[index].phase & 7 == 0 {
            let parent = parent.expect("positive summoned lifetime has a live parent");
            let parent_position = parent.position;
            // sub_16FC0: parent-relative nearest hostile actor, using frozen membership.
            let mut distance = u32::MAX;
            let mut candidate = None;
            let members = self.frozen_class3_slots.as_deref().map_or_else(
                || {
                    self.source_entities
                        .iter()
                        .filter(|actor| actor.active && actor.class == 3 && actor.life >= 0)
                        .map(|actor| actor.entity_index)
                        .collect::<Vec<_>>()
                },
                |slots| slots.to_vec(),
            );
            for slot in members {
                let Some(actor) = self.source_entity_view(slot) else {
                    continue;
                };
                if actor.source_id == parent.source_id || !matches!(actor.model, 0 | 1) {
                    continue;
                }
                let d = squared_wrapping_xy_distance(parent_position, actor.position);
                if d < distance {
                    distance = d;
                    candidate = Some(actor.entity_index);
                }
            }
            self.goats[index].target_entity_index = candidate;
            target = candidate.and_then(|slot| self.source_entity_view(slot));
        }
        if let Some(target) = target {
            self.target_movement(index, world, effects);
            if self.goats[index].phase & 7 == 0
                && distance_3d_wrapped(self.goats[index].position, target.position)
                    < u32::from(self.config.perception_radius)
            {
                self.goats[index].action = 10;
            }
        } else {
            self.goats[index].target_entity_index =
                relation_index(self.goats[index].parent_entity_index);
            self.target_movement(index, world, effects);
            self.goats[index].target_entity_index = None;
            self.goats[index].lifetime = self.goats[index].lifetime.wrapping_sub(4);
        }
    }

    fn alliance_flash(&mut self, index: usize, clear: bool) {
        let goat = &self.goats[index];
        let flash = if clear {
            0
        } else if goat.lifetime >= 16 && goat.lifetime <= goat.initial_lifetime.wrapping_sub(16) {
            if goat.phase & 0x1c == 16 {
                goat.phase & 3
            } else {
                0
            }
        } else if goat.phase & 1 != 0 {
            0
        } else {
            2
        };
        let bits = match flash {
            1 | 3 => 0x02000000,
            2 => 0x04000000,
            _ => 0,
        };
        self.goats[index].control_flags = (self.goats[index].control_flags & !0x06000000) | bits;
        let mut child = self.child_links[index];
        let mut remaining = self.goats.len();
        while let Some(slot) = child {
            if remaining == 0 {
                break;
            }
            let Some(index) = self.goats.iter().position(|goat| goat.entity_index == slot) else {
                break;
            };
            self.goats[index].control_flags =
                (self.goats[index].control_flags & !0x06000000) | bits;
            child = self.child_links[index];
            remaining -= 1;
        }
    }

    pub(super) fn alliance_target(&mut self, index: usize) -> Option<u16> {
        let target = self.goats[index].target_entity_index;
        if self.goats[index].stage_var_2 != 14 {
            return target;
        }
        self.alliance_flash(index, false);
        self.goats[index].lifetime = self.goats[index].lifetime.wrapping_sub(1);
        if self.goats[index].lifetime <= 0 {
            return None;
        }
        if let Some(parent) = self.source_entity_view(self.goats[index].parent_entity_index) {
            if target == Some(parent.entity_index) {
                return None;
            }
            let expected = parent
                .target_entity_index
                .or(relation_index(parent.current_attacker.cast_unsigned()));
            if expected.is_some() && expected != target {
                return None;
            }
        }
        target
    }

    fn step_alliance<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        effects: &mut Vec<ExternalEffect>,
    ) {
        self.goats[index].lifetime = self.goats[index].lifetime.wrapping_sub(1);
        if self.goats[index].lifetime > 0 {
            self.alliance_flash(index, false);
        }
        let parent = self
            .source_entity_view(self.goats[index].parent_entity_index)
            .filter(|parent| parent.active && parent.life > 0);
        if self.goats[index].lifetime <= 0 || parent.is_none() {
            self.goats[index].stage_var_2 = 10;
            self.goats[index].target_entity_index = None;
            self.goats[index].parent_entity_index = 0;
            self.goats[index].stage_var_1 = self.goats[index].stage_target as i8;
            self.alliance_flash(index, true);
            return;
        }
        let parent = parent.expect("live Alliance parent");
        let old_target = self.goats[index].target_entity_index;
        self.goats[index].target_entity_index = Some(parent.entity_index);
        self.target_movement(index, world, effects);
        self.goats[index].target_entity_index = parent
            .target_entity_index
            .or(relation_index(parent.current_attacker.cast_unsigned()))
            .or(old_target);
        let target = self.goats[index]
            .target_entity_index
            .and_then(|slot| self.source_entity_view(slot));
        if let Some(target) = target {
            if !target.active
                || target.life <= 0
                || target.control_flags & 0x400 != 0
                || (target.parent_entity_index == self.goats[index].parent_entity_index
                    && target.stage_var_2 == 14)
            {
                self.goats[index].target_entity_index = None;
            } else if distance_3d_wrapped(self.goats[index].position, target.position)
                < u32::from(self.config.perception_radius)
            {
                self.goats[index].action = 10;
                effects.push(ExternalEffect::OwnerFeedback {
                    owner: parent.entity_index,
                    selector: 24,
                });
            }
        }
    }
}

fn null_view() -> SourceEntityView {
    SourceEntityView {
        entity_index: 0,
        source_id: 0,
        class: 0,
        model: 0,
        active: false,
        control_flags: 0,
        life: 0,
        current_attacker: 0,
        kill_credit_entity: 0,
        action: 0,
        position: Position::default(),
        child_relation: None,
        actual_speed: 0,
        maximum_speed: 0,
        behavior_flags: 0,
        partner_entity_index: None,
        target_entity_index: None,
        parent_entity_index: 0,
        yaw: 0,
        roll: 0,
        half_width: 0,
        stage_var_1: 0,
        stage_var_2: 0,
        stage_target: 0,
    }
}
