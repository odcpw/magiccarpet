//! Goat construction, source actions 8–15, damage, StageVar routes and death.

use crate::{
    FlightWorld, Position, SourceAudioCallTag, Tile, angle_distance,
    combat::actors::{
        DamageMailboxMut, DamageWriter, consume_damage, inherit_killer, inherit_linked_life,
    },
    distance_3d_wrapped, heading_for_positions, mark_source_audio_call, move_entity,
    native_world::EntitySnapshot,
    turn_toward, wrapping_i16,
};

pub(crate) mod runtime;
#[cfg(test)]
mod runtime_tests;
mod stage;

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
pub(crate) const GOAT_CONSTRUCTOR_ACTION: u8 = 9;
pub(crate) const GOAT_CONSTRUCTOR_LIFE: i32 = 600;
pub(crate) const GOAT_CONSTRUCTOR_MANA: i32 = 300;
pub(crate) const GOAT_CONSTRUCTOR_MINIMUM_SPEED: i16 = 54;
pub(crate) const GOAT_CONSTRUCTOR_MAXIMUM_SPEED: i16 = 18;
pub(crate) const GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT: i16 = 75;
pub(crate) const GOAT_DESCRIPTOR_INDEX: u16 = 98;
pub(crate) const GOAT_DESCRIPTOR_CADENCE: i8 = 33;

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
    /// Source `array_0x52_82.roll`, distinct from entity orientation `roll`.
    pub collision_half_depth: i16,
    pub collision_vertical_offset: i16,
    /// Source `array_0x52_82.fov`, used by the cave clearance gate.
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
    /// Source `dword_0xA0_160x->byte_160_0x20_32` for this entity's
    /// descriptor. Action 11 reads the partner descriptor, not the primary
    /// family's shared configuration, when choosing the partner follow-up.
    pub behavior_flags: u8,
    /// Source `word_0x32_50` partner relation used by action 11.  This is
    /// distinct from the target mailbox at `word_0x96_150`.
    pub partner_entity_index: Option<u16>,
    pub target_entity_index: Option<u16>,
    pub parent_entity_index: u16,
    pub stage_target: u16,
    pub lifetime: i16,
    pub initial_lifetime: i16,
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
            collision_half_depth: read_i16(bytes, 0x0056),
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
            behavior_flags: GoatBehaviorConfig::LEVEL_ONE.behavior_flags,
            partner_entity_index: relation_index(read_u16(bytes, 0x0032)),
            target_entity_index: relation_index(read_u16(bytes, 0x0096)),
            parent_entity_index: read_u16(bytes, 0x0028),
            stage_target: read_u16(bytes, 0x004a),
            lifetime: read_i16(bytes, 0x002e),
            initial_lifetime: read_i16(bytes, 0x0030),
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
            collision_half_depth: read_i16(bytes, 0x005a),
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
            behavior_flags: GoatBehaviorConfig::LEVEL_ONE.behavior_flags,
            partner_entity_index: relation_index(read_u16(bytes, 0x0036)),
            target_entity_index: relation_index(read_u16(bytes, 0x009a)),
            parent_entity_index: read_u16(bytes, 0x002c),
            stage_target: read_u16(bytes, 0x004e),
            lifetime: read_i16(bytes, 0x0032),
            initial_lifetime: read_i16(bytes, 0x0034),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceEntityState {
    entity_index: u16,
    source_id: u16,
    class: u8,
    model: u8,
    active: bool,
    control_flags: u32,
    life: i32,
    current_attacker: i16,
    kill_credit_entity: i16,
    action: u8,
    position: Position,
    child_relation: Option<u16>,
    actual_speed: i16,
    maximum_speed: i16,
    behavior_flags: u8,
    partner_entity_index: Option<u16>,
    target_entity_index: Option<u16>,
    parent_entity_index: u16,
    yaw: i16,
    roll: i16,
    half_width: i16,
    stage_var_1: i8,
    stage_var_2: i8,
    stage_target: u16,
}

impl SourceEntityState {
    fn from_snapshot(snapshot: &EntitySnapshot) -> Self {
        let bytes = snapshot.record_bytes();
        Self {
            entity_index: snapshot.index,
            source_id: snapshot.source_id,
            class: snapshot.class,
            model: snapshot.model,
            active: snapshot.active,
            control_flags: snapshot.control_flags,
            life: snapshot.life,
            current_attacker: read_i16(bytes, 0x002a),
            kill_credit_entity: read_i16(bytes, 0x0028),
            action: snapshot.action,
            position: snapshot.position,
            child_relation: snapshot_child_relation(snapshot),
            actual_speed: snapshot.actual_speed,
            maximum_speed: read_i16(bytes, 0x008a),
            behavior_flags: if snapshot.class == GOAT_CLASS && snapshot.model == GOAT_MODEL {
                GoatBehaviorConfig::LEVEL_TWO.behavior_flags
            } else {
                0
            },
            partner_entity_index: relation_index(read_u16(bytes, 0x0036)),
            target_entity_index: relation_index(read_u16(bytes, 0x009a)),
            parent_entity_index: read_u16(bytes, 44),
            yaw: snapshot.yaw,
            roll: read_i16(bytes, 36),
            half_width: read_i16(bytes, 88),
            stage_var_1: bytes[76] as i8,
            stage_var_2: bytes[77] as i8,
            stage_target: read_u16(bytes, 78),
        }
    }
}

fn source_entity_states(entities: &[EntitySnapshot]) -> Box<[SourceEntityState]> {
    entities
        .iter()
        .map(SourceEntityState::from_snapshot)
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceEntityView {
    entity_index: u16,
    source_id: u16,
    class: u8,
    model: u8,
    active: bool,
    control_flags: u32,
    life: i32,
    current_attacker: i16,
    kill_credit_entity: i16,
    action: u8,
    position: Position,
    child_relation: Option<u16>,
    actual_speed: i16,
    maximum_speed: i16,
    behavior_flags: u8,
    partner_entity_index: Option<u16>,
    target_entity_index: Option<u16>,
    parent_entity_index: u16,
    yaw: i16,
    roll: i16,
    half_width: i16,
    stage_var_1: i8,
    stage_var_2: i8,
    stage_target: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoatFamily {
    goats: Box<[GoatState]>,
    /// Source `word_0x34_52` child-chain links.  These are kept separately
    /// from `GoatState`: a link may target a non-Goat source row, in which
    /// case the bounded family treats it as an explicit unsupported tail.
    child_links: Box<[Option<u16>]>,
    /// Immutable metadata for the complete source pool. The native loop
    /// rebuilds the class-5/model linked lists from this pool before dispatch;
    /// retaining it lets action 11 follow source order and inspect a child
    /// relation that is not itself a typed Goat.
    source_entities: Box<[SourceEntityState]>,
    stage_point: Position,
    config: GoatBehaviorConfig,
    outer_tick: u32,
    frozen_model_slots: Option<Box<[Vec<u16>; 29]>>,
    frozen_class3_slots: Option<Box<[u16]>>,
}

impl GoatFamily {
    pub(crate) fn empty(
        entities: &[EntitySnapshot],
        stage_point: Position,
        config: GoatBehaviorConfig,
    ) -> Self {
        Self {
            goats: Box::new([]),
            child_links: Box::new([]),
            source_entities: source_entity_states(entities),
            stage_point,
            config,
            outer_tick: 0,
            frozen_model_slots: None,
            frozen_class3_slots: None,
        }
    }
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
            source_entities: source_entity_states(entities),
            stage_point,
            config,
            outer_tick: 0,
            frozen_model_slots: None,
            frozen_class3_slots: None,
        })
    }

    /// Adopt every live Goat row from a validated campaign reset in source
    /// slot order.  The campaign package owns the population; unlike the
    /// retained Level 1/2 compatibility constructors this path does not
    /// assume a fixed slot census.
    pub fn from_campaign_entities(
        entities: &[EntitySnapshot],
        variables: &[crate::native_world::RuntimeStageVariable],
        config: GoatBehaviorConfig,
    ) -> Result<Self, String> {
        let mut goats = Vec::new();
        let mut child_links = Vec::new();
        for snapshot in entities.iter().filter(|entity| {
            entity.active && entity.class == GOAT_CLASS && entity.model == GOAT_MODEL
        }) {
            goats.push(GoatState::from_snapshot(snapshot)?);
            child_links.push(snapshot_child_relation(snapshot));
        }
        // Campaign callbacks resolve each Goat's own route. The retained
        // standalone-family entry only needs a fallback for a point route;
        // an unrelated mission's slot 3 must never constrain construction.
        let mut stage_point = None;
        for goat in goats
            .iter()
            .filter(|goat| matches!(goat.stage_var_2, 1 | 2))
        {
            let point =
                stage::point_for_route(goat.entity_index, goat.stage_var_1 as u8, variables)?;
            stage_point.get_or_insert(point);
        }
        Ok(Self {
            goats: goats.into_boxed_slice(),
            child_links: child_links.into_boxed_slice(),
            source_entities: source_entity_states(entities),
            stage_point: stage_point.unwrap_or_default(),
            config,
            outer_tick: 0,
            frozen_model_slots: None,
            frozen_class3_slots: None,
        })
    }

    /// Add one `AddCreature_4B490` result to the existing source-ordered Goat
    /// family. The shared NewEvent arena owns slot allocation, RNG seeding,
    /// map linkage, and reset; this family remains the sole behavior carrier.
    pub(crate) fn add_authored_goat(
        &mut self,
        entity_index: u16,
        entity_rng: u16,
        phase: u8,
        position: Position,
        max_life: i32,
    ) -> Result<(), String> {
        if max_life <= 0 {
            return Err(format!(
                "Goat source slot {entity_index} has invalid maximum life {max_life}"
            ));
        }
        if self
            .goats
            .iter()
            .any(|goat| goat.entity_index == entity_index && goat.active)
        {
            return Err(format!(
                "Goat source slot {entity_index} is already occupied"
            ));
        }

        let goat = GoatState {
            entity_index,
            source_id: entity_index,
            active: true,
            control_flags: 8,
            max_life,
            life: max_life,
            pending_damage: 0,
            pending_attacker: 0,
            kill_credit_entity: 0,
            current_attacker: 0,
            rng: entity_rng,
            yaw: 0,
            pitch: 0,
            roll: 0,
            position,
            collision_half_width: GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT,
            collision_half_depth: GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT,
            collision_vertical_offset: GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT,
            collision_half_height: GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT,
            render_parameter_index: GOAT_RENDER_PARAMETER_INDEX,
            animation_frame: 0,
            action: GOAT_CONSTRUCTOR_ACTION,
            phase,
            proximity_countdown: GOAT_DESCRIPTOR_CADENCE,
            proximity_subcounter: 0,
            stage_var_1: 0,
            stage_var_2: 0,
            actual_speed: GOAT_CONSTRUCTOR_MAXIMUM_SPEED,
            minimum_speed: GOAT_CONSTRUCTOR_MINIMUM_SPEED,
            maximum_speed: GOAT_CONSTRUCTOR_MAXIMUM_SPEED,
            mana: GOAT_CONSTRUCTOR_MANA,
            behavior_flags: self.config.behavior_flags,
            partner_entity_index: None,
            target_entity_index: None,
            parent_entity_index: 0,
            stage_target: 0,
            lifetime: 0,
            initial_lifetime: 0,
        };

        if let Some(index) = self
            .goats
            .iter()
            .position(|old| old.entity_index == entity_index)
        {
            self.goats[index] = goat;
            self.child_links[index] = None;
            return Ok(());
        }
        let insert_at = self
            .goats
            .iter()
            .position(|candidate| candidate.entity_index > entity_index)
            .unwrap_or(self.goats.len());
        let mut goats = self.goats.to_vec();
        let mut child_links = self.child_links.to_vec();
        goats.insert(insert_at, goat);
        child_links.insert(insert_at, None);
        self.goats = goats.into_boxed_slice();
        self.child_links = child_links.into_boxed_slice();
        Ok(())
    }

    /// Bind one reset-live semantic Goat before the first scheduler visit.
    ///
    /// The imported arena and this typed family are parallel projections of
    /// the same source row. Validate the package baseline in the typed owner,
    /// then update both the live Goat and its source-order relation snapshot so
    /// movement and action-11 child reads cannot observe split authority.
    pub(crate) fn bind_frame_zero_semantic_variant(
        &mut self,
        entity_index: u16,
        package_position: Position,
        authored_position: Position,
        package_max_life: i32,
        authored_max_life: i32,
    ) -> Result<(), String> {
        if authored_max_life <= 0 {
            return Err(format!(
                "Goat source slot {entity_index} has invalid authored maximum life {authored_max_life}"
            ));
        }
        let goat_index = self
            .goats
            .iter()
            .position(|goat| goat.entity_index == entity_index)
            .ok_or_else(|| format!("Goat source slot {entity_index} is absent"))?;
        let goat = &self.goats[goat_index];
        if goat.position != package_position
            || goat.max_life != package_max_life
            || goat.life != package_max_life
        {
            return Err(format!(
                "Goat source slot {entity_index} diverges from its semantic package baseline"
            ));
        }
        let source_index = self
            .source_entities
            .iter()
            .position(|source| source.entity_index == entity_index)
            .ok_or_else(|| format!("Goat source relation {entity_index} is absent"))?;
        let source = &self.source_entities[source_index];
        if source.position != package_position || source.life != package_max_life {
            return Err(format!(
                "Goat source relation {entity_index} diverges from its semantic package baseline"
            ));
        }

        let (goats, source_entities) = (&mut self.goats, &mut self.source_entities);
        let goat = goats
            .get_mut(goat_index)
            .ok_or_else(|| format!("validated Goat source slot {entity_index} disappeared"))?;
        let source = source_entities
            .get_mut(source_index)
            .ok_or_else(|| format!("validated Goat source relation {entity_index} disappeared"))?;
        goat.position = authored_position;
        goat.max_life = authored_max_life;
        goat.life = authored_max_life;
        source.position = authored_position;
        source.life = authored_max_life;
        Ok(())
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
            source_entities: source_entity_states(entities),
            stage_point,
            config,
            outer_tick: 0,
            frozen_model_slots: None,
            frozen_class3_slots: None,
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
            source_entities: Box::new([]),
            stage_point: GOAT_STAGE_POINT,
            config: GoatBehaviorConfig::LEVEL_ONE,
            outer_tick: 0,
            frozen_model_slots: None,
            frozen_class3_slots: None,
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

    fn source_entity_view(&self, entity_index: u16) -> Option<SourceEntityView> {
        // Runtime metadata is refreshed from the arena. A reclaimed Goat's
        // tombstone must not shadow a different occupant reusing its slot.
        if let Some((goat_index, goat)) = self.goats.iter().enumerate().find(|(_, goat)| {
            goat.entity_index == entity_index && (goat.active || self.frozen_model_slots.is_none())
        }) {
            return Some(SourceEntityView {
                entity_index: goat.entity_index,
                source_id: goat.source_id,
                class: GOAT_CLASS,
                model: GOAT_MODEL,
                active: goat.active,
                control_flags: goat.control_flags,
                life: goat.life,
                current_attacker: goat.current_attacker,
                kill_credit_entity: goat.kill_credit_entity,
                action: goat.action,
                position: goat.position,
                child_relation: self.child_links[goat_index],
                actual_speed: goat.actual_speed,
                maximum_speed: goat.maximum_speed,
                behavior_flags: goat.behavior_flags,
                partner_entity_index: goat.partner_entity_index,
                target_entity_index: goat.target_entity_index,
                parent_entity_index: goat.parent_entity_index,
                yaw: goat.yaw,
                roll: goat.roll,
                half_width: goat.collision_half_width,
                stage_var_1: goat.stage_var_1,
                stage_var_2: goat.stage_var_2,
                stage_target: goat.stage_target,
            });
        }
        self.source_entities
            .iter()
            .find(|entity| entity.entity_index == entity_index)
            .map(|entity| SourceEntityView {
                entity_index: entity.entity_index,
                source_id: entity.source_id,
                class: entity.class,
                model: entity.model,
                active: entity.active,
                control_flags: entity.control_flags,
                life: entity.life,
                current_attacker: entity.current_attacker,
                kill_credit_entity: entity.kill_credit_entity,
                action: entity.action,
                position: entity.position,
                child_relation: entity.child_relation,
                actual_speed: entity.actual_speed,
                maximum_speed: entity.maximum_speed,
                behavior_flags: entity.behavior_flags,
                partner_entity_index: entity.partner_entity_index,
                target_entity_index: entity.target_entity_index,
                parent_entity_index: entity.parent_entity_index,
                yaw: entity.yaw,
                roll: entity.roll,
                half_width: entity.half_width,
                stage_var_1: entity.stage_var_1,
                stage_var_2: entity.stage_var_2,
                stage_target: entity.stage_target,
            })
    }

    /// Mutate the source relation/action fields for either a typed Goat or a
    /// global source row. The remc2 action-11 routine writes the global entity
    /// table, so a relation is not invalid merely because its row is outside
    /// the typed Goat slice.
    fn update_source_action11_relation(
        &mut self,
        entity_index: u16,
        target_entity_index: Option<u16>,
        partner_entity_index: Option<u16>,
        action: u8,
    ) -> bool {
        if let Some(goat) = self
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == entity_index)
        {
            goat.target_entity_index = target_entity_index;
            goat.partner_entity_index = partner_entity_index;
            goat.action = action;
            return true;
        }
        if let Some(entity) = self
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == entity_index)
        {
            entity.target_entity_index = target_entity_index;
            entity.partner_entity_index = partner_entity_index;
            entity.action = action;
            return true;
        }
        false
    }

    /// Return the class-5/model linked-list order rebuilt by the native outer
    /// loop. The source walks entity-pool order, excludes dead rows and its
    /// three terminal/draw actions (0xb4, 0xe8, and 0xea), and links each
    /// surviving row through `next_0`.
    fn source_model_linked_list(&self, model: u8) -> Vec<u16> {
        if let Some(lists) = &self.frozen_model_slots {
            return lists.get(usize::from(model)).cloned().unwrap_or_default();
        }
        if self.source_entities.is_empty() {
            return self
                .goats
                .iter()
                .filter(|goat| {
                    goat.active
                        && goat.life >= 0
                        && goat.action != 0xb4
                        && goat.action != 0xe8
                        && goat.action != 0xea
                })
                .map(|goat| goat.entity_index)
                .collect();
        }
        self.source_entities
            .iter()
            .filter_map(|entity| {
                let current = self.source_entity_view(entity.entity_index)?;
                (current.class == GOAT_CLASS
                    && current.model == model
                    && current.active
                    && current.life >= 0
                    && current.action != 0xb4
                    && current.action != 0xe8
                    && current.action != 0xea)
                    .then_some(current.entity_index)
            })
            .collect()
    }

    pub(crate) fn apply_house_construction_kill(
        &mut self,
        source_entity_index: u16,
        source_id: u16,
    ) {
        self.apply_source_kill(source_entity_index, source_id, Some(source_id));
    }

    pub(crate) fn apply_source_kill(
        &mut self,
        source_entity_index: u16,
        killer: u16,
        attacker: Option<u16>,
    ) -> bool {
        let Some(goat) = self
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == source_entity_index)
        else {
            return false;
        };
        goat.life = -1;
        goat.kill_credit_entity = killer.cast_signed();
        if let Some(attacker) = attacker {
            goat.current_attacker = attacker.cast_signed();
        }
        true
    }

    #[must_use]
    pub const fn outer_tick(&self) -> u32 {
        self.outer_tick
    }

    #[must_use]
    pub(crate) const fn behavior_config(&self) -> GoatBehaviorConfig {
        self.config
    }

    pub(crate) fn replace_behavior_config(&mut self, config: GoatBehaviorConfig) {
        self.config = config;
        for goat in &mut self.goats {
            goat.behavior_flags = config.behavior_flags;
        }
    }

    /// Advance one unpaused source outer turn.
    ///
    /// Campaign scheduling supplies the live arena and full StageVar context;
    /// this Level 1 entry retains its package-bound point route.
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

    pub(crate) fn reclaim_source_slot(&mut self, source_slot: u16) -> Option<GoatStepEvent> {
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
            8 => self.step_action_8(family_index, world, &mut events),
            9 => self.step_roam(family_index, world, players, &mut events),
            10 => self.step_action_10(family_index, world, players, &mut events),
            11 => self.step_action_11(family_index, world, &mut events),
            GOAT_PATROL_ACTION => {
                self.step_stage(
                    family_index,
                    world,
                    players,
                    &[],
                    &mut Vec::new(),
                    &mut events,
                )
                .expect("standalone Goat steps use the retained point, without runtime routes");
                self.advance_action_15_tail(family_index, &mut events);
            }
            14 => self.step_retaliation(family_index, world, players, &mut events),
            12 => {
                // `PreKillGoat_1F4F0` selects base action + 5.
                self.pre_kill(family_index);
            }
            13 => self.step_final_kill(family_index, &mut events),
            _ => return events,
        }
        self.goats[family_index].phase = self.goats[family_index].phase.wrapping_add(1);
        events
    }

    /// Port the source action-10 hit callback (`sub_1F440`): force action 14
    /// and minimum speed, apply the shared `sub_1C980(a1, 8)` damage/child
    /// chain, then run the distinct `HitGoat_1F530` modulo-43 tail. Action 10
    /// is not the roaming/acquisition continuation; that is action 9's
    /// `sub_1F3C0` path.
    fn step_action_10<W: FlightWorld>(
        &mut self,
        family_index: usize,
        world: &W,
        players: &[GoatPlayerTarget],
        events: &mut Vec<GoatStepEvent>,
    ) {
        self.goats[family_index].action = 14;
        self.goats[family_index].actual_speed = self.goats[family_index].minimum_speed;
        self.step_retaliation(family_index, world, players, events);
    }

    /// `sub_1BD90` is stationary: consume linked damage, acquire a partner,
    /// relax height, then run the action-8 wrapper (never the roaming mover).
    fn step_action_8<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        events: &mut Vec<GoatStepEvent>,
    ) {
        match self.apply_action11_damage(index) {
            DamageResult::Dead => self.goats[index].action = 12,
            DamageResult::Hit(attacker) => {
                self.goats[index].target_entity_index = relation_index(attacker.cast_unsigned());
                self.goats[index].action = self.engagement_action(index);
            }
            DamageResult::None => {
                if self.goats[index].behavior_flags & 4 == 0
                    && u16::from(self.goats[index].phase)
                        .is_multiple_of(self.config.perception_cadence)
                    && let Some(partner) = self.nearest_unpaired_peer_in_cone(index)
                {
                    self.goats[index].partner_entity_index = Some(partner);
                    self.goats[index].action = 11;
                }
            }
        }
        relax_to_terrain(
            &mut self.goats[index].position,
            world,
            self.config.ground_clearance,
            self.config.vertical_relaxation,
        );
        self.advance_action_8_tail(index, events);
    }

    fn engagement_action(&self, index: usize) -> u8 {
        if self.goats[index].behavior_flags & 8 != 0 {
            14
        } else {
            10
        }
    }

    fn pre_kill(&mut self, index: usize) {
        let mut child = self.child_links[index];
        let mut visited = 0;
        while let Some(slot) = child {
            if visited >= self.source_entities.len().max(self.goats.len()) {
                break;
            }
            let Some(child_index) = self.goats.iter().position(|goat| goat.entity_index == slot)
            else {
                break;
            };
            if child_index == index {
                break;
            }
            self.goats[child_index].action = 13;
            let mut killer = self.goats[index].kill_credit_entity.cast_unsigned();
            inherit_killer(
                &mut killer,
                self.goats[child_index].kill_credit_entity.cast_unsigned(),
            );
            self.goats[index].kill_credit_entity = killer.cast_signed();
            child = self.child_links[child_index];
            visited += 1;
        }
        self.goats[index].action = 13;
    }

    pub(crate) fn finish_source_turn(&mut self) {
        self.outer_tick = self.outer_tick.wrapping_add(1);
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

        let partner_view = self.source_entity_view(partner_entity_index);
        let partner_valid = partner_view.is_some_and(|partner| {
            partner.class == GOAT_CLASS
                && partner.model == GOAT_MODEL
                && partner.active
                && partner.life >= 0
                && partner.control_flags & 0x0000_0400 == 0
        });

        match self.apply_action11_damage(family_index) {
            DamageResult::Dead => {
                let attacker = self.goats[family_index].current_attacker;
                if partner_valid {
                    let partner_action =
                        if partner_view.expect("valid partner view").behavior_flags & 8 != 0 {
                            14
                        } else {
                            10
                        };
                    self.update_source_action11_relation(
                        partner_entity_index,
                        relation_index(attacker.cast_unsigned()),
                        None,
                        partner_action,
                    );
                }
                let goat = &mut self.goats[family_index];
                goat.partner_entity_index = None;
                goat.action = 12;
            }
            DamageResult::Hit(attacker) => {
                let action = if self.goats[family_index].behavior_flags & 8 != 0 {
                    14
                } else {
                    10
                };
                if partner_valid {
                    let partner_action =
                        if partner_view.expect("valid partner view").behavior_flags & 8 != 0 {
                            14
                        } else {
                            10
                        };
                    self.update_source_action11_relation(
                        partner_entity_index,
                        relation_index(attacker.cast_unsigned()),
                        None,
                        partner_action,
                    );
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
                    let partner = partner_view.expect("valid partner view");
                    let partner_action = partner.action;
                    match i16::from(partner_action) - 8 {
                        // Partner action 8/9: face the current partner and
                        // copy `partner.maxSpeed + partner.actSpeed` into
                        // the primary Goat's current speed.
                        0 | 1 => {
                            let heading = heading_for_positions(
                                self.goats[family_index].position,
                                partner.position,
                            );
                            let goat = &mut self.goats[family_index];
                            goat.roll = i16::try_from(heading)
                                .expect("11-bit action-11 partner heading fits i16");
                            goat.actual_speed =
                                partner.maximum_speed.wrapping_add(partner.actual_speed);
                            self.apply_partner_linked_scan(family_index);
                        }
                        // Partner action 10/11: inherit its target; action 11
                        // also follows the partner's relation chain.
                        2 => {
                            let goat = &mut self.goats[family_index];
                            goat.target_entity_index = partner.target_entity_index;
                            goat.partner_entity_index = None;
                            goat.action = 10;
                        }
                        3 => {
                            let goat_position = self.goats[family_index].position;
                            // The source immediately dereferences the copied
                            // relation. Entity zero is its null sentinel; an
                            // unknown non-zero relation is kept fail-closed at
                            // that same zero position, never the old partner.
                            let next_position = partner
                                .partner_entity_index
                                .and_then(|next_partner_entity_index| {
                                    self.source_entity_view(next_partner_entity_index)
                                })
                                .map_or(Position::default(), |partner| partner.position);
                            let goat = &mut self.goats[family_index];
                            goat.partner_entity_index = partner.partner_entity_index;
                            goat.roll =
                                i16::try_from(heading_for_positions(goat_position, next_position))
                                    .expect("11-bit action-11 chain heading fits i16");
                            goat.actual_speed =
                                partner.maximum_speed.wrapping_add(partner.actual_speed);
                            self.apply_partner_linked_scan(family_index);
                        }
                        // Partner action 14: inherit its target and enter our
                        // own action-14 branch.
                        6 => {
                            let goat = &mut self.goats[family_index];
                            goat.target_entity_index = partner.target_entity_index;
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
        let event_action = self.goats[family_index].action;
        match self.apply_action11_damage(family_index) {
            DamageResult::Dead => {
                self.goats[family_index].action = 12;
            }
            DamageResult::Hit(attacker) => {
                let goat = &mut self.goats[family_index];
                goat.target_entity_index = relation_index(attacker.cast_unsigned());
                goat.action = if goat.behavior_flags & 8 != 0 { 14 } else { 10 };
                relax_to_terrain(
                    &mut goat.position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
            }
            DamageResult::None => {
                self.move_action11_source(family_index, world);
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

                    if self.goats[family_index].proximity_countdown != 0 {
                        if let Some(target_entity_index) =
                            self.nearest_player_in_cone(family_index, players)
                        {
                            let goat = &mut self.goats[family_index];
                            goat.target_entity_index = Some(target_entity_index);
                            goat.action = if goat.behavior_flags & 8 != 0 { 14 } else { 10 };
                            events.push(GoatStepEvent::EngagementBoundaryReached {
                                entity_index: goat.entity_index,
                                target_entity_index,
                            });
                        } else if self.goats[family_index].behavior_flags & 4 == 0
                            && let Some(partner_entity_index) =
                                self.nearest_unpaired_peer_in_cone(family_index)
                        {
                            let goat = &mut self.goats[family_index];
                            goat.partner_entity_index = Some(partner_entity_index);
                            goat.action = 11;
                        }
                    }
                }
            }
        }
        self.advance_action_9_tail(family_index, event_action, events);
    }

    /// Both actions 10 and 14 call HitGoat/sub_1C980, including the
    /// modulo-43 wrapper on every visit, even while the target stays valid.
    fn step_retaliation<W: FlightWorld>(
        &mut self,
        index: usize,
        world: &W,
        players: &[GoatPlayerTarget],
        events: &mut Vec<GoatStepEvent>,
    ) {
        match self.apply_action11_damage(index) {
            DamageResult::Dead => self.goats[index].action = 12,
            DamageResult::Hit(attacker) => {
                self.goats[index].target_entity_index = relation_index(attacker.cast_unsigned());
                relax_to_terrain(
                    &mut self.goats[index].position,
                    world,
                    self.config.ground_clearance,
                    self.config.vertical_relaxation,
                );
            }
            DamageResult::None => {
                self.move_action11_source(index, world);
                let target = self.alliance_target(index).and_then(|slot| {
                    let target = self
                        .source_entity_view(slot)
                        .filter(|target| target.active && target.life >= 0)?;
                    if self.frozen_model_slots.is_none() && target.class == 3 && target.model == 0 {
                        // The retained Level 1 entry receives current players,
                        // while its other source views come from the reset.
                        // Campaign callbacks instead refresh the whole arena.
                        players
                            .iter()
                            .find(|player| {
                                player.entity_index == slot && player.control_flags & 0x400 == 0
                            })
                            .map(|player| player.position)
                    } else {
                        (target.control_flags & 0x400 == 0).then_some(target.position)
                    }
                });
                if let Some(position) = target {
                    if self.goats[index].phase & 3 == 0 {
                        self.goats[index].roll =
                            ((heading_for_positions(self.goats[index].position, position) + 1024)
                                & 0x7ff) as i16;
                        self.apply_peer_separation(index);
                    }
                    if u16::from(self.goats[index].phase)
                        .is_multiple_of(self.config.perception_cadence)
                        && distance_3d_wrapped(self.goats[index].position, position)
                            >= u32::from(self.config.perception_radius)
                    {
                        self.goats[index].action = 9;
                    }
                } else {
                    self.goats[index].action = 9;
                }
            }
        }
        self.advance_action_14_hit_tail(index, events);
    }

    /// Apply the action-11 mailbox and its source `word_0x34_52` child chain.
    /// The original walks the global entity table in authored link order and
    /// adopts the first lower-life child, including that child's attacker
    /// mailbox.  A relation outside this typed family is an explicit bounded
    /// tail: it cannot be dereferenced safely here and terminates the walk.
    fn apply_action11_damage(&mut self, family_index: usize) -> DamageResult {
        let goat = &mut self.goats[family_index];
        let mut pending = goat.pending_attacker.cast_unsigned();
        let mut current = goat.current_attacker.cast_unsigned();
        let mut hit = consume_damage(
            &mut goat.life,
            &mut goat.pending_damage,
            &mut pending,
            &mut current,
        )
        .is_some();
        goat.pending_attacker = pending.cast_signed();
        goat.current_attacker = current.cast_signed();

        let mut child_entity_index = self.child_links[family_index];
        let mut visited = 0_usize;
        while let Some(child_entity) = child_entity_index {
            if visited >= self.source_entities.len().max(self.goats.len()) {
                break;
            }
            let Some(child) = self.source_entity_view(child_entity) else {
                break;
            };
            if child.entity_index == self.goats[family_index].entity_index {
                break;
            }
            let goat = &mut self.goats[family_index];
            let mut current = goat.current_attacker.cast_unsigned();
            if inherit_linked_life(
                &mut goat.life,
                &mut current,
                child.life,
                child.current_attacker.cast_unsigned(),
            ) {
                hit = true;
                goat.current_attacker = current.cast_signed();
                break;
            }
            child_entity_index = child.child_relation;
            visited += 1;
        }

        if self.goats[family_index].life < 0 {
            self.goats[family_index].kill_credit_entity = self.goats[family_index].current_attacker;
            DamageResult::Dead
        } else if hit {
            DamageResult::Hit(self.goats[family_index].current_attacker)
        } else {
            DamageResult::None
        }
    }

    #[cfg(test)]
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
                && (source_sub102d0_probe_blocked(
                    candidate,
                    heading,
                    self.goats[family_index].collision_half_width,
                    self.goats[family_index].collision_half_depth,
                    self.goats[family_index].collision_half_height,
                    self.config.ground_clearance,
                    self.config.allowed_material_mask,
                    world,
                ) || terrain_slope_measure(candidate, world)
                    >= i32::from(self.config.terrain_threshold));
            if blocked {
                self.goats[family_index].control_flags |= 0x0004_0000;
                // `sub_1B8C0` commits each failed retry heading before it
                // attempts the next one; the final blocked state therefore
                // retains the fourth (byte-preserving) heading.
                self.goats[family_index].yaw = heading;
                continue;
            }

            let goat = &mut self.goats[family_index];
            goat.position = candidate;
            goat.yaw =
                heading.wrapping_add(turn_toward(heading, goat.roll, self.config.turn_clamp))
                    & 0x07ff;
            if attempt == 0 {
                // `sub_1B8C0` clears the obstruction marker after either
                // kind of first-attempt success. A later retry after a
                // blocked first probe retains the bit, even if that retry
                // stays within the original tile.
                goat.control_flags &= !0x0004_0000;
            }
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
        for slot in self.source_model_linked_list(GOAT_MODEL) {
            let Some(peer) = self.source_entity_view(slot) else {
                continue;
            };
            if peer.source_id == source_id {
                continue;
            }
            // sub_1C980 promotes unsigned positions to int before subtraction.
            if (i32::from(position.x) - i32::from(peer.position.x)).abs() < half_width
                && (i32::from(position.y) - i32::from(peer.position.y)).abs() < half_width
            {
                self.goats[family_index].roll =
                    heading_for_positions(peer.position, position) as i16;
                return;
            }
        }
    }

    /// Source action-11 partner scan: start at the model's linked-list head,
    /// reject equal source IDs, and use strict wrapped signed x/y bounds of
    /// 256. The list is rebuilt from the complete class-5 source pool in the
    /// same ascending slot order as `sub_68BF0`, not from the typed Goat slice.
    fn apply_partner_linked_scan(&mut self, family_index: usize) {
        let position = self.goats[family_index].position;
        let source_id = self.goats[family_index].source_id;
        for candidate_entity_index in self.source_model_linked_list(GOAT_MODEL) {
            let Some(candidate) = self.source_entity_view(candidate_entity_index) else {
                continue;
            };
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

    fn advance_action_9_tail(
        &mut self,
        family_index: usize,
        event_action: u8,
        events: &mut Vec<GoatStepEvent>,
    ) {
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
                action: event_action,
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

    fn advance_action_8_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
        let goat = &mut self.goats[family_index];
        goat.rng = next_rng(goat.rng);
        if goat.rng.is_multiple_of(77) {
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

    fn advance_action_14_hit_tail(&mut self, family_index: usize, events: &mut Vec<GoatStepEvent>) {
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
        let vigilant_always_perceives = self.config.behavior_flags & 0x10 != 0;
        if goat.action != GOAT_PATROL_ACTION
            || goat.stage_var_2 != GOAT_PATROL_STAGE_MODE
            || (!vigilant_always_perceives && goat.proximity_countdown == 0)
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
            if player.control_flags & (0x20 | 0x0000_0400) != 0 {
                continue;
            }
            let dx = if goat.action == 15 && goat.stage_var_2 == 2 {
                i32::from(player.position.x) - i32::from(goat.position.x)
            } else {
                i32::from(
                    player
                        .position
                        .x
                        .wrapping_sub(goat.position.x)
                        .cast_signed(),
                )
            };
            let dy = i32::from(
                player
                    .position
                    .y
                    .wrapping_sub(goat.position.y)
                    .cast_signed(),
            );
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

    fn nearest_unpaired_peer_in_cone(&self, family_index: usize) -> Option<u16> {
        let goat = &self.goats[family_index];
        let mut nearest: Option<(u32, u16)> = None;
        for candidate_entity_index in self.source_model_linked_list(GOAT_MODEL) {
            let Some(candidate) = self.source_entity_view(candidate_entity_index) else {
                continue;
            };
            if candidate.entity_index == goat.entity_index
                || candidate.partner_entity_index.is_some()
            {
                continue;
            }
            let dx = i32::from(
                candidate
                    .position
                    .x
                    .wrapping_sub(goat.position.x)
                    .cast_signed(),
            );
            let dy = i32::from(
                candidate
                    .position
                    .y
                    .wrapping_sub(goat.position.y)
                    .cast_signed(),
            );
            let distance = dx
                .wrapping_mul(dx)
                .cast_unsigned()
                .wrapping_add(dy.wrapping_mul(dy).cast_unsigned());
            if distance > u32::from(self.config.perception_radius).pow(2) {
                continue;
            }
            let heading = if goat.action == 8 {
                heading_for_positions(candidate.position, goat.position)
            } else {
                heading_for_positions(goat.position, candidate.position)
            };
            if angle_distance(i32::from(goat.yaw), heading) >= self.config.perception_half_angle {
                continue;
            }
            if nearest.is_none_or(|(nearest_distance, _)| distance < nearest_distance) {
                nearest = Some((distance, candidate.entity_index));
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

    /// Bind the imported raw control dword for a live source row.  This keeps
    /// source byte[2] suppression (including `KillEntity_1C930`'s death-burst
    /// gate) testable without exposing the backing family storage.
    pub fn set_control_flags(&mut self, entity_index: u16, control_flags: u32) -> bool {
        let Some(goat) = self
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == entity_index)
        else {
            return false;
        };
        goat.control_flags = control_flags;
        true
    }

    /// Gravity's sub_3A200 writes an absolute accumulated bounty and owner,
    /// independently of the ordinary/contact damage writer's replace rules.
    pub(crate) fn set_gravity_bounty(&mut self, slot: u16, payload: i32, owner: u16) -> bool {
        let Some(goat) = self
            .goats
            .iter_mut()
            .find(|goat| goat.active && goat.entity_index == slot)
        else {
            return false;
        };
        goat.pending_damage = payload;
        goat.pending_attacker = owner as i16;
        true
    }

    #[cfg(test)]
    pub fn replace_goat_for_test(&mut self, updated: GoatState) -> bool {
        let Some(index) = self
            .goats
            .iter()
            .position(|goat| goat.entity_index == updated.entity_index)
        else {
            return false;
        };
        self.goats[index] = updated;
        true
    }

    /// Apply the fields written by Alliance's source impact to the live Goat
    /// authority before its next scheduled visit.
    pub(crate) fn sync_alliance_target(&mut self, row: &EntitySnapshot) -> bool {
        if !row.active || row.class != GOAT_CLASS || row.model != GOAT_MODEL {
            return false;
        }
        let Some(goat) = self.goats.iter_mut().find(|goat| {
            goat.active && goat.entity_index == row.index && goat.source_id == row.source_id
        }) else {
            return false;
        };
        let bytes = row.record_bytes();
        if bytes.len() < 156 {
            return false;
        }
        goat.action = row.action;
        goat.stage_var_1 = bytes[76] as i8;
        goat.stage_var_2 = bytes[77] as i8;
        goat.stage_target = read_u16(bytes, 78);
        goat.parent_entity_index = read_u16(bytes, 44);
        goat.lifetime = read_i16(bytes, 50);
        goat.initial_lifetime = read_i16(bytes, 52);
        goat.target_entity_index = relation_index(read_u16(bytes, 154));
        true
    }

    /// The source area writer accepts zero payload and owner words. Its
    /// caller owns overlap selection; this owner retains the real mailbox.
    pub(crate) fn submit_source_damage(&mut self, slot: u16, payload: u16, owner: u16) -> bool {
        let Some(goat) = self
            .goats
            .iter_mut()
            .find(|goat| goat.active && goat.entity_index == slot && goat.source_id != owner)
        else {
            return false;
        };
        let mut pending = goat.pending_attacker.cast_unsigned();
        DamageMailboxMut::new(&mut goat.pending_damage, &mut pending).submit(
            i32::from(payload),
            owner,
            DamageWriter::Ordinary,
        );
        goat.pending_attacker = pending.cast_signed();
        true
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
        let mut pending = goat.pending_attacker.cast_unsigned();
        DamageMailboxMut::new(&mut goat.pending_damage, &mut pending).submit(
            damage,
            attacker.cast_unsigned(),
            DamageWriter::Ordinary,
        );
        goat.pending_attacker = pending.cast_signed();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DamageResult {
    None,
    Hit(i16),
    Dead,
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
#[cfg(test)]
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

/// Port the `sub_102D0(a1, predicted, 1)` probe used by `sub_1B8C0`.
///
/// The source checks the complete forward footprint, not only the candidate
/// tile: material and cave clearance are sampled every 256 units until the
/// larger `array_0x52_82.pitch/roll` extent is reached. Keeping this separate
/// from the Level 1 mover prevents the Level 2 action-11 path from silently
/// inheriting a different collision contract.
fn source_sub102d0_probe_blocked<W: FlightWorld>(
    candidate: Position,
    heading: i16,
    collision_half_width: i16,
    collision_half_depth: i16,
    collision_half_height: i16,
    ground_clearance: i16,
    allowed_material_mask: u32,
    world: &W,
) -> bool {
    let maximum = i32::from(collision_half_width.max(collision_half_depth).max(0));
    let mut probe = candidate;
    let mut distance = 0_i32;
    while distance <= maximum {
        if !allowed_material_mask & world.material_mask(probe) != 0 {
            return true;
        }

        // `sub_102D0` checks the cave angle bit before `sub_11E70`.  The
        // presence of a ceiling is the typed seam's cave-level marker.
        if world.cave_collision_blocked(probe) {
            return true;
        }
        if let Some(ceiling) = world.ceiling_height(probe) {
            if world.angle_flags(Tile {
                x: probe.x.to_be_bytes()[0],
                y: probe.y.to_be_bytes()[0],
            }) & 8
                != 0
                || i32::from(collision_half_height)
                    + i32::from(world.ground_height(probe))
                    + i32::from(ground_clearance)
                    > i32::from(ceiling)
            {
                return true;
            }
        }

        distance += 256;
        move_entity(&mut probe, heading, 0, 256);
    }
    false
}

#[cfg(test)]
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
    use std::{cell::Cell, path::PathBuf};

    use super::*;
    use crate::goat::LiveWorldState;
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

    pub(super) struct OpenWorld;

    impl FlightWorld for OpenWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
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

    struct FirstCrossingBlockedWorld {
        material_calls: Cell<u8>,
    }

    impl FlightWorld for FirstCrossingBlockedWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            let call = self.material_calls.get();
            self.material_calls.set(call.saturating_add(1));
            if call == 0 { 1 } else { 4 }
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    struct CaveBoundaryWorld;

    impl FlightWorld for CaveBoundaryWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            Some(2_048)
        }

        fn material_mask(&self, _position: Position) -> u32 {
            4
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            8
        }
    }

    struct IntermediateMaterialProbeWorld {
        material_calls: Cell<u8>,
    }

    impl FlightWorld for IntermediateMaterialProbeWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            None
        }

        fn material_mask(&self, _position: Position) -> u32 {
            let call = self.material_calls.get();
            self.material_calls.set(call.saturating_add(1));
            // The first heading is clear at its final candidate but blocked
            // by the second 256-unit probe. Later retry headings are clear.
            if call == 1 { 1 } else { 4 }
        }

        fn angle_flags(&self, _tile: Tile) -> u8 {
            0
        }
    }

    struct IntermediateCeilingProbeWorld {
        ceiling_calls: Cell<u8>,
    }

    impl FlightWorld for IntermediateCeilingProbeWorld {
        fn ground_height(&self, _position: Position) -> i16 {
            0
        }

        fn ceiling_height(&self, _position: Position) -> Option<i16> {
            let call = self.ceiling_calls.get();
            self.ceiling_calls.set(call.saturating_add(1));
            // Keep the first candidate open, then close the intermediate
            // 256-unit sample; subsequent retry headings stay open.
            Some(if call == 1 { 1 } else { 2_048 })
        }

        fn material_mask(&self, _position: Position) -> u32 {
            4
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

    pub(super) fn level_two_family() -> GoatFamily {
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
        let reset_v3_hash = first.hash_v3(reset_rng);
        assert_eq!(
            reset_v1_hash, 0xd85c_8aee_41a9_dc64,
            "legacy v1 byte stream remains pinned"
        );
        assert_eq!(
            reset_v2_hash, 0xc5f2_8129_5e04_4de5,
            "v2 composes the pinned v1 hash with reset global RNG"
        );
        let mut changed_depth = second.clone();
        changed_depth.goats.goats[0].collision_half_depth ^= 1;
        assert_eq!(changed_depth.hash_v1(), reset_v1_hash);
        assert_eq!(changed_depth.hash_v2(reset_rng), reset_v2_hash);
        assert_ne!(
            changed_depth.hash_v3(reset_rng),
            reset_v3_hash,
            "v3 distinguishes the Level 2 collision-depth extension without changing v1/v2"
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
        assert_ne!(first.hash_v3(reset_rng), reset_v3_hash);
        assert_eq!(second.hash_v1(), reset_v1_hash);
        assert_eq!(second.hash_v2(reset_rng), reset_v2_hash);
        assert_eq!(second.hash_v3(reset_rng), reset_v3_hash);

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
        assert_eq!(first.hash_v3(reset_rng), reset_v3_hash);
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
        // sub_1BF90/sub_1C560 clear the owner word, retaining the payload.
        assert_eq!(goat.pending_damage, 601);
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
    fn vigilant_policy_runs_only_in_the_ordinary_patrol_phase_and_rejects_stale_targets() {
        let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let world = NativeWorld::load(
            private_root.join("generated/level1-export-m"),
            private_root.join("generated/level1-asset-closure.json"),
        )
        .expect("qualified Level 1 owner package");
        let mut vigilant_config = GoatBehaviorConfig::LEVEL_ONE;
        vigilant_config.perception_cadence = 1;
        vigilant_config.perception_radius = u16::MAX;
        vigilant_config.perception_half_angle = 1_025;
        vigilant_config.behavior_flags |= 0x10;
        let owner_player = &world.entities()[152];
        let player = GoatPlayerTarget {
            entity_index: owner_player.index,
            position: owner_player.position,
            control_flags: owner_player.control_flags,
        };

        let family =
            GoatFamily::from_entities(world.entities(), world.goat_stage_point(), vigilant_config)
                .expect("real owner Goat family with vigilant compiled policy");
        let mut acquired = family.clone();
        let events = acquired.step_with_players(world.terrain(), &[player]);
        let engaged = events
            .iter()
            .filter_map(|event| match event {
                GoatStepEvent::EngagementBoundaryReached { entity_index, .. } => {
                    Some(*entity_index)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!engaged.is_empty());
        for entity_index in &engaged {
            let goat = acquired
                .goats()
                .iter()
                .find(|goat| goat.entity_index == *entity_index)
                .unwrap();
            assert_eq!(goat.action, 14);
            assert_eq!(goat.stage_var_2, 10);
            assert_eq!(goat.target_entity_index, Some(player.entity_index));
        }

        let mut wrong_dispatch_phase = family;
        for goat in &mut wrong_dispatch_phase.goats {
            if goat.action == GOAT_PATROL_ACTION {
                goat.stage_var_2 = GOAT_PATROL_STAGE_MODE.wrapping_add(1);
            }
        }
        let events = wrong_dispatch_phase.step_with_players(world.terrain(), &[player]);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, GoatStepEvent::EngagementBoundaryReached { .. }))
        );

        // sub_1C980 rejects the hide bit, not the perception-only 0x20 bit.
        let mut perception_hidden = acquired.clone();
        perception_hidden.step_with_players(
            world.terrain(),
            &[GoatPlayerTarget {
                control_flags: 0x20,
                ..player
            }],
        );
        for entity_index in &engaged {
            assert_eq!(
                perception_hidden
                    .goats
                    .iter()
                    .find(|goat| goat.entity_index == *entity_index)
                    .unwrap()
                    .action,
                14,
            );
        }
        for rejected_flags in [0x0000_0400] {
            let mut following_pass = acquired.clone();
            following_pass.step_with_players(
                world.terrain(),
                &[GoatPlayerTarget {
                    control_flags: rejected_flags,
                    ..player
                }],
            );
            for entity_index in &engaged {
                let goat = following_pass
                    .goats()
                    .iter()
                    .find(|goat| goat.entity_index == *entity_index)
                    .unwrap();
                assert_eq!(goat.action, 9, "ineligible target must leave retaliation");
            }
        }

        let mut moved_player = acquired.clone();
        let index = moved_player
            .goats
            .iter()
            .position(|goat| goat.entity_index == engaged[0])
            .unwrap();
        moved_player.config.perception_cadence = 1;
        moved_player.config.perception_radius = 1_024;
        moved_player.goats[index].phase = 0;
        moved_player.goats[index].actual_speed = 0;
        let current_player = GoatPlayerTarget {
            position: moved_player.goats[index].position,
            ..player
        };
        moved_player.step_source_slot(engaged[0], world.terrain(), &[current_player]);
        assert_eq!(
            moved_player.goats[index].action, 14,
            "retaliation reads the current player position"
        );

        let mut missing_target = acquired;
        missing_target.step_with_players(world.terrain(), &[]);
        for entity_index in engaged {
            let goat = missing_target
                .goats()
                .iter()
                .find(|goat| goat.entity_index == entity_index)
                .unwrap();
            assert_eq!(goat.action, 9, "stale target must leave retaliation");
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
        // sub_1BF90/sub_1C560 clear the owner word, retaining the payload.
        assert_eq!(goat.pending_damage, 501);
        assert_eq!(goat.pending_attacker, 0);
        assert_eq!(goat.kill_credit_entity, 152);
        assert_eq!(goat.actual_speed, goat.maximum_speed);
    }

    #[test]
    fn action14_ordinary_and_direct_hit_share_the_modulo43_tail() {
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
        let goat_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 50)
            .expect("Level 1 Goat slot 50");
        let goat_position = family.goats[goat_index].position;
        let player = GoatPlayerTarget {
            entity_index: 152,
            position: goat_position,
            control_flags: 0,
        };

        // Every action-14 visit calls HitGoat_1F530. A modulo-77-only
        // seed must not sound, and retained retaliation preserves speed.
        family.goats[goat_index].action = 14;
        family.goats[goat_index].target_entity_index = Some(152);
        family.goats[goat_index].phase = 1;
        family.goats[goat_index].rng = 129;
        family.goats[goat_index].actual_speed = 0;
        let ordinary_events = family.step_source_slot(50, world.terrain(), &[player]);
        assert_eq!(family.goats[goat_index].action, 14);
        assert_eq!(family.goats[goat_index].rng, next_rng(129));
        assert_eq!(family.goats[goat_index].actual_speed, 0);
        assert!(!ordinary_events.iter().any(|event| matches!(
            event,
            GoatStepEvent::Bleat {
                entity_index: 50,
                sound_id: 46,
                action: 14,
                ..
            }
        )));

        // The ordinary visit also sounds for a modulo-43 seed.
        family.goats[goat_index].phase = 1;
        family.goats[goat_index].rng = 62;
        let ordinary_events = family.step_source_slot(50, world.terrain(), &[player]);
        assert!(ordinary_events.iter().any(|event| matches!(
            event,
            GoatStepEvent::Bleat {
                entity_index: 50,
                sound_id: 46,
                action: 14,
                ..
            }
        )));

        // Direct damage takes the same wrapper, including its RNG draw.
        family.goats[goat_index].action = 14;
        family.goats[goat_index].target_entity_index = Some(152);
        family.goats[goat_index].phase = 1;
        family.goats[goat_index].rng = 62;
        family.goats[goat_index].actual_speed = family.goats[goat_index].minimum_speed;
        family
            .inject_damage(50, 1, 152)
            .expect("direct action-14 hit");
        let hit_events = family.step_source_slot(50, world.terrain(), &[player]);
        assert_eq!(family.goats[goat_index].action, 14);
        assert_eq!(family.goats[goat_index].rng, next_rng(62));
        assert!(hit_events.iter().any(|event| matches!(
            event,
            GoatStepEvent::Bleat {
                entity_index: 50,
                sound_id: 46,
                action: 14,
                ..
            }
        )));

        family.goats[goat_index].action = 14;
        family.goats[goat_index].life = 1;
        family.goats[goat_index].phase = 1;
        family.goats[goat_index].rng = 62;
        family
            .inject_damage(50, 2, 152)
            .expect("lethal direct action-14 hit");
        let lethal_events = family.step_source_slot(50, world.terrain(), &[player]);
        assert_eq!(family.goats[goat_index].action, 12);
        assert_eq!(family.goats[goat_index].rng, next_rng(62));
        assert!(lethal_events.iter().any(|event| matches!(
            event,
            GoatStepEvent::Bleat {
                entity_index: 50,
                sound_id: 46,
                action: 12,
                ..
            }
        )));
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
        // sub_1BF90/sub_1C560 clear the owner word, retaining the payload.
        assert_eq!(goat.pending_damage, 100);
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
    fn action11_invalid_partner_damage_retaliates_against_a_live_non_player_source() {
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
        // sub_1BF90/sub_1C560 clear the owner word, retaining the payload.
        assert_eq!(primary.pending_damage, 100);
        assert_eq!(primary.pending_attacker, 0);
        // A non-zero but invalid relation still takes the damage branch. The
        // source row-98 behavior flag preserves the player-engagement bit,
        // so `sub_1C560(a2=8)` selects action 14 before the wrapper tail.
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
            .expect("primary retaliating Goat");
        // Source slot 7 is a live class-10 row: sub_1C980 accepts a live
        // non-player target and does not require the player-target slice.
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(7));
        assert_eq!(primary.phase, 2);
        assert_eq!(primary.rng, next_rng(rng_after_transition));
    }

    #[test]
    fn action11_global_partner_row_is_valid_and_mutated_on_hit() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let primary = &mut family.goats[primary_index];
        primary.action = 11;
        primary.phase = 0;
        primary.partner_entity_index = Some(80);
        primary.pending_damage = 1;
        primary.pending_attacker = 7;
        primary.current_attacker = 0;
        family.set_child_link_for(39, None);

        let partner = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("global partner source row");
        partner.class = GOAT_CLASS;
        partner.model = GOAT_MODEL;
        partner.active = true;
        partner.control_flags = 0;
        partner.life = 10_000;
        partner.action = 9;
        partner.behavior_flags = GoatBehaviorConfig::LEVEL_TWO.behavior_flags;
        partner.partner_entity_index = Some(66);
        partner.target_entity_index = None;

        family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);

        let primary = &family.goats[primary_index];
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(7));
        let partner = family
            .source_entities
            .iter()
            .find(|entity| entity.entity_index == 80)
            .expect("global partner source row");
        assert_eq!(partner.action, 14);
        assert_eq!(partner.target_entity_index, Some(7));
        assert_eq!(partner.partner_entity_index, None);
    }

    #[test]
    fn action11_global_partner_row_uses_its_own_retaliation_flag_on_hit() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let primary = &mut family.goats[primary_index];
        primary.action = 11;
        primary.phase = 0;
        primary.partner_entity_index = Some(80);
        primary.pending_damage = 1;
        primary.pending_attacker = 7;
        primary.current_attacker = 0;
        family.set_child_link_for(39, None);

        let partner = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("global partner source row");
        partner.class = GOAT_CLASS;
        partner.model = GOAT_MODEL;
        partner.active = true;
        partner.control_flags = 0;
        partner.life = 10_000;
        partner.action = 9;
        partner.behavior_flags = 0;
        partner.partner_entity_index = Some(66);
        partner.target_entity_index = None;

        family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);

        let primary = &family.goats[primary_index];
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(7));
        assert_eq!(primary.partner_entity_index, None);
        let partner = family
            .source_entities
            .iter()
            .find(|entity| entity.entity_index == 80)
            .expect("global partner source row");
        assert_eq!(partner.action, 10);
        assert_eq!(partner.target_entity_index, Some(7));
        assert_eq!(partner.partner_entity_index, None);
    }

    #[test]
    fn action11_global_partner_row_uses_source_action_and_target() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let primary = &mut family.goats[primary_index];
        primary.action = 11;
        primary.phase = 0;
        primary.partner_entity_index = Some(80);
        primary.pending_damage = 0;
        primary.pending_attacker = 0;
        primary.target_entity_index = None;
        family.set_child_link_for(39, None);

        let partner = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("global partner source row");
        partner.class = GOAT_CLASS;
        partner.model = GOAT_MODEL;
        partner.active = true;
        partner.control_flags = 0;
        partner.life = 10_000;
        partner.action = 10;
        partner.target_entity_index = Some(152);

        family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);

        let primary = &family.goats[primary_index];
        assert_eq!(primary.action, 10);
        assert_eq!(primary.target_entity_index, Some(152));
        assert_eq!(primary.partner_entity_index, None);
    }

    #[test]
    fn action11_global_partner_row_rejects_wrong_class_without_typed_fallback() {
        let mut family = level_two_family();
        let primary = family
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        primary.action = 11;
        primary.phase = 0;
        primary.partner_entity_index = Some(80);
        primary.pending_damage = 0;
        primary.pending_attacker = 0;
        family.set_child_link_for(39, None);

        let partner = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("global partner source row");
        partner.class = GOAT_CLASS.wrapping_add(1);
        partner.model = GOAT_MODEL;
        partner.active = true;
        partner.life = 10_000;
        partner.action = 10;
        partner.target_entity_index = Some(152);

        family.step_source_slot(39, &MaterialBoundaryWorld { all_water: false }, &[]);

        let primary = family
            .goats
            .iter()
            .find(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        assert_eq!(primary.action, 9);
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(primary.target_entity_index, None);
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
    fn action11_nonlethal_hit_uses_partner_66_and_exact_terrain_settle() {
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
            .position(|goat| goat.entity_index == 66)
            .expect("authored action-9 partner Goat");
        let primary_position = family.goats[primary_index].position;
        let partner_life = family.goats[partner_index].life;
        family.goats[primary_index].action = 11;
        family.goats[primary_index].phase = 0;
        family.goats[primary_index].partner_entity_index = Some(66);
        family.goats[primary_index].position.z = 300;
        family
            .inject_damage(39, 100, 152)
            .expect("nonlethal action-11 hit");

        family.step_source_slot(39, &world, &[]);

        let primary = &family.goats[primary_index];
        assert_eq!(primary.life, 500);
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(152));
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(primary.position.x, primary_position.x);
        assert_eq!(primary.position.y, primary_position.y);
        assert_eq!(primary.position.z, 44);
        // sub_1BF90/sub_1C560 clear the owner word, retaining the payload.
        assert_eq!(primary.pending_damage, 100);
        assert_eq!(primary.pending_attacker, 0);

        let partner = &family.goats[partner_index];
        assert_eq!(partner.life, partner_life);
        assert_eq!(partner.action, 14);
        assert_eq!(partner.target_entity_index, Some(152));
        assert_eq!(partner.partner_entity_index, None);
    }

    #[test]
    fn action10_missing_target_moves_and_falls_back_to_action9() {
        let world = OpenWorld;
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary Goat");
        family.goats[primary_index].action = 10;
        family.goats[primary_index].phase = 1;
        family.goats[primary_index].position = Position {
            x: 255,
            y: 1_000,
            z: 0,
        };
        family.goats[primary_index].yaw = 512;
        family.goats[primary_index].actual_speed = 1;
        family.goats[primary_index].target_entity_index = None;
        family.goats[primary_index].partner_entity_index = None;
        let rng = family.goats[primary_index].rng;

        let events = family.step_source_slot(39, &world, &[]);
        let primary = &family.goats[primary_index];
        assert_eq!(primary.action, 9);
        assert_eq!(primary.target_entity_index, None);
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(primary.actual_speed, primary.maximum_speed);
        assert_eq!(source_tile(primary.position).0, 1);
        assert_eq!(primary.rng, next_rng(rng));
        assert_eq!(primary.phase, 2);
        assert!(events.is_empty(), "the modulo-43 seed emits no bleat");
    }

    #[test]
    fn action9_peer_fallback_requires_proximity_and_uses_unpaired_peer() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 40)
            .expect("primary action-9 Goat");
        for goat in &mut family.goats {
            if !matches!(goat.entity_index, 40 | 41 | 42) {
                goat.position = Position {
                    x: 30_000,
                    y: 30_000,
                    z: 0,
                };
                goat.partner_entity_index = Some(66);
            }
        }
        family.goats[primary_index].action = 9;
        family.goats[primary_index].phase = 0;
        family.goats[primary_index].position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        family.goats[primary_index].yaw = 0;
        family.goats[primary_index].actual_speed = 0;
        family.goats[primary_index].proximity_countdown = 1;
        family.goats[primary_index].target_entity_index = None;
        family.goats[primary_index].partner_entity_index = None;
        family
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == 41)
            .expect("nearest unpaired peer")
            .position = Position {
            x: 10_000,
            y: 9_400,
            z: 0,
        };
        let paired = family
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == 42)
            .expect("paired peer");
        paired.position = Position {
            x: 10_000,
            y: 9_300,
            z: 0,
        };
        paired.partner_entity_index = Some(66);

        family.step_source_slot(40, &world, &[]);
        let primary = &family.goats[primary_index];
        assert_eq!(primary.action, 11);
        assert_eq!(primary.partner_entity_index, Some(41));
        assert_eq!(primary.target_entity_index, None);

        let mut gated = level_two_family();
        let gated_index = gated
            .goats
            .iter()
            .position(|goat| goat.entity_index == 40)
            .expect("gated action-9 Goat");
        for goat in &mut gated.goats {
            if !matches!(goat.entity_index, 40 | 41) {
                goat.position = Position {
                    x: 30_000,
                    y: 30_000,
                    z: 0,
                };
                goat.partner_entity_index = Some(66);
            }
        }
        gated.goats[gated_index].action = 9;
        gated.goats[gated_index].phase = 0;
        gated.goats[gated_index].position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        gated.goats[gated_index].yaw = 0;
        gated.goats[gated_index].actual_speed = 0;
        gated.goats[gated_index].proximity_countdown = 0;
        gated.goats[gated_index].partner_entity_index = None;
        gated
            .goats
            .iter_mut()
            .find(|goat| goat.entity_index == 41)
            .expect("gated peer")
            .position = Position {
            x: 10_000,
            y: 9_400,
            z: 0,
        };
        gated.step_source_slot(40, &world, &[]);
        let gated_primary = &gated.goats[gated_index];
        assert_eq!(gated_primary.action, 9);
        assert_eq!(gated_primary.partner_entity_index, None);
    }

    #[test]
    fn action10_child_chain_uses_sub_1c980_tracking_and_modulo43_tail() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary Goat");
        family.goats[primary_index].action = 10;
        family.goats[primary_index].phase = 0;
        family.goats[primary_index].life = 500;
        family.goats[primary_index].position.z = 300;
        family.goats[primary_index].target_entity_index = None;
        family.goats[primary_index].partner_entity_index = None;
        assert!(family.set_child_link_for(39, Some(80)));
        let child = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("global child source row");
        child.class = GOAT_CLASS.wrapping_add(1);
        child.model = 0;
        child.active = true;
        child.life = 499;
        child.current_attacker = 321;
        let rng = family.goats[primary_index].rng;

        family.step_source_slot(39, &world, &[]);
        let primary = &family.goats[primary_index];
        assert_eq!(primary.life, 499);
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(321));
        assert_eq!(primary.partner_entity_index, None);
        assert_eq!(primary.position.z, 44);
        assert_eq!(primary.actual_speed, primary.minimum_speed);
        assert_eq!(primary.pending_damage, 0);
        assert_eq!(primary.pending_attacker, 0);
        assert_eq!(primary.rng, next_rng(rng));
    }

    #[test]
    fn action10_lethal_child_hit_enters_action12_with_max_speed_and_kill_credit() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary Goat");
        family.goats[primary_index].action = 10;
        family.goats[primary_index].life = 1;
        family.goats[primary_index].target_entity_index = None;
        assert!(family.set_child_link_for(39, Some(80)));
        let child = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("global child source row");
        child.class = GOAT_CLASS.wrapping_add(1);
        child.model = 0;
        child.active = true;
        child.life = -1;
        child.current_attacker = 321;

        family.step_source_slot(39, &world, &[]);
        let primary = &family.goats[primary_index];
        assert_eq!(primary.life, -1);
        assert_eq!(primary.action, 12);
        assert_eq!(primary.kill_credit_entity, 321);
        assert_eq!(primary.actual_speed, primary.maximum_speed);
        assert_eq!(primary.target_entity_index, None);
    }

    #[test]
    fn action11_partner_action10_dispatches_hit_goat_next_turn() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut family = level_two_family();
        // Damage identities resolve in this Level 2 arena, not Level 1's
        // fixed player slot 152. A supplied target cannot revive a free row.
        let player_slot = family
            .source_entities
            .iter()
            .find(|actor| actor.active && actor.class == 3 && actor.model == 0)
            .expect("live Level 2 player")
            .entity_index;
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let partner_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 66)
            .expect("authored partner Goat");
        family.goats[primary_index].action = 11;
        family.goats[primary_index].partner_entity_index = Some(66);
        family.goats[partner_index].behavior_flags = 0;
        family
            .inject_damage(39, 100, player_slot.cast_signed())
            .expect("nonlethal action-11 partner hit");

        family.step_source_slot(39, &world, &[]);
        assert_eq!(family.goats[partner_index].action, 10);
        assert_eq!(
            family.goats[partner_index].target_entity_index,
            Some(player_slot)
        );

        let target_position = Position {
            x: family.goats[partner_index].position.x.wrapping_add(120),
            y: family.goats[partner_index].position.y.wrapping_add(80),
            z: family.goats[partner_index].position.z,
        };
        let players = [GoatPlayerTarget {
            entity_index: player_slot,
            position: target_position,
            control_flags: 0,
        }];
        family.step_source_slot(66, &world, &players);
        assert_eq!(family.goats[partner_index].action, 14);
        assert_eq!(
            family.goats[partner_index].roll,
            i16::try_from(
                heading_for_positions(family.goats[partner_index].position, target_position)
                    .wrapping_add(1_024)
                    & 0x07ff
            )
            .expect("11-bit action-10 retreat heading fits i16")
        );
        assert_eq!(
            family.goats[partner_index].actual_speed,
            family.goats[partner_index].minimum_speed
        );
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

    #[test]
    fn action11_child_chain_reads_global_non_goat_row() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        family.goats[primary_index].life = 400;
        family.goats[primary_index].pending_damage = 0;
        family.goats[primary_index].pending_attacker = 0;
        family.goats[primary_index].current_attacker = 0;
        assert!(family.set_child_link_for(39, Some(80)));

        let child = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("non-Goat source child row");
        child.class = GOAT_CLASS.wrapping_add(1);
        child.model = 0;
        child.life = 399;
        child.current_attacker = 321;
        child.active = true;

        assert_eq!(
            family.apply_action11_damage(primary_index),
            DamageResult::Hit(321),
            "action 11 adopts the first lower-life child from the global source chain"
        );
        assert_eq!(family.goats[primary_index].life, 399);
        assert_eq!(family.goats[primary_index].current_attacker, 321);
    }

    #[test]
    fn action9_child_chain_uses_source_damage_and_retaliation_transition() {
        let world = MaterialBoundaryWorld { all_water: false };
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 40)
            .expect("authored action-9 Goat");
        let primary_position = family.goats[primary_index].position;
        let primary_rng = family.goats[primary_index].rng;
        family.goats[primary_index].action = 9;
        family.goats[primary_index].phase = 1;
        family.goats[primary_index].position.z = 300;
        family.goats[primary_index].life = 400;
        family.goats[primary_index].pending_damage = 0;
        family.goats[primary_index].pending_attacker = 0;
        family.goats[primary_index].current_attacker = 0;
        assert!(family.set_child_link_for(40, Some(80)));

        let child = family
            .source_entities
            .iter_mut()
            .find(|entity| entity.entity_index == 80)
            .expect("non-Goat source child row");
        child.class = GOAT_CLASS.wrapping_add(1);
        child.model = 0;
        child.life = 399;
        child.current_attacker = 321;
        child.active = true;

        let events = family.step_source_slot(40, &world, &[]);
        let primary = &family.goats[primary_index];
        assert_eq!(primary.life, 399);
        assert_eq!(primary.action, 14);
        assert_eq!(primary.target_entity_index, Some(321));
        assert_eq!(primary.pending_damage, 0);
        assert_eq!(primary.pending_attacker, 0);
        assert_eq!(primary.position.x, primary_position.x);
        assert_eq!(primary.position.y, primary_position.y);
        assert_eq!(primary.position.z, 44);
        assert_eq!(primary.actual_speed, primary.minimum_speed);
        assert_eq!(primary.rng, next_rng(primary_rng));
        assert_eq!(primary.phase, 2);
        assert!(events.is_empty(), "the source tail seed emits no bleat");
    }

    #[test]
    fn action9_uses_source_intermediate_ceiling_probe_for_retry() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 40)
            .expect("authored action-9 Goat");
        let goat = &mut family.goats[primary_index];
        goat.action = 9;
        goat.phase = 1;
        goat.position = Position { x: 0, y: 0, z: 0 };
        goat.yaw = 0;
        goat.actual_speed = 300;
        goat.collision_half_width = 512;
        goat.collision_half_depth = 512;
        goat.collision_half_height = 10;
        goat.pending_damage = 0;
        goat.pending_attacker = 0;
        let world = IntermediateCeilingProbeWorld {
            ceiling_calls: Cell::new(0),
        };

        family.step_source_slot(40, &world, &[]);

        assert!(
            world.ceiling_calls.get() >= 4,
            "action 9 must use sub_1B8C0's intermediate ceiling probes"
        );
        assert_ne!(
            family.goats[primary_index].yaw,
            source_retry_headings(0)[0],
            "the first heading must be rejected before the retry path accepts"
        );
        assert_ne!(
            family.goats[primary_index].control_flags & 0x0004_0000,
            0,
            "a retry after the blocked first probe retains the obstruction marker"
        );
    }

    #[test]
    fn action11_partner_scan_uses_source_model_order_after_typed_slice_reorder() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let first_source_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 40)
            .expect("first model-list candidate");
        let second_source_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 41)
            .expect("second model-list candidate");
        family.goats[primary_index].position = Position {
            x: 1_000,
            y: 1_000,
            z: 0,
        };
        family.goats[first_source_index].position = Position {
            x: 1_100,
            y: 1_000,
            z: 0,
        };
        family.goats[second_source_index].position = Position {
            x: 1_010,
            y: 1_000,
            z: 0,
        };
        family.goats.swap(first_source_index, second_source_index);

        family.apply_partner_linked_scan(primary_index);
        assert_eq!(
            family.goats[primary_index].roll,
            i16::try_from(heading_for_positions(
                Position {
                    x: 1_100,
                    y: 1_000,
                    z: 0,
                },
                Position {
                    x: 1_000,
                    y: 1_000,
                    z: 0,
                },
            ))
            .expect("source model-list heading fits i16"),
            "source-pool order wins over typed-family storage order"
        );
    }

    #[test]
    fn action11_blocked_retries_retain_source_final_heading() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        family.goats[primary_index].position = Position {
            x: 512,
            y: 512,
            z: 0,
        };
        family.goats[primary_index].yaw = 512;
        family.goats[primary_index].actual_speed = 1_000;
        family.move_action11_source(primary_index, &MaterialBoundaryWorld { all_water: true });
        assert_eq!(
            family.goats[primary_index].yaw,
            source_retry_headings(512)[3],
            "sub_1B8C0 leaves the fourth failed retry heading in yaw"
        );
        assert_eq!(family.goats[primary_index].life, -1);
        assert_ne!(family.goats[primary_index].control_flags & 0x0004_0000, 0);
    }

    #[test]
    fn action11_first_tile_crossing_success_clears_obstruction_marker() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let goat = &mut family.goats[primary_index];
        goat.position = Position {
            x: 255,
            y: 1_000,
            z: 0,
        };
        goat.yaw = 512;
        goat.actual_speed = 1;
        goat.control_flags |= 0x0004_0000;

        family.move_action11_source(primary_index, &OpenWorld);

        let goat = &family.goats[primary_index];
        assert_eq!(source_tile(goat.position).0, 1);
        assert_eq!(goat.control_flags & 0x0004_0000, 0);
    }

    #[test]
    fn action11_same_tile_retry_after_blocked_first_probe_retains_marker() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let goat = &mut family.goats[primary_index];
        goat.position = Position {
            x: 255,
            y: 1_000,
            z: 0,
        };
        goat.yaw = 512;
        goat.actual_speed = 1;
        goat.control_flags |= 0x0004_0000;
        let world = FirstCrossingBlockedWorld {
            material_calls: Cell::new(0),
        };

        family.move_action11_source(primary_index, &world);

        let goat = &family.goats[primary_index];
        // Retry probes run unconditionally, even without another tile cross.
        assert_eq!(world.material_calls.get(), 2);
        assert_eq!(source_tile(goat.position).0, 0);
        assert_ne!(goat.control_flags & 0x0004_0000, 0);
    }

    #[test]
    fn action11_rejects_cave_angle_bit_eight_candidates() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        family.goats[primary_index].position = Position {
            x: 512,
            y: 512,
            z: 0,
        };
        family.goats[primary_index].yaw = 512;
        family.goats[primary_index].actual_speed = 1_000;
        family.move_action11_source(primary_index, &CaveBoundaryWorld);
        assert_eq!(family.goats[primary_index].life, -1);
        assert_eq!(
            family.goats[primary_index].yaw,
            source_retry_headings(512)[3]
        );
    }

    #[test]
    fn action11_sub102d0_rejects_intermediate_material_probe() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let goat = &mut family.goats[primary_index];
        goat.position = Position { x: 0, y: 0, z: 0 };
        goat.yaw = 0;
        goat.actual_speed = 300;
        goat.collision_half_width = 512;
        goat.collision_half_depth = 512;
        let world = IntermediateMaterialProbeWorld {
            material_calls: Cell::new(0),
        };

        family.move_action11_source(primary_index, &world);

        assert!(
            world.material_calls.get() >= 4,
            "source path probe must inspect an intermediate 256-unit sample"
        );
        assert_ne!(
            family.goats[primary_index].yaw,
            source_retry_headings(0)[0],
            "the first heading must be rejected before the retry path accepts"
        );
        assert_ne!(
            family.goats[primary_index].control_flags & 0x0004_0000,
            0,
            "a retry after the blocked first probe retains the obstruction marker"
        );
    }

    #[test]
    fn action11_sub102d0_rejects_intermediate_ceiling_probe() {
        let mut family = level_two_family();
        let primary_index = family
            .goats
            .iter()
            .position(|goat| goat.entity_index == 39)
            .expect("primary action-11 Goat");
        let goat = &mut family.goats[primary_index];
        goat.position = Position { x: 0, y: 0, z: 0 };
        goat.yaw = 0;
        goat.actual_speed = 300;
        goat.collision_half_width = 512;
        goat.collision_half_depth = 512;
        goat.collision_half_height = 10;
        let world = IntermediateCeilingProbeWorld {
            ceiling_calls: Cell::new(0),
        };

        family.move_action11_source(primary_index, &world);

        assert!(
            world.ceiling_calls.get() >= 4,
            "source path probe must inspect an intermediate cave sample"
        );
        assert_ne!(
            family.goats[primary_index].yaw,
            source_retry_headings(0)[0],
            "the first heading must be rejected by the cave ceiling probe"
        );
    }
}
