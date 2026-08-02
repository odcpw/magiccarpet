//! Source-ordered Goat-family simulation for the Level 1 and Level 2 pools.
//!
//! The implemented class-5/model-1 paths are action 9, action 11, action 14,
//! and the action-15 patrol branch proven by the private remc2 source and
//! exact-build captures. Unsupported action and `StageVar` branches remain
//! explicit.

use crate::{
    FlightWorld, Position, SourceAudioCallTag, angle_distance, distance_3d_wrapped,
    goat_map_chain::{GoatMapChains, GoatMapSeed},
    heading_for_positions,
    house::{HouseFamily, HouseResidentEntry, HouseState, HouseStepEvent},
    level1_mission::{HouseConstructionSourceEffect, Level1SourceEntityView},
    mark_source_audio_call, move_entity,
    native_world::EntitySnapshot,
    turn_toward,
    villager::{VillagerBehaviorConfig, VillagerFamily, VillagerState, VillagerStepEffect},
    wrapping_i16,
};

pub const LEVEL_ONE_GOAT_INDICES: [u16; 33] = [
    50, 79, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101,
    102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
];
/// The Level 2 Day reset binds the same class-5/model-1 family to the
/// contiguous source pool 39..=74.  Slots 39 and 71 are the two action-11
/// partner rows; the remaining rows retain their authored action-9/15 paths.
pub const LEVEL_TWO_GOAT_INDICES: [u16; 36] = [
    39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
    63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74,
];

pub const GOAT_CLASS: u8 = 5;
pub const GOAT_MODEL: u8 = 1;
pub const GOAT_PATROL_ACTION: u8 = 15;
pub const GOAT_PATROL_STAGE_MODE: i8 = 2;
pub const GOAT_RENDER_PARAMETER_INDEX: u16 = 238;
pub const GOAT_BASE_TMAP: u16 = 152;

fn goat_bleat_source_call(action: u8) -> SourceAudioCallTag {
    match action {
        8 => SourceAudioCallTag::GoatAction08Bleat,
        9 => SourceAudioCallTag::GoatAction09Bleat,
        11 => SourceAudioCallTag::GoatAction11Bleat,
        14 => SourceAudioCallTag::GoatAction14HitBleat,
        15 => SourceAudioCallTag::GoatAction15ConstructorBleat,
        _ => unreachable!("only remc2 Goat bleat actions have source-call identities"),
    }
}
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
    pub allowed_material_mask: u32,
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
        allowed_material_mask: 0xfff0_80fe,
        perception_cadence: 32,
        perception_radius: 1_536,
        perception_half_angle: 853,
        behavior_flags: 9,
    };

    /// Level 2 Day Goat rows use the same class-5/model-1 parameter row as
    /// the source `AddCreature_4B490` constructor: zero-based presentation
    /// row 98 (the exported one-based relation is 99). Row 99 belongs to the
    /// class-5/model-26 constructor and is not a Goat parameter.
    pub const LEVEL_TWO: Self = Self::LEVEL_ONE;

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
            allowed_material_mask: read_u32(bytes, 20),
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

    /// Decode the Level 2 Day Goat parameter relation. The source relation is
    /// one-based `99`, which the export parser resolves to row 98.
    pub fn from_level_two_parameter_record(bytes: &[u8; 34]) -> Result<Self, String> {
        Self::from_parameter_record(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoatPlayerTarget {
    pub entity_index: u16,
    pub position: Position,
    pub control_flags: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoatStepEvent {
    Bleat {
        #[cfg(test)]
        audio_site: crate::MissionAudioSite,
        entity_index: u16,
        sound_id: u16,
        position: Position,
        action: u8,
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
    pub collision_vertical_offset: i16,
    pub collision_half_height: i16,
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
    /// Source `word_0x32_50` partner relation used by action 11.  This is
    /// distinct from the target mailbox at `word_0x96_150`.
    pub partner_entity_index: Option<u16>,
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
            collision_vertical_offset: read_i16(bytes, 0x0052),
            collision_half_height: read_i16(bytes, 0x0058),
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
            partner_entity_index: relation_index(read_u16(bytes, 0x0032)),
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
            collision_vertical_offset: read_i16(bytes, 0x0056),
            collision_half_height: read_i16(bytes, 0x005c),
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
            partner_entity_index: relation_index(read_u16(bytes, 0x0036)),
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
    /// Source `word_0x34_52` child-chain links.  These are kept separately
    /// from `GoatState`: a link may target a non-Goat source row, in which
    /// case the bounded family treats it as an explicit unsupported tail.
    child_links: Box<[Option<u16>]>,
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
        let mut child_links = Vec::with_capacity(LEVEL_ONE_GOAT_INDICES.len());
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
            child_links.push(snapshot_child_relation(snapshot));
        }
        Ok(Self {
            goats: goats.into_boxed_slice(),
            child_links: child_links.into_boxed_slice(),
            stage_point,
            config,
            outer_tick: 0,
        })
    }

    /// Construct the complete Level 2 class-5/model-1 family in authored
    /// source-pool order.  Level 2 keeps a separate pool from Level 1; in
    /// particular, reset slots 39 and 71 are the two action-11 rows.
    pub fn from_level_two_entities(
        entities: &[EntitySnapshot],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Result<Self, String> {
        let mut goats = Vec::with_capacity(LEVEL_TWO_GOAT_INDICES.len());
        let mut child_links = Vec::with_capacity(LEVEL_TWO_GOAT_INDICES.len());
        for expected_index in LEVEL_TWO_GOAT_INDICES {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Level 2 Goat entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "Level 2 Goat pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            let goat = GoatState::from_snapshot(snapshot)?;
            if !goat.active {
                return Err(format!(
                    "Level 2 Goat entity {expected_index} must be active at reset"
                ));
            }
            let expected_action = if matches!(expected_index, 39 | 71) {
                11
            } else {
                9
            };
            if goat.action != expected_action {
                return Err(format!(
                    "Level 2 Goat entity {expected_index} action mismatch: expected {expected_action}, got {}",
                    goat.action
                ));
            }
            let expected_partner = (expected_action == 11).then_some(66);
            if goat.partner_entity_index != expected_partner {
                return Err(format!(
                    "Level 2 Goat entity {expected_index} partner mismatch: expected {expected_partner:?}, got {:?}",
                    goat.partner_entity_index,
                ));
            }
            goats.push(goat);
            child_links.push(snapshot_child_relation(snapshot));
        }
        Ok(Self {
            goats: goats.into_boxed_slice(),
            child_links: child_links.into_boxed_slice(),
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
            child_links: vec![None; LEVEL_ONE_GOAT_INDICES.len()].into_boxed_slice(),
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
    pub fn child_links(&self) -> &[Option<u16>] {
        &self.child_links
    }

    #[must_use]
    pub fn child_link_for(&self, entity_index: u16) -> Option<u16> {
        self.goats
            .iter()
            .position(|goat| goat.entity_index == entity_index)
            .and_then(|index| self.child_links.get(index).copied().flatten())
    }

    /// Test/source-vector seam for mutating the imported child-chain field.
    pub fn set_child_link_for(&mut self, entity_index: u16, child: Option<u16>) -> bool {
        let Some(index) = self
            .goats
            .iter()
            .position(|goat| goat.entity_index == entity_index)
        else {
            return false;
        };
        self.child_links[index] = child;
        true
    }

    fn apply_house_construction_kill(&mut self, source_entity_index: u16, source_id: u16) {
        let Some(goat) = self
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == source_entity_index)
        else {
            return;
        };
        goat.life = -1;
        goat.kill_credit_entity = source_id.cast_signed();
        goat.current_attacker = source_id.cast_signed();
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
        let mut events = self.begin_source_turn(players);
        for source_slot in LEVEL_ONE_GOAT_INDICES {
            events.extend(self.step_source_slot(source_slot, world, players));
        }
        self.finish_source_turn();
        events
    }

    /// Advance one Level 2 source turn over its authored Goat pool.
    pub fn step_level_two_with_players<W: FlightWorld>(
        &mut self,
        world: &W,
        players: &[GoatPlayerTarget],
    ) -> Vec<GoatStepEvent> {
        let mut events = self.begin_source_turn(players);
        for source_slot in LEVEL_TWO_GOAT_INDICES {
            events.extend(self.step_source_slot(source_slot, world, players));
        }
        self.finish_source_turn();
        events
    }

    pub(crate) fn begin_source_turn(&mut self, players: &[GoatPlayerTarget]) -> Vec<GoatStepEvent> {
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
        events
    }

    fn reclaim_source_slot(&mut self, source_slot: u16) -> Option<GoatStepEvent> {
        let goat = self
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == source_slot)?;
        if !goat.active || goat.control_flags & 0x0000_0400 == 0 {
            return None;
        }
        goat.active = false;
        Some(GoatStepEvent::Reclaimed {
            entity_index: source_slot,
        })
    }

    pub(crate) fn step_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        players: &[GoatPlayerTarget],
    ) -> Vec<GoatStepEvent> {
        let mut events = Vec::new();
        let Some(family_index) = self
            .goats
            .iter()
            .position(|goat| goat.entity_index == source_slot && goat.active)
        else {
            return events;
        };
        match self.goats[family_index].action {
            9 => self.step_roam(family_index, world, players, &mut events),
            10 => self.step_action_10(family_index, world, &mut events),
            11 => self.step_action_11(family_index, world, &mut events),
            GOAT_PATROL_ACTION
                if self.goats[family_index].stage_var_2 == GOAT_PATROL_STAGE_MODE =>
            {
                self.step_one_patrol(family_index, world, players, &mut events);
            }
            14 => self.step_retaliation(family_index, world, players, &mut events),
            12 => {
                // `PreKillGoat_1F4F0` selects base action + 5.
                self.goats[family_index].action = 13;
            }
            13 => self.step_final_kill(family_index, &mut events),
            _ => return events,
        }
        self.goats[family_index].phase = self.goats[family_index].phase.wrapping_add(1);
        events
    }

    /// Port the source action-10 continuation (`sub_1F3C0`): apply pending
    /// damage and movement through `sub_1BF90(a1, 8)`, then consume the
    /// compiler-derived RNG/audio tail.  Level 2 action-11 partners can enter
    /// this state when the authored behavior flag clears the retaliation bit.
    fn step_action_10<W: FlightWorld>(
        &mut self,
        family_index: usize,
        world: &W,
        events: &mut Vec<GoatStepEvent>,
    ) {
        match self.apply_pending_damage(family_index) {
            DamageResult::Dead => {
                self.goats[family_index].action = 12;
            }
            DamageResult::Hit(attacker) => {
                self.goats[family_index].target_entity_index =
                    relation_index(attacker.cast_unsigned());
                let goat = &mut self.goats[family_index];
                if self.config.behavior_flags & 8 != 0 {
                    goat.action = 14;
                }
                relax_to_terrain(
                    &mut goat.position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
            }
            DamageResult::None => {
                self.move_patrol(family_index, world);
            }
        }
        self.advance_action_10_tail(family_index, events);
    }

    pub(crate) fn finish_source_turn(&mut self) {
        self.outer_tick = self.outer_tick.wrapping_add(1);
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

        match self.apply_pending_damage(family_index) {
            DamageResult::None => {}
            DamageResult::Hit(attacker) => {
                if players
                    .iter()
                    .any(|player| player.entity_index.cast_signed() == attacker)
                {
                    let goat = &mut self.goats[family_index];
                    goat.target_entity_index = Some(attacker.cast_unsigned());
                    goat.stage_var_2 = 10;
                    // Row 98 behavior flag bit 3 selects base action + 6.
                    goat.action = 14;
                }
                let goat = &mut self.goats[family_index];
                relax_to_terrain(
                    &mut goat.position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
                self.advance_action_15_tail(family_index, events);
                return;
            }
            DamageResult::Dead => {
                // `sub_1DDA0` selects base action + 4; Goat base is eight.
                self.goats[family_index].action = 12;
                self.advance_action_15_tail(family_index, events);
                return;
            }
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

    /// Port the pinned remc2 `sub_1F470` / `sub_1C560(a1, 8)` action-11 path.
    ///
    /// The relation field is deliberately handled before damage and movement:
    /// a zero relation is the source's immediate action-9 transition, while
    /// an invalid non-zero relation still consumes pending damage and then
    /// falls back through the movement/cleanup path.
    fn step_action_11<W: FlightWorld>(
        &mut self,
        family_index: usize,
        world: &W,
        events: &mut Vec<GoatStepEvent>,
    ) {
        let Some(partner_entity_index) = self.goats[family_index].partner_entity_index else {
            self.goats[family_index].action = 9;
            self.advance_action_11_tail(family_index, events);
            return;
        };

        let partner_index = self
            .goats
            .iter()
            .position(|goat| goat.entity_index == partner_entity_index);
        let partner_valid = partner_index.is_some_and(|index| {
            let partner = &self.goats[index];
            partner.active && partner.life >= 0 && partner.control_flags & 0x0000_0400 == 0
        });

        match self.apply_action11_damage(family_index) {
            DamageResult::Dead => {
                let attacker = self.goats[family_index].current_attacker;
                if partner_valid {
                    let partner_index = partner_index.expect("valid partner index");
                    let partner_action = if self.config.behavior_flags & 8 != 0 {
                        14
                    } else {
                        10
                    };
                    let partner = &mut self.goats[partner_index];
                    partner.target_entity_index = relation_index(attacker.cast_unsigned());
                    partner.partner_entity_index = None;
                    partner.action = partner_action;
                }
                let goat = &mut self.goats[family_index];
                goat.partner_entity_index = None;
                goat.action = 12;
            }
            DamageResult::Hit(attacker) => {
                let action = if self.config.behavior_flags & 8 != 0 {
                    14
                } else {
                    10
                };
                if partner_valid {
                    let partner_index = partner_index.expect("valid partner index");
                    let partner = &mut self.goats[partner_index];
                    partner.target_entity_index = relation_index(attacker.cast_unsigned());
                    partner.partner_entity_index = None;
                    partner.action = action;
                }
                let goat = &mut self.goats[family_index];
                goat.target_entity_index = relation_index(attacker.cast_unsigned());
                goat.partner_entity_index = None;
                goat.action = action;
                relax_to_terrain(
                    &mut goat.position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
            }
            DamageResult::None => {
                self.move_action11_source(family_index, world);
                if !partner_valid {
                    let goat = &mut self.goats[family_index];
                    goat.partner_entity_index = None;
                    goat.action = 9;
                } else if u16::from(self.goats[family_index].phase)
                    .is_multiple_of(self.config.perception_cadence)
                {
                    let partner_index = partner_index.expect("valid partner index");
                    let partner_action = self.goats[partner_index].action;
                    match i16::from(partner_action) - 8 {
                        // Partner action 8/9: face the current partner and
                        // copy `partner.maxSpeed + partner.actSpeed` into
                        // the primary Goat's current speed.
                        0 | 1 => {
                            let partner_position = self.goats[partner_index].position;
                            let partner_maximum_speed = self.goats[partner_index].maximum_speed;
                            let partner_actual_speed = self.goats[partner_index].actual_speed;
                            let heading = heading_for_positions(
                                self.goats[family_index].position,
                                partner_position,
                            );
                            let goat = &mut self.goats[family_index];
                            goat.roll = i16::try_from(heading)
                                .expect("11-bit action-11 partner heading fits i16");
                            goat.actual_speed =
                                partner_maximum_speed.wrapping_add(partner_actual_speed);
                            self.apply_partner_linked_scan(family_index);
                        }
                        // Partner action 10/11: inherit its target; action 11
                        // also follows the partner's relation chain.
                        2 => {
                            let partner_target = self.goats[partner_index].target_entity_index;
                            let goat = &mut self.goats[family_index];
                            goat.target_entity_index = partner_target;
                            goat.partner_entity_index = None;
                            goat.action = 10;
                        }
                        3 => {
                            let partner_partner = self.goats[partner_index].partner_entity_index;
                            let partner_maximum_speed = self.goats[partner_index].maximum_speed;
                            let partner_actual_speed = self.goats[partner_index].actual_speed;
                            let goat_position = self.goats[family_index].position;
                            // The source immediately dereferences the copied
                            // relation. Entity zero is its null sentinel; an
                            // unknown non-zero relation is kept fail-closed at
                            // that same zero position, never the old partner.
                            let next_position = partner_partner
                                .and_then(|next_partner_entity_index| {
                                    self.goats.iter().find(|candidate| {
                                        candidate.entity_index == next_partner_entity_index
                                    })
                                })
                                .map_or(Position::default(), |partner| partner.position);
                            let goat = &mut self.goats[family_index];
                            goat.partner_entity_index = partner_partner;
                            goat.roll =
                                i16::try_from(heading_for_positions(goat_position, next_position))
                                    .expect("11-bit action-11 chain heading fits i16");
                            goat.actual_speed =
                                partner_maximum_speed.wrapping_add(partner_actual_speed);
                            self.apply_partner_linked_scan(family_index);
                        }
                        // Partner action 14: inherit its target and enter our
                        // own action-14 branch.
                        6 => {
                            let partner_target = self.goats[partner_index].target_entity_index;
                            let goat = &mut self.goats[family_index];
                            goat.target_entity_index = partner_target;
                            goat.partner_entity_index = None;
                            goat.action = 14;
                        }
                        _ => {
                            let goat = &mut self.goats[family_index];
                            goat.partner_entity_index = None;
                            goat.action = 9;
                        }
                    }
                }
            }
        }

        self.advance_action_11_tail(family_index, events);
    }

    fn step_roam<W: FlightWorld>(
        &mut self,
        family_index: usize,
        world: &W,
        players: &[GoatPlayerTarget],
        events: &mut Vec<GoatStepEvent>,
    ) {
        match self.apply_pending_damage(family_index) {
            DamageResult::Dead => {
                self.goats[family_index].action = 12;
            }
            DamageResult::Hit(attacker) => {
                let goat = &mut self.goats[family_index];
                goat.target_entity_index = Some(attacker.cast_unsigned());
                goat.action = if self.config.behavior_flags & 8 != 0 {
                    14
                } else {
                    10
                };
                relax_to_terrain(
                    &mut goat.position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
            }
            DamageResult::None => {
                self.move_patrol(family_index, world);
                if u16::from(self.goats[family_index].phase)
                    .is_multiple_of(self.config.perception_cadence)
                {
                    let first = next_rng(self.goats[family_index].rng);
                    let second = next_rng(first);
                    self.goats[family_index].rng = second;
                    let sign = 2 * (i32::from(first % 157) / 79) - 1;
                    let magnitude = i32::from(second & 0xff) + 85;
                    self.goats[family_index].roll = wrapping_i16(
                        i64::from(self.goats[family_index].roll) + i64::from(sign * magnitude),
                    ) & 0x07ff;

                    if self.goats[family_index].proximity_countdown != 0
                        && let Some(target_entity_index) =
                            self.nearest_player_in_cone(family_index, players)
                    {
                        let goat = &mut self.goats[family_index];
                        goat.target_entity_index = Some(target_entity_index);
                        goat.action = if self.config.behavior_flags & 8 != 0 {
                            14
                        } else {
                            10
                        };
                        events.push(GoatStepEvent::EngagementBoundaryReached {
                            entity_index: goat.entity_index,
                            target_entity_index,
                        });
                    }
                }
            }
        }
        self.advance_action_9_tail(family_index, events);
    }

    fn step_retaliation<W: FlightWorld>(
        &mut self,
        family_index: usize,
        world: &W,
        players: &[GoatPlayerTarget],
        events: &mut Vec<GoatStepEvent>,
    ) {
        match self.apply_pending_damage(family_index) {
            DamageResult::Dead => {
                self.goats[family_index].action = 12;
            }
            DamageResult::Hit(attacker) => {
                self.goats[family_index].target_entity_index = Some(attacker.cast_unsigned());
                let goat = &mut self.goats[family_index];
                relax_to_terrain(
                    &mut goat.position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
            }
            DamageResult::None => {
                self.move_patrol(family_index, world);
                let target = self.goats[family_index]
                    .target_entity_index
                    .and_then(|target| {
                        players
                            .iter()
                            .find(|player| player.entity_index == target)
                            .copied()
                    });
                if let Some(target) = target {
                    if self.goats[family_index].phase & 3 == 0 {
                        let heading = heading_for_positions(
                            self.goats[family_index].position,
                            target.position,
                        );
                        self.goats[family_index].roll = i16::try_from((heading + 1_024) & 0x07ff)
                            .expect("11-bit retreat heading fits i16");
                        self.apply_peer_separation(family_index);
                    }
                    if u16::from(self.goats[family_index].phase)
                        .is_multiple_of(self.config.perception_cadence)
                        && distance_3d_wrapped(self.goats[family_index].position, target.position)
                            >= u32::from(self.config.perception_radius)
                    {
                        self.goats[family_index].action = 9;
                    }
                } else {
                    self.goats[family_index].action = 9;
                }
            }
        }
        self.advance_action_14_tail(family_index, events);
    }

    fn apply_pending_damage(&mut self, family_index: usize) -> DamageResult {
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
            DamageResult::Dead
        } else if goat.current_attacker != 0 {
            DamageResult::Hit(goat.current_attacker)
        } else {
            DamageResult::None
        }
    }

    /// Apply the action-11 mailbox and its source `word_0x34_52` child chain.
    /// The original walks the global entity table in authored link order and
    /// adopts the first lower-life child, including that child's attacker
    /// mailbox.  A relation outside this typed family is an explicit bounded
    /// tail: it cannot be dereferenced safely here and terminates the walk.
    fn apply_action11_damage(&mut self, family_index: usize) -> DamageResult {
        let goat = &mut self.goats[family_index];
        if goat.pending_attacker != 0 {
            goat.life = goat.life.wrapping_sub(goat.pending_damage);
            goat.current_attacker = goat.pending_attacker;
            goat.pending_damage = 0;
            goat.pending_attacker = 0;
        } else {
            goat.current_attacker = 0;
        }

        let mut child_entity_index = self.child_links[family_index];
        let mut visited = 0_usize;
        while let Some(child_entity) = child_entity_index {
            if visited >= self.goats.len() {
                break;
            }
            let Some(child_index) = self
                .goats
                .iter()
                .position(|candidate| candidate.entity_index == child_entity)
            else {
                break;
            };
            if child_index == family_index {
                break;
            }
            let child_life = self.goats[child_index].life;
            if child_life < self.goats[family_index].life {
                self.goats[family_index].life = child_life;
                self.goats[family_index].current_attacker =
                    self.goats[child_index].current_attacker;
                break;
            }
            child_entity_index = self.child_links[child_index];
            visited += 1;
        }

        if self.goats[family_index].life < 0 {
            self.goats[family_index].kill_credit_entity = self.goats[family_index].current_attacker;
            DamageResult::Dead
        } else if self.goats[family_index].current_attacker != 0 {
            DamageResult::Hit(self.goats[family_index].current_attacker)
        } else {
            DamageResult::None
        }
    }

    fn move_patrol<W: FlightWorld>(&mut self, family_index: usize, world: &W) {
        let original = self.goats[family_index].position;
        let original_yaw = self.goats[family_index].yaw;
        let headings = [
            original_yaw,
            original_yaw.wrapping_add(341) & 0x07ff,
            original_yaw.wrapping_sub(341) & 0x07ff,
            original_yaw.wrapping_add(1_024) & 0x07ff,
        ];

        for (attempt, heading) in headings.into_iter().enumerate() {
            let mut candidate = original;
            relax_to_terrain(
                &mut candidate,
                world,
                self.config.ground_clearance,
                self.config.vertical_relaxation,
            );
            move_entity(
                &mut candidate,
                heading,
                0,
                self.goats[family_index].actual_speed,
            );
            let crossed_tile = source_tile(candidate) != source_tile(original);
            let blocked = (attempt != 0 || crossed_tile)
                && (material_probe_blocked(
                    candidate,
                    heading,
                    self.goats[family_index].collision_half_width,
                    self.config.allowed_material_mask,
                    world,
                ) || terrain_slope_measure(candidate, world)
                    >= i32::from(self.config.terrain_threshold));
            if blocked {
                self.goats[family_index].control_flags |= 0x0004_0000;
                self.goats[family_index].yaw = heading;
                continue;
            }

            let goat = &mut self.goats[family_index];
            goat.position = candidate;
            goat.yaw = heading.wrapping_add(source_turn_delta(
                heading,
                goat.roll,
                self.config.turn_clamp,
            )) & 0x07ff;
            if attempt == 0 {
                goat.control_flags &= !0x0004_0000;
            }
            return;
        }

        let goat = &mut self.goats[family_index];
        if self.config.behavior_flags & 1 != 0 || world.material_mask(goat.position) == 1 {
            goat.life = -1;
        }
    }

    /// Source `sub_1B8C0` movement used by action 11.  This is deliberately
    /// separate from the older generic mover: the source only probes
    /// collision/slope after a tile crossing and uses three byte-preserving
    /// retry headings rather than symmetric +/-341 turns.
    fn move_action11_source<W: FlightWorld>(&mut self, family_index: usize, world: &W) {
        let control_flags = self.goats[family_index].control_flags;
        if control_flags & 0x0000_0800 != 0 {
            self.goats[family_index].control_flags &= !0x0000_0800;
            return;
        }

        let original = self.goats[family_index].position;
        let original_yaw = self.goats[family_index].yaw;
        let headings = source_retry_headings(original_yaw);
        for heading in headings {
            let mut candidate = original;
            relax_to_terrain(
                &mut candidate,
                world,
                self.config.ground_clearance,
                self.config.vertical_relaxation,
            );
            move_entity(
                &mut candidate,
                heading,
                0,
                self.goats[family_index].actual_speed,
            );
            let crossed_tile = source_tile(candidate) != source_tile(original);
            let blocked = crossed_tile
                && (material_probe_blocked(
                    candidate,
                    heading,
                    self.goats[family_index].collision_half_width,
                    self.config.allowed_material_mask,
                    world,
                ) || terrain_slope_measure(candidate, world)
                    >= i32::from(self.config.terrain_threshold));
            if blocked {
                self.goats[family_index].control_flags |= 0x0004_0000;
                continue;
            }

            let goat = &mut self.goats[family_index];
            goat.position = candidate;
            goat.yaw =
                heading.wrapping_add(turn_toward(heading, goat.roll, self.config.turn_clamp))
                    & 0x07ff;
            return;
        }

        let goat = &mut self.goats[family_index];
        if self.config.behavior_flags & 1 != 0 || world.material_mask(goat.position) == 1 {
            goat.life = -1;
        }
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

    /// Source action-11 partner scan: start at the model's linked-list head,
    /// skip rows with a live partner relation, reject equal source IDs, and
    /// use strict wrapped signed x/y bounds of 256.  The typed family is
    /// stored in that authored source order, so the first matching row is the
    /// same deterministic tie-break as the source list walk.
    fn apply_partner_linked_scan(&mut self, family_index: usize) {
        let position = self.goats[family_index].position;
        let source_id = self.goats[family_index].source_id;
        for candidate in self.goats.iter() {
            if candidate.entity_index == self.goats[family_index].entity_index
                || candidate.source_id == source_id
                || signed_axis_distance(position.x, candidate.position.x) >= 256
                || signed_axis_distance(position.y, candidate.position.y) >= 256
            {
                continue;
            }
            self.goats[family_index].roll =
                i16::try_from(heading_for_positions(candidate.position, position))
                    .expect("11-bit linked-partner heading fits i16");
            break;
        }
    }

    fn advance_action_15_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(77) {
            mark_source_audio_call(goat_bleat_source_call(15));
            events.push(GoatStepEvent::Bleat {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::GoatBleat,
                entity_index: goat.entity_index,
                sound_id: 46,
                position: goat.position,
                action: goat.action,
            });
        }
        goat.actual_speed = if goat.action == 14 {
            goat.minimum_speed
        } else {
            goat.maximum_speed
        };
    }

    fn advance_action_9_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(77) {
            mark_source_audio_call(goat_bleat_source_call(9));
            events.push(GoatStepEvent::Bleat {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::GoatBleat,
                entity_index: goat.entity_index,
                sound_id: 46,
                position: goat.position,
                action: goat.action,
            });
        }
        if goat.action == 14 {
            goat.actual_speed = goat.minimum_speed;
        }
    }

    /// The action-11 wrapper consumes exactly one LCG draw after
    /// `sub_1C560`, emits sound 46 on modulo-77, then applies the action-14
    /// minimum-speed override.  Keep this ordering distinct from action 9/15
    /// tails because it is part of the reset-visible Level 2 contract.
    fn advance_action_11_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(77) {
            mark_source_audio_call(goat_bleat_source_call(11));
            events.push(GoatStepEvent::Bleat {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::GoatBleat,
                entity_index: goat.entity_index,
                sound_id: 46,
                position: goat.position,
                action: goat.action,
            });
        }
        if goat.action == 14 {
            goat.actual_speed = goat.minimum_speed;
        }
    }

    fn advance_action_10_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(77) {
            // The source wrapper is the `a2 = 8` family tail.  Keep the
            // existing action-08 call identity while exposing the actual
            // action in the event payload for the downstream audio mapper.
            mark_source_audio_call(goat_bleat_source_call(8));
            events.push(GoatStepEvent::Bleat {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::GoatBleat,
                entity_index: goat.entity_index,
                sound_id: 46,
                position: goat.position,
                action: goat.action,
            });
        }
        if goat.action == 14 {
            goat.actual_speed = goat.minimum_speed;
        }
    }

    fn advance_action_14_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        if goat.action != 14 {
            goat.actual_speed = goat.maximum_speed;
        }
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(43) {
            mark_source_audio_call(goat_bleat_source_call(14));
            events.push(GoatStepEvent::Bleat {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::GoatBleat,
                entity_index: goat.entity_index,
                sound_id: 46,
                position: goat.position,
                action: goat.action,
            });
        }
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

        let Some(target_entity_index) = self.nearest_player_in_cone(family_index, players) else {
            return;
        };
        let goat = &mut self.goats[family_index];
        goat.target_entity_index = Some(target_entity_index);
        goat.stage_var_2 = 10;
        goat.action = 14;
        events.push(GoatStepEvent::EngagementBoundaryReached {
            entity_index: goat.entity_index,
            target_entity_index,
        });
    }

    fn nearest_player_in_cone(
        &self,
        family_index: usize,
        players: &[GoatPlayerTarget],
    ) -> Option<u16> {
        let goat = &self.goats[family_index];
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
        nearest.map(|(_, entity_index)| entity_index)
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
        if goat.pending_attacker != 0 {
            goat.pending_damage = goat.pending_damage.wrapping_add(damage);
        } else {
            goat.pending_damage = damage;
        }
        goat.pending_attacker = attacker;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DamageResult {
    None,
    Hit(i16),
    Dead,
}

/// Complete mutable non-player state owned by one native play session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveWorldState {
    goats: GoatFamily,
    houses: HouseFamily,
    villagers: VillagerFamily,
    goat_map_chains: GoatMapChains,
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
        let goats = GoatFamily::from_entities(entities, stage_point, config)?;
        let houses = HouseFamily::from_entities(entities)?;
        let villagers = VillagerFamily::from_entities(entities, VillagerBehaviorConfig::LEVEL_ONE)?;
        let goat_map_chains = GoatMapChains::from_positions(&goat_map_seeds(goats.goats()))?;
        Ok(Self {
            goats,
            houses,
            villagers,
            goat_map_chains,
        })
    }

    /// Construct the owner-package live world with exact reset terrain-cell
    /// membership and `oldMapEntity` ordering.
    ///
    /// # Errors
    ///
    /// Returns Goat decode errors or any malformed/missing map-chain error.
    pub fn from_entities_with_map(
        entities: &[EntitySnapshot],
        terrain_entity_indices: &[i16],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Result<Self, String> {
        let goats = GoatFamily::from_entities(entities, stage_point, config)?;
        let houses = HouseFamily::from_entities(entities)?;
        let villagers = VillagerFamily::from_entities(entities, VillagerBehaviorConfig::LEVEL_ONE)?;
        let goat_map_chains = GoatMapChains::from_reset(
            &goat_map_seeds(goats.goats()),
            terrain_entity_indices,
            |entity_index| {
                entities
                    .get(usize::from(entity_index))
                    .map(EntitySnapshot::old_map_entity)
            },
        )?;
        Ok(Self {
            goats,
            houses,
            villagers,
            goat_map_chains,
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

    #[must_use]
    pub fn houses(&self) -> &[HouseState] {
        self.houses.houses()
    }

    #[must_use]
    pub fn villagers(&self) -> &[VillagerState] {
        self.villagers.villagers()
    }

    /// Commit the host-owned class-5 writes emitted by remc2 `sub_57390`.
    /// The raw source arena remains authoritative for generic rows; this keeps
    /// the Goat/Villager typed authorities isomorphic before their later
    /// same-turn dispatch.
    pub fn apply_house_construction_source_effects(
        &mut self,
        effects: &[HouseConstructionSourceEffect],
    ) {
        for effect in effects {
            let HouseConstructionSourceEffect::ActorKilledAndLinked {
                house_source_id,
                source_entity_index,
                ..
            } = *effect
            else {
                continue;
            };
            self.goats
                .apply_house_construction_kill(source_entity_index, house_source_id);
            self.villagers
                .apply_house_construction_kill(source_entity_index, house_source_id);
        }
    }

    #[must_use]
    pub const fn goat_map_chains(&self) -> &GoatMapChains {
        &self.goat_map_chains
    }

    pub fn reset(
        &mut self,
        entities: &[EntitySnapshot],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Result<(), String> {
        self.goats = GoatFamily::from_entities(entities, stage_point, config)?;
        self.houses = HouseFamily::from_entities(entities)?;
        self.villagers =
            VillagerFamily::from_entities(entities, VillagerBehaviorConfig::LEVEL_ONE)?;
        self.goat_map_chains = GoatMapChains::from_positions(&goat_map_seeds(self.goats()))?;
        Ok(())
    }

    pub fn step<W: FlightWorld>(&mut self, world: &W) -> Vec<GoatStepEvent> {
        let events = self.goats.step_patrol(world);
        self.sync_goat_map_chains();
        events
    }

    pub fn step_with_players<W: FlightWorld>(
        &mut self,
        world: &W,
        players: &[GoatPlayerTarget],
    ) -> Vec<GoatStepEvent> {
        let events = self.goats.step_with_players(world, players);
        self.sync_goat_map_chains();
        events
    }

    pub fn begin_source_turn(&mut self, players: &[GoatPlayerTarget]) -> Vec<GoatStepEvent> {
        self.goats.begin_source_turn(players)
    }

    pub fn release_villager_stage_group_7(&mut self) -> usize {
        self.villagers.release_stage_group_7()
    }

    pub fn step_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        players: &[GoatPlayerTarget],
    ) -> Vec<GoatStepEvent> {
        self.goats.step_source_slot(source_slot, world, players)
    }

    pub fn step_villager_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        source_views: &[Level1SourceEntityView],
    ) -> Vec<VillagerStepEffect> {
        self.villagers
            .step_source_slot(source_slot, world, source_views, &mut self.houses)
    }

    /// Route resident entry through the existing authoritative house family.
    #[must_use]
    pub fn try_enter_resident(&mut self, entity_index: u16) -> HouseResidentEntry {
        self.houses.try_enter_resident(entity_index)
    }

    /// Commit a Builder-produced class-10/model-45 row to the one
    /// authoritative live house family.
    pub fn insert_builder_house(&mut self, house: HouseState) -> Result<(), String> {
        self.houses.insert_builder_house(house)
    }

    pub fn initialize_builder_house(&mut self, house: HouseState) -> Result<(), String> {
        self.houses.initialize_builder_house(house)
    }

    pub fn set_builder_house_action(
        &mut self,
        source_entity_index: u16,
        action: u8,
    ) -> Result<(), String> {
        self.houses
            .set_builder_house_action(source_entity_index, action)
    }

    #[must_use]
    pub fn step_house_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
    ) -> Vec<HouseStepEvent> {
        let events = self.houses.step_source_slot(source_slot, world);
        events
    }

    #[must_use]
    pub fn step_house_source_slot_with_action_51_block<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        action_51_blocked: bool,
    ) -> Vec<HouseStepEvent> {
        self.houses
            .step_source_slot_with_action_51_block(source_slot, world, action_51_blocked)
    }

    pub fn submit_house_possession(
        &mut self,
        entity_index: u16,
        owner_entity_index: u16,
        payload: u32,
    ) -> Result<(), String> {
        self.houses
            .submit_possession(entity_index, owner_entity_index, payload)
    }

    pub fn commit_house_removal_state(
        &mut self,
        entity_index: u16,
        final_rng: u16,
        final_occupants: i32,
    ) -> Result<(), String> {
        self.houses
            .commit_removal_state(entity_index, final_rng, final_occupants)?;
        Ok(())
    }

    pub fn finish_source_turn(&mut self) {
        self.goats.finish_source_turn();
        self.sync_goat_map_chains();
    }

    pub fn reclaim_source_slot(&mut self, source_slot: u16) -> Option<GoatStepEvent> {
        let event = self.goats.reclaim_source_slot(source_slot);
        self.sync_goat_map_chains();
        event
    }

    pub fn inject_damage(
        &mut self,
        entity_index: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<(), String> {
        if self
            .houses
            .houses()
            .iter()
            .any(|house| house.entity_index == entity_index && house.active)
        {
            self.houses.inject_damage(entity_index, damage, attacker)
        } else if self
            .villagers
            .villagers()
            .iter()
            .any(|villager| villager.entity_index == entity_index)
        {
            self.villagers.inject_damage(entity_index, damage, attacker)
        } else {
            self.goats.inject_damage(entity_index, damage, attacker)
        }
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
            hash.i16(goat.collision_vertical_offset);
            hash.i16(goat.collision_half_height);
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
        hash.u32(u32::try_from(self.houses().len()).expect("house count fits u32"));
        for house in self.houses() {
            hash.u16(house.entity_index);
            hash.u16(house.source_id);
            hash.u8(u8::from(house.active));
            hash.u32(house.control_flags);
            hash.i32(house.max_life);
            hash.i32(house.life);
            hash.i32(house.occupants);
            hash.u16(house.rng);
            hash.u16(house.position.x);
            hash.u16(house.position.y);
            hash.i16(house.position.z);
            hash.i16(house.collision_vertical_offset);
            hash.i16(house.collision_half_width);
            hash.i16(house.collision_half_depth);
            hash.i16(house.collision_half_height);
            hash.u8(house.action);
            hash.u8(house.phase);
            hash.u8(house.building_index);
            hash.i16(house.capacity);
            hash.u16(house.owner_entity_index);
            hash.i32(house.pending_damage);
            hash.i16(house.pending_attacker);
            hash.u16(house.pending_possession_owner);
            hash.u32(house.pending_possession_payload);
            hash.u16(house.parameter.life_mana_scalar);
            hash.u8(house.parameter.flags);
            hash.u8(house.parameter.destruction_successor);
        }
        hash.u32(u32::try_from(self.villagers().len()).expect("Villager count fits u32"));
        for villager in self.villagers() {
            hash.u16(villager.entity_index);
            hash.u16(villager.source_id);
            hash.u8(u8::from(villager.active));
            hash.u32(villager.control_flags);
            hash.i32(villager.reset_counter);
            hash.i32(villager.max_life);
            hash.i32(villager.life);
            hash.i32(villager.pending_damage);
            hash.i16(villager.pending_attacker);
            hash.i16(villager.current_attacker);
            hash.i16(villager.kill_credit_entity);
            hash.u16(villager.rng);
            hash.i16(villager.yaw);
            hash.i16(villager.pitch);
            hash.i16(villager.roll);
            hash.u16(villager.position.x);
            hash.u16(villager.position.y);
            hash.i16(villager.position.z);
            hash.i16(villager.collision_half_width);
            hash.i16(villager.collision_vertical_offset);
            hash.i16(villager.collision_half_height);
            hash.u16(villager.render_parameter_index);
            hash.i8(villager.animation_frame);
            hash.u8(villager.action);
            hash.u8(villager.phase);
            hash.i8(villager.stage_var_1);
            hash.i8(villager.stage_var_2);
            hash.i16(villager.actual_speed);
            hash.i16(villager.minimum_speed);
            hash.i16(villager.maximum_speed);
            hash.i32(villager.mana);
            hash.u16(villager.owner_entity_index);
            match villager.target_entity_index {
                Some(index) => {
                    hash.u8(1);
                    hash.u16(index);
                }
                None => hash.u8(0),
            }
        }
        hash.finish()
    }

    /// Compose the legacy typed-row hash with the mission-owned global RNG.
    ///
    /// The v1 preimage remains byte-for-byte compatible. Public current-state
    /// projections use this explicit v2 domain so future-affecting global RNG
    /// mutations cannot alias while the typed live rows remain unchanged.
    #[must_use]
    pub fn hash_v2(&self, global_rng: u32) -> u64 {
        let mut hash = Fnv1a64::new();
        hash.bytes(b"mc2-live-world-v2\0");
        hash.u64(self.hash_v1());
        hash.u32(global_rng);
        hash.finish()
    }

    fn sync_goat_map_chains(&mut self) {
        self.goat_map_chains
            .sync(&goat_map_seeds(self.goats.goats()))
            .expect("validated Goat map chains remain internally consistent");
    }
}

fn goat_map_seeds(goats: &[GoatState]) -> Vec<GoatMapSeed> {
    goats
        .iter()
        .map(|goat| GoatMapSeed {
            entity_index: goat.entity_index,
            active: goat.active,
            x: goat.position.x,
            y: goat.position.y,
        })
        .collect()
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

    fn u64(&mut self, value: u64) {
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

fn snapshot_child_relation(snapshot: &EntitySnapshot) -> Option<u16> {
    let bytes = snapshot.record_bytes();
    relation_index(read_u16(bytes, 0x0038))
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

fn source_tile(position: Position) -> (u8, u8) {
    (position.x.to_be_bytes()[0], position.y.to_be_bytes()[0])
}

fn source_retry_headings(yaw: i16) -> [i16; 4] {
    let yaw = i32::from(yaw);
    let first = yaw & 0x07ff;
    let second = (yaw + 341) & 0x07ff;
    // The decompiled source writes the low byte from yaw-85 and only the
    // three high angle bits from yaw-341.  Do not simplify this to yaw-341.
    let third = ((yaw.wrapping_sub(85) as u16 & 0x00ff)
        | ((((yaw.wrapping_sub(341) as u16) >> 8) & 7) << 8))
        & 0x07ff;
    let fourth = ((((yaw + 0x400) as u16) & 0x0700) | (yaw as u8 as u16)) & 0x07ff;
    [
        i16::try_from(first).expect("source retry heading fits i16"),
        i16::try_from(second).expect("source retry heading fits i16"),
        i16::try_from(third).expect("source retry heading fits i16"),
        i16::try_from(fourth).expect("source retry heading fits i16"),
    ]
}

/// Exact `sub_58350` / (`sub_582B0`, `sub_582F0`) signed shortest-angle
/// delta used after each successful source movement attempt. Angles are
/// eleven-bit values; the source clamps the signed shortest delta to the
/// parameter-row turn limit before adding it to the attempted heading.
fn source_turn_delta(from: i16, to: i16, maximum: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    if delta == 0 {
        return 0;
    }
    let magnitude = delta.abs().min(i32::from(maximum.max(0)));
    i16::try_from(magnitude * delta.signum()).expect("source turn delta fits i16")
}

fn material_probe_blocked<W: FlightWorld>(
    candidate: Position,
    heading: i16,
    collision_half_width: i16,
    allowed_material_mask: u32,
    world: &W,
) -> bool {
    let mut probe = candidate;
    let maximum = i32::from(collision_half_width.max(0));
    let mut distance = 0_i32;
    while distance <= maximum {
        if !allowed_material_mask & world.material_mask(probe) != 0 {
            return true;
        }
        distance += 256;
        move_entity(&mut probe, heading, 0, 256);
    }
    false
}

fn terrain_slope_measure<W: FlightWorld>(position: Position, world: &W) -> i32 {
    let (tile_x, tile_y) = source_tile(position);
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
        Tile,
        legacy_terrain::LegacyTerrain,
        native_world::NativeWorld,
        regression_state::{RegressionEntity, RegressionStateSequence},
    };

    struct MaterialBoundaryWorld {
        all_water: bool,
    }

    impl FlightWorld for MaterialBoundaryWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, position: Position) -> u32 {
            if self.all_water || position.x.to_be_bytes()[0] == 1 {
                1
            } else {
                4
            }
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    struct SlopeWorld {
        raw_point_2: i16,
    }

    impl FlightWorld for SlopeWorld {
        fn ground_height(&self, position: Position) -> i16 {
            let tile = source_tile(position);
            if tile == (2, 0) {
                self.raw_point_2 * 32
            } else {
                0
            }
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            4
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions/level1",
        )
    }

    fn level_two_fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("generated/l2-debug3.fJ3PaM/level2")
    }

    fn level_two_family() -> GoatFamily {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_root())
            .expect("qualified Level 2 owner package");
        let config = GoatBehaviorConfig::from_level_two_parameter_record(
            inspection
                .presentation()
                .entity_parameters
                .get(98)
                .expect("Level 2 Goat parameter row 98"),
        )
        .expect("exact Level 2 Goat parameter row 98");
        GoatFamily::from_level_two_entities(
            inspection.entities(),
            Position { x: 0, y: 0, z: 0 },
            config,
        )
        .expect("complete Level 2 Goat family")
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
        let reset_rng = 0xad6e_67d9;
        let reset_v1_hash = first.hash_v1();
        let reset_v2_hash = first.hash_v2(reset_rng);
        assert_eq!(
            reset_v1_hash, 0xd85c_8aee_41a9_dc64,
            "legacy v1 byte stream remains pinned"
        );
        assert_eq!(
            reset_v2_hash, 0xc5f2_8129_5e04_4de5,
            "v2 composes the pinned v1 hash with reset global RNG"
        );
        assert_eq!(first, second, "live rows start byte-for-byte identical");
        assert_eq!(
            first.hash_v1(),
            reset_v1_hash,
            "legacy v1 excludes mission-owned global RNG"
        );
        assert_ne!(
            first.hash_v2(reset_rng.wrapping_add(1)),
            reset_v2_hash,
            "an RNG-only mutation must change v2 live_world_hash while every live row is identical"
        );

        first.step(world.terrain());
        assert_ne!(first.hash_v1(), reset_v1_hash);
        assert_ne!(first.hash_v2(reset_rng), reset_v2_hash);
        assert_eq!(second.hash_v1(), reset_v1_hash);
        assert_eq!(second.hash_v2(reset_rng), reset_v2_hash);

        first
            .reset(
                world.entities(),
                GOAT_STAGE_POINT,
                GoatBehaviorConfig::LEVEL_ONE,
            )
            .expect("reset live world");
        assert_eq!(first, second);
        assert_eq!(first.hash_v1(), reset_v1_hash);
        assert_eq!(first.hash_v2(reset_rng), reset_v2_hash);
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

    #[test]
    fn row_98_material_mask_and_slope_threshold_are_exact() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        assert_eq!(world.goat_config().allowed_material_mask, 0xfff0_80fe);
        assert_eq!(world.goat_config().terrain_threshold, 20);

        let probe = Position {
            x: 1 << 8,
            y: 0,
            z: 0,
        };
        assert_eq!(
            terrain_slope_measure(probe, &SlopeWorld { raw_point_2: 19 }),
            19
        );
        assert_eq!(
            terrain_slope_measure(probe, &SlopeWorld { raw_point_2: 20 }),
            20
        );
    }

    #[test]
    fn generic_goat_mover_rejects_water_and_uses_opposite_fallback() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut family = GoatFamily::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat family");
        let goat = &mut family.goats[0];
        goat.position = Position {
            x: 250,
            y: 32_768,
            z: 0,
        };
        goat.yaw = 512;
        goat.roll = 1_536;
        goat.actual_speed = 54;
        goat.control_flags &= !0x0004_0000;

        family.move_patrol(0, &MaterialBoundaryWorld { all_water: false });
        let goat = &family.goats[0];
        assert_eq!(goat.position.x, 196);
        assert_eq!(goat.position.y, 32_768);
        assert_eq!(goat.yaw, 1_536);
        assert_eq!(goat.life, 600);
        assert_ne!(goat.control_flags & 0x0004_0000, 0);
        assert_eq!(
            MaterialBoundaryWorld { all_water: false }.material_mask(goat.position),
            4
        );
    }

    #[test]
    fn generic_goat_mover_kills_when_all_four_headings_are_blocked() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut family = GoatFamily::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat family");
        let goat = &mut family.goats[0];
        goat.position = Position {
            x: 250,
            y: 32_768,
            z: 0,
        };
        goat.yaw = 512;
        goat.actual_speed = 54;

        family.move_patrol(0, &MaterialBoundaryWorld { all_water: true });
        let goat = &family.goats[0];
        assert_eq!(
            goat.position,
            Position {
                x: 250,
                y: 32_768,
                z: 0,
            }
        );
        assert_eq!(goat.yaw, 1_536);
        assert_eq!(goat.life, -1);
        assert_ne!(goat.control_flags & 0x0004_0000, 0);
    }

    #[test]
    fn action_15_hit_enters_action_14_and_retaliation_consumes_accumulated_damage() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut family = GoatFamily::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat family");
        let player = GoatPlayerTarget {
            entity_index: 152,
            position: Position {
                x: family.goats[0].position.x,
                y: family.goats[0].position.y.wrapping_sub(100),
                z: family.goats[0].position.z,
            },
            control_flags: 0,
        };

        family.inject_damage(50, 100, 152).expect("first area hit");
        family.step_with_players(world.terrain(), &[player]);
        let goat = &family.goats[0];
        assert_eq!(goat.life, 500);
        assert_eq!(goat.action, 14);
        assert_eq!(goat.stage_var_2, 10);
        assert_eq!(goat.target_entity_index, Some(152));
        assert_eq!(goat.phase, 2);
        assert_eq!(goat.actual_speed, goat.minimum_speed);

        family
            .inject_damage(50, 200, 152)
            .expect("first pending retaliation hit");
        family
            .inject_damage(50, 301, 152)
            .expect("second pending retaliation hit");
        assert_eq!(family.goats[0].pending_damage, 501);
        family.step_with_players(world.terrain(), &[player]);
        let goat = &family.goats[0];
        assert_eq!(goat.life, -1);
        assert_eq!(goat.action, 12);
        assert_eq!(goat.phase, 3);
        assert_eq!(goat.pending_damage, 0);
        assert_eq!(goat.pending_attacker, 0);
        assert_eq!(goat.kill_credit_entity, 152);
        assert_eq!(goat.actual_speed, goat.maximum_speed);
    }

    #[test]
    fn action_9_reacquires_on_cadence_and_damage_interrupts_immediately() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut family = GoatFamily::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat family");
        let player = GoatPlayerTarget {
            entity_index: 152,
            position: Position {
                x: family.goats[0].position.x,
                y: family.goats[0].position.y.wrapping_sub(100),
                z: family.goats[0].position.z,
            },
            control_flags: 0,
        };

        {
            let goat = &mut family.goats[0];
            goat.action = 9;
            goat.stage_var_2 = 10;
            goat.phase = 32;
            goat.proximity_countdown = 2;
            goat.yaw = 0;
            goat.roll = 0;
            goat.actual_speed = 0;
            goat.target_entity_index = None;
        }
        let events = family.step_with_players(world.terrain(), &[player]);
        let goat = &family.goats[0];
        assert_eq!(goat.action, 14);
        assert_eq!(goat.target_entity_index, Some(152));
        assert_eq!(goat.actual_speed, goat.minimum_speed);
        assert!(events.contains(&GoatStepEvent::EngagementBoundaryReached {
            entity_index: 50,
            target_entity_index: 152,
        }));

        {
            let goat = &mut family.goats[0];
            goat.action = 9;
            goat.phase = 33;
            goat.target_entity_index = None;
        }
        family
            .inject_damage(50, 100, 152)
            .expect("action-9 pending hit");
        family.step_with_players(world.terrain(), &[player]);
        let goat = &family.goats[0];
        assert_eq!(goat.life, 500);
        assert_eq!(goat.action, 14);
        assert_eq!(goat.target_entity_index, Some(152));
        assert_eq!(goat.actual_speed, goat.minimum_speed);
        assert_eq!(goat.pending_damage, 0);
        assert_eq!(goat.pending_attacker, 0);
    }

    #[test]
    fn owner_map_live_goats_never_commit_to_disallowed_materials() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut family = GoatFamily::from_entities(
            world.entities(),
            world.goat_stage_point(),
            world.goat_config(),
        )
        .expect("live Goat family");

        for tick in 0..10_000 {
            family.step_patrol(world.terrain());
            for goat in family.goats().iter().filter(|goat| goat.active) {
                assert_eq!(
                    !world.goat_config().allowed_material_mask & world.material_mask(goat.position),
                    0,
                    "Goat {} entered a disallowed material at tick {tick}",
                    goat.entity_index
                );
            }
        }
    }

    #[test]
    fn level_two_reset_has_action11_only_at_source_slots_39_and_71() {
        let row98 = [
            0x27, 0x00, 0x2d, 0x00, 0x05, 0x00, 0x16, 0x00, 0x05, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0xff, 0x14, 0x00, 0x00, 0x02, 0xfe, 0x80, 0xf0, 0xff, 0x00, 0x09, 0x20, 0x00,
            0x00, 0x06, 0x55, 0x03, 0x09, 0x00,
        ];
        assert_eq!(
            GoatBehaviorConfig::from_level_two_parameter_record(&row98)
                .expect("exact Goat parameter row 98"),
            GoatBehaviorConfig::LEVEL_TWO
        );
        let inspection = NativeWorld::inspect_level2(level_two_fixture_root())
            .expect("qualified Level 2 owner package");
        let action11_slots: Vec<_> = inspection
            .entities()
            .iter()
            .filter(|entity| {
                entity.class == GOAT_CLASS && entity.model == GOAT_MODEL && entity.action == 11
            })
            .map(|entity| entity.index)
            .collect();
        assert_eq!(action11_slots, [39, 71]);
        assert_eq!(LEVEL_TWO_GOAT_INDICES.len(), 36);
        assert_eq!(LEVEL_TWO_GOAT_INDICES[0], 39);
        assert_eq!(LEVEL_TWO_GOAT_INDICES[35], 74);

        let config = GoatBehaviorConfig::from_level_two_parameter_record(
            inspection
                .presentation()
                .entity_parameters
                .get(98)
                .expect("Level 2 Goat parameter row 98"),
        )
        .expect("exact Level 2 Goat parameter row 98");
        assert_eq!(config, GoatBehaviorConfig::LEVEL_TWO);
        for slot in action11_slots {
            let goat = inspection
                .entities()
                .get(usize::from(slot))
                .expect("action-11 source slot");
            assert_eq!(goat.parameter_relation, Some(98));
            assert_eq!(
                read_u16(goat.record_bytes(), 0x0036),
                66,
                "reset action-11 partner relation"
            );
            assert_eq!(
                inspection.entities()[66].action,
                9,
                "action-11 partner is the authored action-9 Goat at slot 66"
            );
        }
    }

    #[test]
    fn action11_no_relation_is_no_call_damage_guard_but_rng_and_phase_still_advance() {
        let mut family = level_two_family();
        let goat = family
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == 39)
            .expect("reset action-11 Goat");
        let seed = (0..=u16::MAX)
            .find(|seed| next_rng(*seed).is_multiple_of(77))
            .expect("an action-11 bleat seed exists");
        goat.action = 11;
        goat.partner_entity_index = None;
        goat.pending_damage = 100;
        goat.pending_attacker = 7;
        goat.rng = seed;
        goat.phase = 0;
        let life = goat.life;

        let events = family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);
        let goat = family
            .goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .expect("action-11 Goat");
        assert_eq!(goat.life, life, "source returns before pending damage");
        assert_eq!(goat.pending_damage, 100);
        assert_eq!(goat.pending_attacker, 7);
        assert_eq!(goat.action, 9);
        assert_eq!(goat.phase, 1);
        assert_eq!(goat.rng, next_rng(seed));
        assert!(matches!(
            events.as_slice(),
            [GoatStepEvent::Bleat {
                entity_index: 39,
                sound_id: 46,
                action: 9,
                ..
            }]
        ));
    }

    #[test]
    fn action11_partner_actions_preserve_source_transition_and_speed_order() {
        let world = MaterialBoundaryWorld { all_water: false };
        for (partner_action, expected_action, expected_relation, expected_target, min_speed) in [
            (8, 11, Some(40), None, false),
            (9, 11, Some(40), None, false),
            (10, 10, None, Some(152), false),
            (11, 11, Some(41), None, false),
            (14, 14, None, Some(152), true),
            (12, 9, None, None, false),
        ] {
            let mut family = level_two_family();
            let primary_index = family
                .goats
                .iter()
                .position(|goat| goat.entity_index == 39)
                .expect("primary action-11 Goat");
            let partner_index = family
                .goats
                .iter()
                .position(|goat| goat.entity_index == 40)
                .expect("partner Goat");
            let next_index = family
                .goats
                .iter()
                .position(|goat| goat.entity_index == 41)
                .expect("action-11 chain target");
            family.goats[primary_index].action = 11;
            family.goats[primary_index].phase = 0;
            family.goats[primary_index].partner_entity_index = Some(40);
            family.goats[primary_index].position = Position {
                x: 1_000,
                y: 1_000,
                z: 0,
            };
            family.goats[primary_index].actual_speed = 3;
            family.goats[primary_index].target_entity_index = None;
            family.goats[partner_index].action = partner_action;
            family.goats[partner_index].position = Position {
                x: 10_000,
                y: 1_000,
                z: 0,
            };
            family.goats[partner_index].maximum_speed = 17;
            family.goats[partner_index].actual_speed = 23;
            family.goats[partner_index].target_entity_index = Some(152);
            family.goats[partner_index].partner_entity_index = (partner_action == 11).then_some(41);
            family.goats[next_index].position = Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            };

            family.step_source_slot(39, &world, &[]);
            let primary = &family.goats[primary_index];
            assert_eq!(
                primary.action, expected_action,
                "partner action {partner_action}"
            );
            assert_eq!(
                primary.partner_entity_index, expected_relation,
                "partner relation for action {partner_action}"
            );
            assert_eq!(
                primary.target_entity_index, expected_target,
                "partner target for action {partner_action}"
            );
            if partner_action == 8 || partner_action == 9 || partner_action == 11 {
                assert_eq!(
                    primary.actual_speed, 40,
                    "partner speed for action {partner_action}"
                );
            }
            if min_speed {
                assert_eq!(primary.actual_speed, primary.minimum_speed);
            }
        }
    }

    #[test]
    fn action11_action11_partner_uses_null_sentinel_for_missing_chain() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let partner_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 40)
            .expect("partner Goat");
        family.goats[primary_index].action = 11;
        family.goats[primary_index].phase = 0;
        family.goats[primary_index].partner_entity_index = Some(40);
        family.goats[primary_index].position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        family.goats[primary_index].actual_speed = 3;
        family.goats[partner_index].action = 11;
        family.goats[partner_index].partner_entity_index = None;
        family.goats[partner_index].maximum_speed = 17;
        family.goats[partner_index].actual_speed = 23;
        family.goats[partner_index].position = Position {
            x: 10_000,
            y: 11_000,
            z: 0,
        };

        family.step_source_slot(39, &world, &[]);
        let primary = &family.goats[primary_index];
        assert_eq!(primary.action, 11);
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(primary.actual_speed, 40);
        assert_eq!(
            primary.roll,
            i16::try_from(heading_for_positions(primary.position, Position::default()))
                .expect("11-bit null-sentinel heading fits i16")
        );
    }

    #[test]
    fn action11_invalid_partner_consumes_damage_before_falling_back_to_roam() {
        let mut family = level_two_family();
        let primary = family
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        primary.action = 11;
        primary.phase = 0;
        primary.partner_entity_index = Some(999);
        primary.pending_damage = 100;
        primary.pending_attacker = 7;
        let events = family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);
        let primary = family
            .goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        assert_eq!(primary.life, 500);
        assert_eq!(primary.current_attacker, 7);
        assert_eq!(primary.pending_damage, 0);
        assert_eq!(primary.pending_attacker, 0);
        // A non-zero but invalid relation still takes the damage branch. The
        // source row-98 behavior flag preserves the player-engagement bit, so
        // `sub_1C560(a2=8)` selects action 14 before the wrapper tail.
        assert_eq!(primary.action, 14);
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(primary.phase, 1);
        assert!(events.is_empty(), "no sound chosen by this seed");

        let rng_after_transition = primary.rng;
        family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);
        let primary = family
            .goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .expect("primary roaming Goat");
        assert_eq!(primary.action, 9);
        assert_eq!(primary.phase, 2);
        assert_eq!(primary.rng, next_rng(rng_after_transition));
    }

    #[test]
    fn action11_damage_transitions_update_partner_before_wrapper_tail() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut hit = level_two_family();
        {
            let primary = hit
                .goats
                .iter_mut()
                .find(|goat| goat.entity_index == 39)
                .expect("primary action-11 Goat");
            primary.action = 11;
            primary.partner_entity_index = Some(40);
            primary.phase = 0;
        }
        hit.inject_damage(39, 100, 152)
            .expect("nonlethal action-11 hit");
        hit.step_source_slot(39, &world, &[]);
        let primary = hit
            .goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .unwrap();
        let partner = hit
            .goats
            .iter()
            .find(|goat| goat.entity_index == 40)
            .unwrap();
        assert_eq!(primary.life, 500);
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(152));
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(partner.action, 14);
        assert_eq!(partner.target_entity_index, Some(152));
        assert_eq!(partner.partner_entity_index, None);

        let mut dead = level_two_family();
        {
            let primary = dead
                .goats
                .iter_mut()
                .find(|goat| goat.entity_index == 39)
                .expect("primary action-11 Goat");
            primary.action = 11;
            primary.partner_entity_index = Some(40);
            primary.phase = 0;
        }
        // The source kill check is strict (`life < 0`), so 601 damage is the
        // first lethal value for the reset life 600 row.
        dead.inject_damage(39, 601, 152)
            .expect("lethal action-11 hit");
        dead.step_source_slot(39, &world, &[]);
        let primary = dead
            .goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .unwrap();
        let partner = dead
            .goats
            .iter()
            .find(|goat| goat.entity_index == 40)
            .unwrap();
        assert_eq!(primary.life, -1);
        assert_eq!(primary.action, 12);
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(partner.action, 14);
        assert_eq!(partner.target_entity_index, Some(152));
        assert_eq!(partner.partner_entity_index, None);
    }

    #[test]
    fn level_two_action11_source_order_replays_and_reset_restores_both_rows() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_root())
            .expect("qualified Level 2 owner package");
        let config = GoatBehaviorConfig::from_level_two_parameter_record(
            inspection
                .presentation()
                .entity_parameters
                .get(98)
                .expect("Level 2 Goat parameter row 98"),
        )
        .expect("exact Level 2 Goat parameter row 98");
        let baseline = GoatFamily::from_level_two_entities(
            inspection.entities(),
            Position { x: 0, y: 0, z: 0 },
            config,
        )
        .expect("complete Level 2 Goat family");
        let mut first = baseline.clone();
        let mut replay = baseline.clone();
        first.step_level_two_with_players(inspection.terrain(), &[]);
        replay.step_level_two_with_players(inspection.terrain(), &[]);
        assert_eq!(first, replay);
        assert_eq!(first.outer_tick(), 1);
        for slot in [39_u16, 71] {
            let goat = first
                .goats
                .iter()
                .find(|goat| goat.entity_index == slot)
                .unwrap();
            assert_eq!(
                goat.action, 11,
                "reset action-11 slot {slot} follows action-9 partner"
            );
            assert_eq!(goat.partner_entity_index, Some(66));
            let reset_phase = baseline
                .goats
                .iter()
                .find(|candidate| candidate.entity_index == slot)
                .unwrap()
                .phase;
            assert_eq!(
                goat.phase,
                reset_phase.wrapping_add(1),
                "source pass advances reset phase"
            );
        }
        let reset = GoatFamily::from_level_two_entities(
            inspection.entities(),
            Position { x: 0, y: 0, z: 0 },
            config,
        )
        .expect("reset Level 2 Goat family");
        assert_eq!(reset, baseline);
    }
}
