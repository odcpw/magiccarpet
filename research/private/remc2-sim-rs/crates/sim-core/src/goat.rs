//! Source-ordered Level 1 Goat-family simulation.
//!
//! This module is deliberately bounded to the class-5/model-1 action-15
//! patrol path proven by the private remc2 source and exact-build captures.
//! Unsupported action and `StageVar` branches remain explicit.

use crate::native_world::EntitySnapshot;
use crate::{
    FlightWorld, Position, angle_distance, heading_for_positions, move_entity, turn_toward,
    wrapping_i16,
};

pub const LEVEL_ONE_GOAT_INDICES: [u16; 33] = [
    50, 79, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101,
    102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
];

pub const GOAT_CLASS: u8 = 5;
pub const GOAT_MODEL: u8 = 1;
pub const GOAT_PATROL_ACTION: u8 = 15;
pub const GOAT_PATROL_STAGE_MODE: i8 = 2;
pub const GOAT_RENDER_PARAMETER_INDEX: u16 = 238;
pub const GOAT_BASE_TMAP: u16 = 152;
pub const GOAT_STAGE_POINT: Position = Position {
    x: 13_568,
    y: 8_192,
    z: 0,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoatBehaviorConfig {
    pub turn_clamp: i16,
    pub ground_clearance: i16,
    pub vertical_relaxation: i16,
    pub terrain_threshold: i16,
    pub perception_cadence: u16,
    pub perception_radius: u16,
    pub perception_half_angle: i16,
    pub behavior_flags: u8,
}

impl GoatBehaviorConfig {
    pub const LEVEL_ONE: Self = Self {
        turn_clamp: 45,
        ground_clearance: 0,
        vertical_relaxation: -256,
        terrain_threshold: 20,
        perception_cadence: 32,
        perception_radius: 1_536,
        perception_half_angle: 853,
        behavior_flags: 9,
    };

    /// Decode the consumed fields of exact entity-parameter row 98.
    ///
    /// # Errors
    ///
    /// Returns an error when any source-consumed field differs from the
    /// qualified Level 1 Goat row.
    pub fn from_parameter_record(bytes: &[u8; 34]) -> Result<Self, String> {
        let config = Self {
            turn_clamp: read_i16(bytes, 2),
            ground_clearance: read_i16(bytes, 12),
            vertical_relaxation: read_i16(bytes, 14),
            terrain_threshold: read_i16(bytes, 16),
            perception_cadence: read_u16(bytes, 26),
            perception_radius: read_u16(bytes, 28),
            perception_half_angle: read_i16(bytes, 30),
            behavior_flags: bytes[32],
        };
        if config != Self::LEVEL_ONE {
            return Err(format!(
                "Level 1 Goat parameter row 98 mismatch: {config:?}"
            ));
        }
        Ok(config)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoatPlayerTarget {
    pub entity_index: u16,
    pub position: Position,
    pub control_flags: u32,
}

/// Derive Goat sound 46's source pitch from the current global RNG without
/// mutating that RNG.
#[must_use]
pub fn goat_sound_rate_delta(global_rng: u32, action: u8) -> i16 {
    let random_value = global_rng.wrapping_mul(9_377).wrapping_add(9_439);
    let variation = i16::try_from(random_value % 20).expect("pitch modulo fits i16");
    if action == 14 {
        variation + 10
    } else {
        variation - 10
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoatStepEvent {
    Bleat {
        entity_index: u16,
        sound_id: u16,
    },
    ManaConverted {
        entity_index: u16,
        mana: i32,
    },
    ExtraManaSphereRequested {
        entity_index: u16,
    },
    RemovalQueued {
        entity_index: u16,
    },
    Reclaimed {
        entity_index: u16,
    },
    EngagementBoundaryReached {
        entity_index: u16,
        target_entity_index: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatState {
    pub entity_index: u16,
    pub source_id: u16,
    pub active: bool,
    pub control_flags: u32,
    pub max_life: i32,
    pub life: i32,
    pub pending_damage: i32,
    pub pending_attacker: i16,
    pub kill_credit_entity: i16,
    pub current_attacker: i16,
    pub rng: u16,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub position: Position,
    pub collision_half_width: i16,
    pub render_parameter_index: u16,
    pub animation_frame: i8,
    pub action: u8,
    pub phase: u8,
    pub proximity_countdown: i8,
    pub proximity_subcounter: i8,
    pub stage_var_1: i8,
    pub stage_var_2: i8,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub mana: i32,
    pub target_entity_index: Option<u16>,
}

impl GoatState {
    /// Decode the source fields reachable from the bounded Goat path.
    ///
    /// # Errors
    ///
    /// Returns an error if the record is not the original 168-byte size or is
    /// not a class-5/model-1 Goat.
    pub fn from_record(entity_index: u16, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 0x00a8 {
            return Err(format!(
                "Goat entity {entity_index} must contain 168 bytes, got {}",
                bytes.len()
            ));
        }
        if bytes[0x003f] != GOAT_CLASS || bytes[0x0040] != GOAT_MODEL {
            return Err(format!(
                "entity {entity_index} is {}/{}, not Goat {GOAT_CLASS}/{GOAT_MODEL}",
                bytes[0x003f], bytes[0x0040]
            ));
        }
        let state = Self {
            entity_index,
            source_id: read_u16(bytes, 0x001a),
            active: bytes[0x003f] != 0 && bytes[0x000d] & 4 == 0,
            control_flags: read_u32(bytes, 0x000c),
            max_life: read_i32(bytes, 0x0004),
            life: read_i32(bytes, 0x0008),
            pending_damage: read_i32(bytes, 0x005e),
            pending_attacker: read_i16(bytes, 0x0062),
            kill_credit_entity: read_i16(bytes, 0x0024),
            current_attacker: read_i16(bytes, 0x0026),
            rng: read_u16(bytes, 0x0014),
            yaw: read_i16(bytes, 0x001c),
            pitch: read_i16(bytes, 0x001e),
            roll: read_i16(bytes, 0x0020),
            position: Position {
                x: read_u16(bytes, 0x004c),
                y: read_u16(bytes, 0x004e),
                z: read_i16(bytes, 0x0050),
            },
            collision_half_width: read_i16(bytes, 0x0054),
            render_parameter_index: read_u16(bytes, 0x005a),
            animation_frame: bytes[0x005c].cast_signed(),
            action: bytes[0x0045],
            phase: bytes[0x003e],
            proximity_countdown: bytes[0x0039].cast_signed(),
            proximity_subcounter: bytes[0x003a].cast_signed(),
            stage_var_1: bytes[0x0048].cast_signed(),
            stage_var_2: bytes[0x0049].cast_signed(),
            actual_speed: read_i16(bytes, 0x0082),
            minimum_speed: read_i16(bytes, 0x0084),
            maximum_speed: read_i16(bytes, 0x0086),
            mana: read_i32(bytes, 0x0090),
            target_entity_index: relation_index(read_u16(bytes, 0x0096)),
        };
        if state.render_parameter_index != GOAT_RENDER_PARAMETER_INDEX {
            return Err(format!(
                "Goat entity {entity_index} render parameter must be {GOAT_RENDER_PARAMETER_INDEX}, got {}",
                state.render_parameter_index
            ));
        }
        Ok(state)
    }

    /// Decode a Goat from one validated immutable-world entity.
    ///
    /// # Errors
    ///
    /// Returns the same record-size or identity errors as [`Self::from_record`].
    pub fn from_snapshot(snapshot: &EntitySnapshot) -> Result<Self, String> {
        let bytes = snapshot.record_bytes();
        if bytes.len() != 176 {
            return Err(format!(
                "exported Goat entity {} must contain 176 bytes, got {}",
                snapshot.index,
                bytes.len()
            ));
        }
        if snapshot.class != GOAT_CLASS || snapshot.model != GOAT_MODEL {
            return Err(format!(
                "entity {} is {}/{}, not Goat {GOAT_CLASS}/{GOAT_MODEL}",
                snapshot.index, snapshot.class, snapshot.model
            ));
        }
        let state = Self {
            entity_index: snapshot.index,
            source_id: snapshot.source_id,
            active: snapshot.active,
            control_flags: snapshot.control_flags,
            max_life: read_i32(bytes, 0x0008),
            life: read_i32(bytes, 0x000c),
            pending_damage: read_i32(bytes, 0x0062),
            pending_attacker: read_i16(bytes, 0x0066),
            kill_credit_entity: read_i16(bytes, 0x0028),
            current_attacker: read_i16(bytes, 0x002a),
            rng: read_u16(bytes, 0x0018),
            yaw: snapshot.yaw,
            pitch: snapshot.pitch,
            roll: read_i16(bytes, 0x0024),
            position: snapshot.position,
            collision_half_width: read_i16(bytes, 0x0058),
            render_parameter_index: read_u16(bytes, 0x005e),
            animation_frame: bytes[0x0060].cast_signed(),
            action: snapshot.action,
            phase: snapshot.phase,
            proximity_countdown: bytes[0x003d].cast_signed(),
            proximity_subcounter: bytes[0x003e].cast_signed(),
            stage_var_1: bytes[0x004c].cast_signed(),
            stage_var_2: bytes[0x004d].cast_signed(),
            actual_speed: snapshot.actual_speed,
            minimum_speed: read_i16(bytes, 0x0088),
            maximum_speed: read_i16(bytes, 0x008a),
            mana: snapshot.mana,
            target_entity_index: relation_index(read_u16(bytes, 0x009a)),
        };
        if state.render_parameter_index != GOAT_RENDER_PARAMETER_INDEX {
            return Err(format!(
                "Goat entity {} render parameter must be {GOAT_RENDER_PARAMETER_INDEX}, got {}",
                snapshot.index, state.render_parameter_index
            ));
        }
        Ok(state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatFamily {
    goats: Box<[GoatState]>,
    stage_point: Position,
    config: GoatBehaviorConfig,
    outer_tick: u32,
}

impl GoatFamily {
    /// Construct the complete 33-member family in ascending pool order.
    ///
    /// # Errors
    ///
    /// Returns an error if a required pool row is missing or does not decode
    /// as a Goat.
    pub fn from_entities(
        entities: &[EntitySnapshot],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Result<Self, String> {
        let mut goats = Vec::with_capacity(LEVEL_ONE_GOAT_INDICES.len());
        for expected_index in LEVEL_ONE_GOAT_INDICES {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Goat entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "Goat pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            goats.push(GoatState::from_snapshot(snapshot)?);
        }
        Ok(Self {
            goats: goats.into_boxed_slice(),
            stage_point,
            config,
            outer_tick: 0,
        })
    }

    /// Construct from exact-build record bytes. This is kept separate from
    /// owner-package loading so regression tests cannot silently replace the
    /// production reset source.
    ///
    /// # Errors
    ///
    /// Returns an error if the set is incomplete, out of order, or contains a
    /// row that is not an exact 168-byte Goat record.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = (u16, &'a [u8])>,
    ) -> Result<Self, String> {
        let mut goats = Vec::with_capacity(LEVEL_ONE_GOAT_INDICES.len());
        let mut records = records.into_iter();
        for expected in LEVEL_ONE_GOAT_INDICES {
            let (actual, bytes) = records.next().ok_or_else(|| {
                format!(
                    "Goat family requires {} records, got {}",
                    LEVEL_ONE_GOAT_INDICES.len(),
                    goats.len()
                )
            })?;
            if expected != actual {
                return Err(format!(
                    "Goat regression order mismatch: expected {expected}, got {actual}"
                ));
            }
            goats.push(GoatState::from_record(actual, bytes)?);
        }
        if let Some((surplus_index, _)) = records.next() {
            return Err(format!(
                "Goat family has surplus record {surplus_index} after {} exact rows",
                LEVEL_ONE_GOAT_INDICES.len()
            ));
        }
        Ok(Self {
            goats: goats.into_boxed_slice(),
            stage_point: GOAT_STAGE_POINT,
            config: GoatBehaviorConfig::LEVEL_ONE,
            outer_tick: 0,
        })
    }

    #[must_use]
    pub fn goats(&self) -> &[GoatState] {
        &self.goats
    }

    #[must_use]
    pub const fn outer_tick(&self) -> u32 {
        self.outer_tick
    }

    /// Advance one unpaused source outer turn.
    ///
    /// Only the captured action-15 / StageVar2-2 patrol branch is live. Other
    /// branches remain unchanged until their own source contracts are frozen.
    pub fn step_patrol<W: FlightWorld>(&mut self, world: &W) -> Vec<GoatStepEvent> {
        self.step_with_players(world, &[])
    }

    pub fn step_with_players<W: FlightWorld>(
        &mut self,
        world: &W,
        players: &[GoatPlayerTarget],
    ) -> Vec<GoatStepEvent> {
        let mut events = Vec::new();

        // `UpdateEntities_57730` reclaims drawing-disabled slots before list
        // construction, countdown processing, and entity dispatch.
        for goat in &mut self.goats {
            if goat.active && goat.control_flags & 0x0000_0400 != 0 {
                goat.active = false;
                events.push(GoatStepEvent::Reclaimed {
                    entity_index: goat.entity_index,
                });
            }
        }

        // `sub_68BF0` runs after class/model list construction and before
        // entity dispatch. Its local-player proximity branch is source-ordered
        // before the action-15 perception gate.
        for goat in &mut self.goats {
            if !goat.active {
                continue;
            }
            if goat.proximity_countdown != 0 {
                goat.proximity_countdown = goat.proximity_countdown.wrapping_sub(1);
                continue;
            }
            if goat.control_flags.to_le_bytes()[0] & 1 != 0 {
                continue;
            }
            if goat.proximity_subcounter != 0 {
                goat.proximity_subcounter = goat.proximity_subcounter.wrapping_sub(1);
                continue;
            }
            if let Some(local_player) = players.first()
                && squared_wrapping_xy_distance(goat.position, local_player.position) < 0x0240_0000
            {
                goat.proximity_countdown = 16;
            }
            goat.proximity_subcounter = 0;
        }

        for family_index in 0..self.goats.len() {
            if !self.goats[family_index].active {
                continue;
            }
            match self.goats[family_index].action {
                GOAT_PATROL_ACTION
                    if self.goats[family_index].stage_var_2 == GOAT_PATROL_STAGE_MODE =>
                {
                    self.step_one_patrol(family_index, world, players, &mut events);
                }
                12 => {
                    // `PreKillGoat_1F4F0` selects base action + 5.
                    self.goats[family_index].action = 13;
                }
                13 => self.step_final_kill(family_index, &mut events),
                _ => {
                    // Other handlers are an explicit unsupported boundary.
                    continue;
                }
            }

            self.goats[family_index].phase = self.goats[family_index].phase.wrapping_add(1);
        }
        self.outer_tick = self.outer_tick.wrapping_add(1);
        events
    }

    #[allow(clippy::verbose_bit_mask)]
    fn step_one_patrol<W: FlightWorld>(
        &mut self,
        family_index: usize,
        world: &W,
        players: &[GoatPlayerTarget],
        events: &mut Vec<GoatStepEvent>,
    ) {
        let far_from_point = {
            let goat = &self.goats[family_index];
            signed_axis_distance(goat.position.x, self.stage_point.x) > 3_072
                || signed_axis_distance(goat.position.y, self.stage_point.y) > 3_072
        };

        self.apply_pending_damage(family_index);
        if self.goats[family_index].life < 0 {
            // `sub_1DDA0` selects base action + 4; Goat base is eight.
            self.goats[family_index].action = 12;
            self.advance_action_15_tail(family_index, events);
            return;
        }

        self.move_patrol(family_index, world);

        if far_from_point {
            if self.goats[family_index].phase & 7 == 0 {
                let new_roll =
                    heading_for_positions(self.goats[family_index].position, self.stage_point);
                self.goats[family_index].roll =
                    i16::try_from(new_roll).expect("11-bit heading fits i16");

                if self.goats[family_index].phase & 0x3f == 0 {
                    let first = next_rng(self.goats[family_index].rng);
                    let second = next_rng(first);
                    self.goats[family_index].rng = second;
                    let sign = 2 * (i32::from(first % 157) / 79) - 1;
                    let magnitude = i32::from(second & 0xff) + 85;
                    self.goats[family_index].roll = wrapping_i16(
                        i64::from(self.goats[family_index].roll) + i64::from(sign * magnitude),
                    ) & 0x07ff;
                }

                self.apply_peer_separation(family_index);
            }
        } else if self.goats[family_index].phase & 0x0f == 0
            && self.goats[family_index].control_flags.to_le_bytes()[2] & 4 == 0
        {
            let rng = next_rng(self.goats[family_index].rng);
            self.goats[family_index].rng = rng;
            self.goats[family_index].roll = self.goats[family_index]
                .roll
                .wrapping_add(i16::try_from(rng % 113 + 142).expect("wander delta fits i16"))
                & 0x07ff;
        }

        self.acquire_player_boundary(family_index, players, events);
        self.advance_action_15_tail(family_index, events);
    }

    fn apply_pending_damage(&mut self, family_index: usize) {
        let goat = &mut self.goats[family_index];
        if goat.pending_attacker != 0 {
            goat.life = goat.life.wrapping_sub(goat.pending_damage);
            goat.current_attacker = goat.pending_attacker;
            goat.pending_damage = 0;
            goat.pending_attacker = 0;
        } else {
            goat.current_attacker = 0;
        }
        if goat.life < 0 {
            goat.kill_credit_entity = goat.current_attacker;
        }
    }

    fn move_patrol<W: FlightWorld>(&mut self, family_index: usize, world: &W) {
        let goat = &mut self.goats[family_index];
        relax_to_terrain(
            &mut goat.position,
            world,
            self.config.ground_clearance,
            self.config.vertical_relaxation,
        );
        move_entity(&mut goat.position, goat.yaw, 0, goat.actual_speed);
        goat.yaw =
            (goat
                .yaw
                .wrapping_add(turn_toward(goat.yaw, goat.roll, self.config.turn_clamp)))
                & 0x07ff;
    }

    fn apply_peer_separation(&mut self, family_index: usize) {
        let position = self.goats[family_index].position;
        let source_id = self.goats[family_index].source_id;
        let half_width = i32::from(self.goats[family_index].collision_half_width);
        for peer_index in 0..self.goats.len() {
            if peer_index == family_index || !self.goats[peer_index].active {
                continue;
            }
            let peer = &self.goats[peer_index];
            if peer.source_id == source_id {
                continue;
            }
            if signed_axis_distance(position.x, peer.position.x) < half_width
                && signed_axis_distance(position.y, peer.position.y) < half_width
            {
                self.goats[family_index].roll =
                    i16::try_from(heading_for_positions(peer.position, position))
                        .expect("11-bit peer heading fits i16");
                return;
            }
        }
    }

    fn advance_action_15_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(77) {
            events.push(GoatStepEvent::Bleat {
                entity_index: goat.entity_index,
                sound_id: 46,
            });
        }
        goat.actual_speed = if goat.action == 14 {
            goat.minimum_speed
        } else {
            goat.maximum_speed
        };
    }

    fn acquire_player_boundary(
        &mut self,
        family_index: usize,
        players: &[GoatPlayerTarget],
        events: &mut Vec<GoatStepEvent>,
    ) {
        let goat = &self.goats[family_index];
        if goat.action != GOAT_PATROL_ACTION
            || goat.stage_var_2 != GOAT_PATROL_STAGE_MODE
            || goat.proximity_countdown == 0
            || !u16::from(goat.phase).is_multiple_of(self.config.perception_cadence)
        {
            return;
        }

        let mut nearest: Option<(u32, u16)> = None;
        for player in players {
            if player.control_flags & 0x20 != 0 {
                continue;
            }
            let dx = i32::from(player.position.x) - i32::from(goat.position.x);
            let dy = i32::from(player.position.y) - i32::from(goat.position.y);
            let distance = dx
                .wrapping_mul(dx)
                .cast_unsigned()
                .wrapping_add(dy.wrapping_mul(dy).cast_unsigned());
            if distance > u32::from(self.config.perception_radius).pow(2) {
                continue;
            }
            let heading = heading_for_positions(goat.position, player.position);
            if angle_distance(i32::from(goat.yaw), heading) >= self.config.perception_half_angle {
                continue;
            }
            if nearest.is_none_or(|(nearest_distance, _)| distance < nearest_distance) {
                nearest = Some((distance, player.entity_index));
            }
        }

        if let Some((_, target_entity_index)) = nearest {
            let goat = &mut self.goats[family_index];
            goat.target_entity_index = Some(target_entity_index);
            goat.stage_var_2 = 10;
            goat.action = 14;
            events.push(GoatStepEvent::EngagementBoundaryReached {
                entity_index: goat.entity_index,
                target_entity_index,
            });
        }
    }

    #[allow(clippy::verbose_bit_mask)]
    fn step_final_kill(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        if goat.phase & 7 != 0 {
            return;
        }
        if goat.mana > 0 {
            // `TransformEntityToManaSphere_36BA0(useManaFraction=false)`
            // advances the dying entity seed once before allocation.
            goat.rng = next_rng(goat.rng);
            events.push(GoatStepEvent::ManaConverted {
                entity_index: goat.entity_index,
                mana: goat.mana,
            });
        }
        events.push(GoatStepEvent::ExtraManaSphereRequested {
            entity_index: goat.entity_index,
        });
        goat.control_flags |= 0x0000_0400;
        events.push(GoatStepEvent::RemovalQueued {
            entity_index: goat.entity_index,
        });
    }

    /// Inject the source pending-damage fields for private qualification.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown/inactive Goat, non-positive damage, or
    /// the zero attacker sentinel.
    pub fn inject_damage(
        &mut self,
        entity_index: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<(), String> {
        if damage <= 0 || attacker == 0 {
            return Err("damage must be positive and attacker must be nonzero".to_owned());
        }
        let goat = self
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == entity_index && goat.active)
            .ok_or_else(|| format!("Goat entity {entity_index} is not live"))?;
        goat.pending_damage = damage;
        goat.pending_attacker = attacker;
        Ok(())
    }
}

/// Complete mutable non-player state owned by one native play session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveWorldState {
    goats: GoatFamily,
}

impl LiveWorldState {
    /// Construct a live world from validated owner-package inputs.
    ///
    /// # Errors
    ///
    /// Returns an error if the exact 33-row Goat family cannot be decoded.
    pub fn from_entities(
        entities: &[EntitySnapshot],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Result<Self, String> {
        Ok(Self {
            goats: GoatFamily::from_entities(entities, stage_point, config)?,
        })
    }

    #[must_use]
    pub const fn outer_tick(&self) -> u32 {
        self.goats.outer_tick()
    }

    #[must_use]
    pub fn goats(&self) -> &[GoatState] {
        self.goats.goats()
    }

    pub fn reset(
        &mut self,
        entities: &[EntitySnapshot],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Result<(), String> {
        self.goats = GoatFamily::from_entities(entities, stage_point, config)?;
        Ok(())
    }

    pub fn step<W: FlightWorld>(&mut self, world: &W) -> Vec<GoatStepEvent> {
        self.goats.step_patrol(world)
    }

    pub fn step_with_players<W: FlightWorld>(
        &mut self,
        world: &W,
        players: &[GoatPlayerTarget],
    ) -> Vec<GoatStepEvent> {
        self.goats.step_with_players(world, players)
    }

    pub fn inject_damage(
        &mut self,
        entity_index: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<(), String> {
        self.goats.inject_damage(entity_index, damage, attacker)
    }

    #[must_use]
    pub fn hash_v1(&self) -> u64 {
        let mut hash = Fnv1a64::new();
        hash.bytes(b"mc2-live-world-v1\0");
        hash.u32(self.outer_tick());
        hash.u32(u32::try_from(self.goats().len()).expect("Goat count fits u32"));
        for goat in self.goats() {
            hash.u16(goat.entity_index);
            hash.u16(goat.source_id);
            hash.u8(u8::from(goat.active));
            hash.u32(goat.control_flags);
            hash.i32(goat.max_life);
            hash.i32(goat.life);
            hash.i32(goat.pending_damage);
            hash.i16(goat.pending_attacker);
            hash.i16(goat.kill_credit_entity);
            hash.i16(goat.current_attacker);
            hash.u16(goat.rng);
            hash.i16(goat.yaw);
            hash.i16(goat.pitch);
            hash.i16(goat.roll);
            hash.u16(goat.position.x);
            hash.u16(goat.position.y);
            hash.i16(goat.position.z);
            hash.i16(goat.collision_half_width);
            hash.u16(goat.render_parameter_index);
            hash.u8(goat.action);
            hash.u8(goat.phase);
            hash.i8(goat.proximity_countdown);
            hash.i8(goat.proximity_subcounter);
            hash.i8(goat.stage_var_1);
            hash.i8(goat.stage_var_2);
            hash.i16(goat.actual_speed);
            hash.i16(goat.minimum_speed);
            hash.i16(goat.maximum_speed);
            hash.i32(goat.mana);
            match goat.target_entity_index {
                Some(index) => {
                    hash.u8(1);
                    hash.u16(index);
                }
                None => hash.u8(0),
            }
        }
        hash.finish()
    }
}

struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn i8(&mut self, value: i8) {
        self.u8(value.cast_unsigned());
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn i16(&mut self, value: i16) {
        self.u16(value.cast_unsigned());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.u32(value.cast_unsigned());
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

fn next_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

const fn relation_index(value: u16) -> Option<u16> {
    if value == 0 { None } else { Some(value) }
}

fn signed_axis_distance(left: u16, right: u16) -> i32 {
    i32::from(left.wrapping_sub(right).cast_signed()).abs()
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

fn relax_to_terrain<W: FlightWorld>(
    position: &mut Position,
    world: &W,
    ground_clearance: i16,
    vertical_relaxation: i16,
) {
    let ground = world.ground_height(*position);
    if position.z > ground {
        position.z = position.z.wrapping_add(vertical_relaxation);
    }
    if position.z <= ground.wrapping_add(ground_clearance) {
        position.z = ground.wrapping_add(ground_clearance);
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed field span"),
    )
}

fn read_i16(bytes: &[u8], offset: usize) -> i16 {
    read_u16(bytes, offset).cast_signed()
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed field span"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed field span"),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        legacy_terrain::LegacyTerrain,
        native_world::NativeWorld,
        regression_state::{RegressionEntity, RegressionStateSequence},
    };

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1",
        )
    }

    fn assert_goat_matches(actual: &GoatState, expected: RegressionEntity<'_>, frame_index: usize) {
        assert_eq!(actual.entity_index, expected.entity_index(), "entity index");
        assert_eq!(actual.source_id, expected.source_id(), "source id");
        assert_eq!(
            actual.control_flags,
            expected.control_flags(),
            "control flags"
        );
        assert_eq!(actual.max_life, expected.max_life(), "max life");
        assert_eq!(actual.life, expected.life(), "life at frame {frame_index}");
        assert_eq!(
            actual.pending_damage,
            expected.pending_damage(),
            "pending damage"
        );
        assert_eq!(
            actual.pending_attacker,
            expected.pending_attacker(),
            "pending attacker"
        );
        assert_eq!(
            actual.kill_credit_entity,
            expected.kill_credit_entity(),
            "kill credit entity"
        );
        assert_eq!(
            actual.current_attacker,
            expected.current_attacker(),
            "current attacker"
        );
        assert_eq!(actual.rng, expected.rng(), "rng at frame {frame_index}");
        assert_eq!(actual.yaw, expected.yaw(), "yaw at frame {frame_index}");
        assert_eq!(actual.pitch, expected.pitch(), "pitch");
        assert_eq!(actual.roll, expected.roll(), "roll at frame {frame_index}");
        assert_eq!(
            actual.position,
            expected.position(),
            "position at frame {frame_index}"
        );
        assert_eq!(
            actual.collision_half_width,
            expected.collision_half_width(),
            "collision half width"
        );
        assert_eq!(
            actual.render_parameter_index,
            read_u16(expected.record_bytes(), 0x005a),
            "render parameter index"
        );
        assert_eq!(
            actual.animation_frame,
            expected.record_bytes()[0x005c].cast_signed(),
            "frozen selector animation frame"
        );
        assert_eq!(
            actual.action,
            expected.action(),
            "action at frame {frame_index}"
        );
        assert_eq!(
            actual.phase,
            expected.phase(),
            "phase at frame {frame_index}"
        );
        assert_eq!(
            actual.proximity_countdown,
            expected.proximity_countdown(),
            "proximity countdown at frame {frame_index}"
        );
        assert_eq!(
            actual.proximity_subcounter,
            expected.record_bytes()[0x003a].cast_signed(),
            "proximity subcounter at frame {frame_index}"
        );
        assert_eq!(actual.stage_var_1, expected.stage_var_1(), "StageVar1");
        assert_eq!(actual.stage_var_2, expected.stage_var_2(), "StageVar2");
        assert_eq!(
            actual.actual_speed,
            expected.actual_speed(),
            "speed at frame {frame_index}"
        );
        assert_eq!(
            actual.minimum_speed,
            expected.minimum_speed(),
            "minimum speed"
        );
        assert_eq!(
            actual.maximum_speed,
            expected.maximum_speed(),
            "maximum speed"
        );
        assert_eq!(actual.mana, expected.mana(), "mana");
        assert_eq!(
            actual.target_entity_index,
            expected.target_entity_index(),
            "target entity index"
        );
    }

    #[test]
    fn exact_level_one_family_matches_all_twenty_preserved_frames() {
        let root = fixture_root();
        let state_bytes =
            std::fs::read(root.join("sequence-002285FF-00356038.bin")).expect("state sequence");
        let terrain_bytes =
            std::fs::read(root.join("sequence-002285FF-002DC4E0.bin")).expect("terrain sequence");
        let sequence = RegressionStateSequence::new(&state_bytes).expect("valid state sequence");
        assert_eq!(sequence.frame_count(), 20);

        let first = sequence.frame(0).expect("frame zero");
        let mut family = GoatFamily::from_records(
            LEVEL_ONE_GOAT_INDICES
                .into_iter()
                .map(|index| (index, first.entity(index).record_bytes())),
        )
        .expect("exact Goat family");

        for (actual, index) in family.goats().iter().zip(LEVEL_ONE_GOAT_INDICES) {
            assert_goat_matches(actual, first.entity(index), 0);
        }

        for frame_index in 1..sequence.frame_count() {
            let terrain =
                LegacyTerrain::from_regression_sequence(&terrain_bytes, frame_index - 1, false)
                    .expect("terrain frame");
            family.step_patrol(&terrain);
            let expected = sequence.frame(frame_index).expect("captured state frame");
            for (actual, index) in family.goats().iter().zip(LEVEL_ONE_GOAT_INDICES) {
                assert_goat_matches(actual, expected.entity(index), frame_index);
            }
        }
    }

    #[test]
    fn owner_package_reset_matches_preserved_frame_zero_for_all_goats() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let family = GoatFamily::from_entities(
            world.entities(),
            GOAT_STAGE_POINT,
            GoatBehaviorConfig::LEVEL_ONE,
        )
        .expect("package Goat family");

        let state_bytes = std::fs::read(fixture_root().join("sequence-002285FF-00356038.bin"))
            .expect("state sequence");
        let sequence = RegressionStateSequence::new(&state_bytes).expect("valid state sequence");
        let reset = sequence.frame(0).expect("preserved reset frame");

        for (actual, index) in family.goats().iter().zip(LEVEL_ONE_GOAT_INDICES) {
            assert_goat_matches(actual, reset.entity(index), 0);
        }
    }

    #[test]
    fn native_session_replays_all_nineteen_preserved_goat_transitions() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let state_bytes = std::fs::read(fixture_root().join("sequence-002285FF-00356038.bin"))
            .expect("state sequence");
        let sequence = RegressionStateSequence::new(&state_bytes).expect("valid state sequence");
        let mut session = world.session();

        for frame_index in 0..sequence.frame_count() {
            if frame_index > 0 {
                session.step(crate::PlayerCommand::default());
            }
            let expected = sequence.frame(frame_index).expect("captured state frame");
            for (actual, index) in session
                .live_world
                .goats()
                .iter()
                .zip(LEVEL_ONE_GOAT_INDICES)
            {
                assert_goat_matches(actual, expected.entity(index), frame_index);
            }
        }
        assert_eq!(session.live_world.outer_tick(), 19);
    }

    #[test]
    fn parser_rejects_non_goats_and_short_records() {
        assert!(GoatState::from_record(50, &[0; 167]).is_err());
        let mut record = [0_u8; 168];
        record[0x3f] = 5;
        record[0x40] = 2;
        assert!(GoatState::from_record(50, &record).is_err());
    }

    #[test]
    fn tail_rng_and_sound_cadence_are_wrapping_and_deterministic() {
        assert_eq!(next_rng(4_306), 16_625);
        let seed = (0..=u16::MAX)
            .find(|seed| next_rng(*seed).is_multiple_of(77))
            .expect("a bleat seed exists");
        assert_eq!(next_rng(seed) % 77, 0);
        assert_eq!(goat_sound_rate_delta(0, 15), 9);
        assert_eq!(goat_sound_rate_delta(0, 14), 29);
        assert_eq!(
            goat_sound_rate_delta(u32::MAX, 15),
            i16::try_from(9_439_u32.wrapping_sub(9_377) % 20).unwrap() - 10
        );
    }

    #[test]
    fn live_world_hash_reset_and_session_isolation_are_deterministic() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut first = LiveWorldState::from_entities(
            world.entities(),
            GOAT_STAGE_POINT,
            GoatBehaviorConfig::LEVEL_ONE,
        )
        .expect("first live world");
        let second = first.clone();
        let reset_hash = first.hash_v1();

        first.step(world.terrain());
        assert_ne!(first.hash_v1(), reset_hash);
        assert_eq!(second.hash_v1(), reset_hash);

        first
            .reset(
                world.entities(),
                GOAT_STAGE_POINT,
                GoatBehaviorConfig::LEVEL_ONE,
            )
            .expect("reset live world");
        assert_eq!(first, second);
        assert_eq!(first.hash_v1(), reset_hash);
    }

    #[test]
    fn damage_prekill_phase_gated_removal_and_reclamation_follow_source_order() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut live = LiveWorldState::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat world");
        let reset = live.clone();

        assert!(live.inject_damage(50, 0, 1).is_err());
        assert!(live.inject_damage(50, 601, 0).is_err());
        live.inject_damage(50, 601, 7)
            .expect("qualified pending-damage injection");

        live.step(world.terrain());
        let goat = live
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 50)
            .unwrap();
        assert_eq!(goat.life, -1);
        assert_eq!(goat.action, 12);
        assert_eq!(goat.phase, 2);
        assert_eq!(goat.pending_damage, 0);
        assert_eq!(goat.pending_attacker, 0);
        assert_eq!(goat.current_attacker, 7);
        assert_eq!(goat.kill_credit_entity, 7);

        live.step(world.terrain());
        let goat = live
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 50)
            .unwrap();
        assert_eq!(goat.action, 13);
        assert_eq!(goat.phase, 3);

        while live
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 50)
            .unwrap()
            .phase
            < 8
        {
            let events = live.step(world.terrain());
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    GoatStepEvent::RemovalQueued { entity_index: 50 }
                ))
            );
        }

        let removal = live.step(world.terrain());
        assert!(removal.contains(&GoatStepEvent::ManaConverted {
            entity_index: 50,
            mana: 300,
        }));
        assert!(removal.contains(&GoatStepEvent::ExtraManaSphereRequested { entity_index: 50 }));
        assert!(removal.contains(&GoatStepEvent::RemovalQueued { entity_index: 50 }));
        let goat = live
            .goats()
            .iter()
            .find(|goat| goat.entity_index == 50)
            .unwrap();
        assert!(goat.active);
        assert_eq!(goat.action, 13);
        assert_eq!(goat.phase, 9);
        assert_ne!(goat.control_flags & 0x0000_0400, 0);

        let reclaim = live.step(world.terrain());
        assert!(reclaim.contains(&GoatStepEvent::Reclaimed { entity_index: 50 }));
        assert!(
            !live
                .goats()
                .iter()
                .find(|goat| goat.entity_index == 50)
                .unwrap()
                .active
        );
        assert!(live.inject_damage(50, 1, 7).is_err());

        live.reset(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("reset live world");
        assert_eq!(live, reset);
    }

    #[test]
    fn acquisition_boundary_uses_countdown_phase_range_angle_and_nearest_player() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let base = LiveWorldState::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat world");

        let configured = |mut live: LiveWorldState| {
            let goat = &mut live.goats.goats[0];
            goat.position = Position {
                x: 20_000,
                y: 20_000,
                z: world.ground_height(Position {
                    x: 20_000,
                    y: 20_000,
                    z: 0,
                }),
            };
            goat.yaw = 0;
            goat.roll = 0;
            goat.actual_speed = 0;
            goat.phase = 32;
            goat.proximity_countdown = 2;
            goat.action = 15;
            goat.stage_var_2 = 2;
            goat.target_entity_index = None;
            live
        };

        let mut positive = configured(base.clone());
        let events = positive.step_with_players(
            world.terrain(),
            &[
                GoatPlayerTarget {
                    entity_index: 7,
                    position: Position {
                        x: 20_000,
                        y: 18_900,
                        z: 0,
                    },
                    control_flags: 0,
                },
                GoatPlayerTarget {
                    entity_index: 8,
                    position: Position {
                        x: 20_000,
                        y: 19_500,
                        z: 0,
                    },
                    control_flags: 0,
                },
            ],
        );
        let goat = &positive.goats()[0];
        assert_eq!(goat.target_entity_index, Some(8));
        assert_eq!(goat.stage_var_2, 10);
        assert_eq!(goat.action, 14);
        assert_eq!(goat.actual_speed, 54);
        assert_eq!(goat.phase, 33);
        assert!(events.contains(&GoatStepEvent::EngagementBoundaryReached {
            entity_index: 50,
            target_entity_index: 8,
        }));

        for (name, mutate, player) in [
            (
                "phase",
                0_u8,
                GoatPlayerTarget {
                    entity_index: 7,
                    position: Position {
                        x: 20_000,
                        y: 19_000,
                        z: 0,
                    },
                    control_flags: 0,
                },
            ),
            (
                "range",
                1_u8,
                GoatPlayerTarget {
                    entity_index: 7,
                    position: Position {
                        x: 20_000,
                        y: 18_463,
                        z: 0,
                    },
                    control_flags: 0,
                },
            ),
            (
                "eligibility",
                1_u8,
                GoatPlayerTarget {
                    entity_index: 7,
                    position: Position {
                        x: 20_000,
                        y: 19_000,
                        z: 0,
                    },
                    control_flags: 0x20,
                },
            ),
        ] {
            let mut rejected = configured(base.clone());
            if mutate == 0 {
                rejected.goats.goats[0].phase = 31;
            }
            rejected.step_with_players(world.terrain(), &[player]);
            assert_eq!(
                rejected.goats()[0].target_entity_index,
                None,
                "{name} rejection"
            );
            assert_eq!(rejected.goats()[0].action, 15, "{name} action");
        }
    }

    #[test]
    fn zero_proximity_countdown_rearms_from_local_player_before_perception() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let base = LiveWorldState::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat world");

        let configured = |mut live: LiveWorldState| {
            let goat = &mut live.goats.goats[0];
            goat.position = Position {
                x: 65_000,
                y: 1_000,
                z: 0,
            };
            goat.proximity_countdown = 0;
            goat.proximity_subcounter = 0;
            goat.phase = 1;
            live
        };
        let near = GoatPlayerTarget {
            entity_index: 152,
            position: Position {
                x: 500,
                y: 1_000,
                z: 0,
            },
            control_flags: 0,
        };

        let mut rearmed = configured(base.clone());
        rearmed.step_with_players(world.terrain(), &[near]);
        assert_eq!(rearmed.goats()[0].proximity_countdown, 16);

        let mut delayed = configured(base.clone());
        delayed.goats.goats[0].proximity_subcounter = 2;
        delayed.step_with_players(world.terrain(), &[near]);
        assert_eq!(delayed.goats()[0].proximity_countdown, 0);
        assert_eq!(delayed.goats()[0].proximity_subcounter, 1);

        let mut suppressed = configured(base);
        suppressed.goats.goats[0].control_flags |= 1;
        suppressed.step_with_players(world.terrain(), &[near]);
        assert_eq!(suppressed.goats()[0].proximity_countdown, 0);
    }
}
