//! Source-ordered Level 1 townie/villager simulation.
//!
//! This is the bounded class-5/model-13 path used by Level 1: initial action
//! 104, free-townie aliases 105-107, death actions 108-109, hit action 110,
//! and staged action 111.

use crate::{
    FlightWorld, Position, heading_for_positions, house::HouseState, move_entity,
    native_world::EntitySnapshot, turn_toward, wrapping_i16,
};

pub const LEVEL_ONE_VILLAGER_INDICES: [u16; 28] = [
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74,
    75, 76, 77, 78,
];
pub const LEVEL_ONE_BUILDING_INDICES: [u16; 47] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
];
pub const VILLAGER_CLASS: u8 = 5;
pub const VILLAGER_MODEL: u8 = 13;
pub const VILLAGER_INITIAL_ACTION: u8 = 104;
pub const VILLAGER_STAGED_ACTION: u8 = 111;
pub const VILLAGER_FREE_ACTION: u8 = 105;
pub const VILLAGER_FREE_ALIAS_ACTION_A: u8 = 106;
pub const VILLAGER_FREE_ALIAS_ACTION_B: u8 = 107;
pub const VILLAGER_HIT_ACTION: u8 = 110;
pub const VILLAGER_DEATH_ACTION: u8 = 108;
pub const VILLAGER_REMOVAL_ACTION: u8 = 109;
pub const VILLAGER_PARAMETER_INDEX: u16 = 100;

const ELIGIBLE_BUILDING_INDICES: [u16; 41] = [
    1, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
    31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VillagerBehaviorConfig {
    pub turn_clamp: i16,
    pub ground_clearance: i16,
    pub vertical_relaxation: i16,
    pub terrain_threshold: i16,
    pub allowed_material_mask: u32,
    pub behavior_cadence: u16,
    pub behavior_flags: u8,
}

impl VillagerBehaviorConfig {
    pub const LEVEL_ONE: Self = Self {
        turn_clamp: 22,
        ground_clearance: 0,
        vertical_relaxation: -256,
        terrain_threshold: 15,
        allowed_material_mask: 0xffff_fefe,
        behavior_cadence: 40,
        behavior_flags: 9,
    };

    pub fn from_parameter_record(bytes: &[u8; 34]) -> Result<Self, String> {
        let config = Self {
            turn_clamp: read_i16(bytes, 2),
            ground_clearance: read_i16(bytes, 12),
            vertical_relaxation: read_i16(bytes, 14),
            terrain_threshold: read_i16(bytes, 16),
            allowed_material_mask: read_u32(bytes, 20),
            behavior_cadence: read_u16(bytes, 26),
            behavior_flags: bytes[32],
        };
        if config != Self::LEVEL_ONE {
            return Err(format!(
                "Level 1 Villager parameter row 100 mismatch: {config:?}"
            ));
        }
        Ok(config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VillagerState {
    pub entity_index: u16,
    pub source_id: u16,
    pub active: bool,
    pub control_flags: u32,
    pub reset_counter: i32,
    pub max_life: i32,
    pub life: i32,
    pub pending_damage: i32,
    pub pending_attacker: i16,
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
    pub stage_var_1: i8,
    pub stage_var_2: i8,
    pub actual_speed: i16,
    pub minimum_speed: i16,
    pub maximum_speed: i16,
    pub target_entity_index: Option<u16>,
}

impl VillagerState {
    pub fn from_snapshot(snapshot: &EntitySnapshot) -> Result<Self, String> {
        if snapshot.class != VILLAGER_CLASS || snapshot.model != VILLAGER_MODEL {
            return Err(format!(
                "entity {} is {}/{}, not Villager {VILLAGER_CLASS}/{VILLAGER_MODEL}",
                snapshot.index, snapshot.class, snapshot.model
            ));
        }
        let bytes = snapshot.record_bytes();
        if bytes.len() != 176 {
            return Err(format!(
                "exported Villager entity {} must contain 176 bytes, got {}",
                snapshot.index,
                bytes.len()
            ));
        }
        let render_parameter_index = read_u16(bytes, 94);
        if !matches!(render_parameter_index, 239 | 241 | 242 | 271) {
            return Err(format!(
                "Villager entity {} has unexpected render parameter {render_parameter_index}",
                snapshot.index
            ));
        }
        Ok(Self {
            entity_index: snapshot.index,
            source_id: read_u16(bytes, 30),
            active: snapshot.active,
            control_flags: read_u32(bytes, 16),
            reset_counter: read_i32(bytes, 20),
            max_life: read_i32(bytes, 8),
            life: read_i32(bytes, 12),
            pending_damage: read_i32(bytes, 98),
            pending_attacker: read_i16(bytes, 102),
            current_attacker: read_i16(bytes, 42),
            rng: read_u16(bytes, 24),
            yaw: read_i16(bytes, 32),
            pitch: read_i16(bytes, 34),
            roll: read_i16(bytes, 36),
            position: snapshot.position,
            collision_vertical_offset: read_i16(bytes, 86),
            collision_half_width: read_i16(bytes, 88),
            collision_half_height: read_i16(bytes, 92),
            render_parameter_index,
            animation_frame: bytes[96].cast_signed(),
            action: snapshot.action,
            phase: snapshot.phase,
            stage_var_1: bytes[76].cast_signed(),
            stage_var_2: bytes[77].cast_signed(),
            actual_speed: read_i16(bytes, 134),
            minimum_speed: read_i16(bytes, 136),
            maximum_speed: read_i16(bytes, 138),
            target_entity_index: relation_index(read_u16(bytes, 154)),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VillagerBuilding {
    entity_index: u16,
    position: Position,
    capacity: i16,
    occupants: i32,
    eligible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VillagerFamily {
    villagers: Box<[VillagerState]>,
    buildings: Box<[VillagerBuilding]>,
    config: VillagerBehaviorConfig,
}

impl VillagerFamily {
    pub fn from_entities(
        entities: &[EntitySnapshot],
        config: VillagerBehaviorConfig,
    ) -> Result<Self, String> {
        let mut villagers = Vec::with_capacity(LEVEL_ONE_VILLAGER_INDICES.len());
        for expected_index in LEVEL_ONE_VILLAGER_INDICES {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Villager entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "Villager pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            let state = VillagerState::from_snapshot(snapshot)?;
            if state.action != VILLAGER_STAGED_ACTION
                || state.stage_var_1 != 2
                || state.stage_var_2 != 7
            {
                return Err(format!(
                    "Villager entity {expected_index} reset state changed: action={} stage={}/{}",
                    state.action, state.stage_var_1, state.stage_var_2
                ));
            }
            villagers.push(state);
        }

        let mut buildings = Vec::with_capacity(LEVEL_ONE_BUILDING_INDICES.len());
        for entity_index in LEVEL_ONE_BUILDING_INDICES {
            let snapshot = entities
                .get(usize::from(entity_index))
                .ok_or_else(|| format!("missing Level 1 building entity {entity_index}"))?;
            let bytes = snapshot.record_bytes();
            buildings.push(VillagerBuilding {
                entity_index,
                position: snapshot.position,
                capacity: read_i16(bytes, 136),
                occupants: read_i32(bytes, 20),
                eligible: snapshot.active
                    && snapshot.class == 10
                    && snapshot.model == 45
                    && ELIGIBLE_BUILDING_INDICES.contains(&entity_index),
            });
        }

        Ok(Self {
            villagers: villagers.into_boxed_slice(),
            buildings: buildings.into_boxed_slice(),
            config,
        })
    }

    #[must_use]
    pub fn villagers(&self) -> &[VillagerState] {
        &self.villagers
    }

    pub fn sync_houses(&mut self, houses: &[HouseState]) {
        for building in &mut self.buildings {
            let Some(house) = houses
                .iter()
                .find(|house| house.entity_index == building.entity_index)
            else {
                building.eligible = false;
                continue;
            };
            building.position = house.position;
            building.capacity = house.capacity;
            building.occupants = house.occupants;
            building.eligible = house.active && house.parameter.produces_villagers();
        }
    }

    /// `sub_12500` class-5 stage service for Level 1's kind-7 broadcast.
    /// This runs before source slot one and does not consume an entity phase.
    pub fn release_stage_group_7(&mut self) -> usize {
        let mut released = 0;
        for villager in &mut self.villagers {
            if villager.active && villager.action == VILLAGER_STAGED_ACTION {
                villager.action = VILLAGER_FREE_ACTION;
                released += 1;
            }
        }
        released
    }

    pub fn step_source_slot<W: FlightWorld>(&mut self, source_slot: u16, world: &W) -> Option<u16> {
        let Some(index) = self
            .villagers
            .iter()
            .position(|villager| villager.entity_index == source_slot && villager.active)
        else {
            return None;
        };
        let entered_building = match self.villagers[index].action {
            VILLAGER_INITIAL_ACTION => {
                self.villagers[index].action = VILLAGER_FREE_ACTION;
                self.step_free(index, world)
            }
            VILLAGER_STAGED_ACTION => {
                self.step_scripted(index, world);
                None
            }
            VILLAGER_FREE_ACTION | VILLAGER_FREE_ALIAS_ACTION_A | VILLAGER_FREE_ALIAS_ACTION_B => {
                self.villagers[index].action = VILLAGER_FREE_ACTION;
                self.step_free(index, world)
            }
            VILLAGER_HIT_ACTION => {
                self.step_hit(index);
                None
            }
            VILLAGER_DEATH_ACTION => {
                self.step_death(index);
                None
            }
            VILLAGER_REMOVAL_ACTION => {
                self.step_removal(index);
                None
            }
            _ => return None,
        };
        self.villagers[index].phase = self.villagers[index].phase.wrapping_add(1);
        entered_building
    }

    pub fn inject_damage(
        &mut self,
        entity_index: u16,
        damage: i32,
        attacker: i16,
    ) -> Result<(), String> {
        if damage <= 0 || attacker == 0 {
            return Err("damage must be positive and attacker must be nonzero".to_owned());
        }
        let villager = self
            .villagers
            .iter_mut()
            .find(|villager| villager.entity_index == entity_index && villager.active)
            .ok_or_else(|| format!("Villager entity {entity_index} is not live"))?;
        villager.pending_damage = villager.pending_damage.wrapping_add(damage);
        villager.pending_attacker = attacker;
        Ok(())
    }

    fn step_scripted<W: FlightWorld>(&mut self, index: usize, world: &W) {
        if self.apply_damage(index) {
            return;
        }
        self.move_actor(index, world);
        if self.villagers[index].phase & 0x0f == 0
            && self.villagers[index].control_flags.to_le_bytes()[2] & 4 == 0
        {
            let rng = next_rng(self.villagers[index].rng);
            self.villagers[index].rng = rng;
            self.villagers[index].roll = self.villagers[index]
                .roll
                .wrapping_add(i16::try_from(rng % 113 + 142).expect("wander delta fits i16"))
                & 0x07ff;
        }
        self.villagers[index].actual_speed = if self.villagers[index].action == VILLAGER_HIT_ACTION
        {
            self.villagers[index].minimum_speed
        } else {
            self.villagers[index].maximum_speed
        };
    }

    fn step_free<W: FlightWorld>(&mut self, index: usize, world: &W) -> Option<u16> {
        if self.apply_damage(index) {
            return None;
        }
        self.move_actor(index, world);
        if !u16::from(self.villagers[index].phase).is_multiple_of(self.config.behavior_cadence) {
            return None;
        }

        if let Some(target_index) = self.villagers[index].target_entity_index {
            let Some(building_index) = self
                .buildings
                .iter()
                .position(|building| building.entity_index == target_index && building.eligible)
            else {
                self.villagers[index].target_entity_index = None;
                self.villagers[index].actual_speed = self.villagers[index].maximum_speed;
                return None;
            };
            let target = self.buildings[building_index];
            if wrapped_distance_3d(self.villagers[index].position, target.position) > 2_048 {
                self.villagers[index].roll = i16::try_from(heading_for_positions(
                    self.villagers[index].position,
                    target.position,
                ))
                .expect("11-bit heading fits i16");
            } else if i32::from(target.capacity) > target.occupants {
                self.villagers[index].reset_counter = 1;
                self.villagers[index].action = VILLAGER_DEATH_ACTION;
                self.buildings[building_index].occupants =
                    self.buildings[building_index].occupants.wrapping_add(1);
                return Some(target_index);
            } else {
                self.villagers[index].target_entity_index = None;
                self.villagers[index].actual_speed = self.villagers[index].maximum_speed;
            }
            return None;
        }

        let first = next_rng(self.villagers[index].rng);
        let second = next_rng(first);
        self.villagers[index].rng = second;
        let sign = 2 * (i32::from(first % 157) / 79) - 1;
        let magnitude = i32::from(second & 0xff) + 85;
        self.villagers[index].roll =
            wrapping_i16(i64::from(self.villagers[index].roll) + i64::from(sign * magnitude))
                & 0x07ff;

        let position = self.villagers[index].position;
        self.villagers[index].target_entity_index = self
            .buildings
            .iter()
            .filter(|building| building.eligible)
            .min_by_key(|building| squared_wrapping_xy_distance(position, building.position))
            .map(|building| building.entity_index);
        if self.villagers[index].target_entity_index.is_some() {
            self.villagers[index].actual_speed =
                self.villagers[index].maximum_speed.wrapping_add(12);
        }
        None
    }

    fn step_hit(&mut self, index: usize) {
        if self.apply_damage(index) {
            return;
        }
        self.villagers[index].target_entity_index = None;
        self.villagers[index].action = VILLAGER_FREE_ACTION;
        self.villagers[index].actual_speed = self.villagers[index].maximum_speed;
    }

    fn step_death(&mut self, index: usize) {
        if self.villagers[index].reset_counter != 0 {
            self.villagers[index].active = false;
            self.villagers[index].control_flags |= 0x0000_0400;
        } else {
            // KillTownie_23680 delegates ordinary deaths to
            // PreKillEntity_1C890(entity, 104), selecting action 109.
            self.villagers[index].action = VILLAGER_REMOVAL_ACTION;
        }
    }

    fn step_removal(&mut self, index: usize) {
        // KillEntity_1C930 only performs its conversion/effect/removal work
        // on the entity's source phase cadence.
        if self.villagers[index].phase & 7 == 0 {
            self.villagers[index].active = false;
            self.villagers[index].control_flags |= 0x0000_0400;
        }
    }

    fn apply_damage(&mut self, index: usize) -> bool {
        let villager = &mut self.villagers[index];
        if villager.pending_attacker != 0 {
            villager.life = villager.life.wrapping_sub(villager.pending_damage);
            villager.current_attacker = villager.pending_attacker;
            villager.pending_damage = 0;
            villager.pending_attacker = 0;
            villager.target_entity_index = Some(villager.current_attacker.cast_unsigned());
            villager.action = if villager.life < 0 {
                VILLAGER_DEATH_ACTION
            } else {
                VILLAGER_HIT_ACTION
            };
            return true;
        }
        villager.current_attacker = 0;
        false
    }

    fn move_actor<W: FlightWorld>(&mut self, index: usize, world: &W) {
        let original = self.villagers[index].position;
        let original_yaw = self.villagers[index].yaw;
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
                self.villagers[index].actual_speed,
            );
            let crossed_tile = source_tile(candidate) != source_tile(original);
            let blocked = (attempt != 0 || crossed_tile)
                && (material_probe_blocked(
                    candidate,
                    heading,
                    self.villagers[index].collision_half_width,
                    self.config.allowed_material_mask,
                    world,
                ) || terrain_slope_measure(candidate, world)
                    >= i32::from(self.config.terrain_threshold));
            if blocked {
                self.villagers[index].control_flags |= 0x0004_0000;
                self.villagers[index].yaw = heading;
                continue;
            }
            let villager = &mut self.villagers[index];
            villager.position = candidate;
            villager.yaw =
                heading.wrapping_add(turn_toward(heading, villager.roll, self.config.turn_clamp))
                    & 0x07ff;
            if attempt == 0 {
                villager.control_flags &= !0x0004_0000;
            }
            return;
        }
        if self.config.behavior_flags & 1 != 0
            || world.material_mask(self.villagers[index].position) == 1
        {
            self.villagers[index].life = -1;
        }
    }
}

fn next_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
}

const fn relation_index(value: u16) -> Option<u16> {
    if value == 0 { None } else { Some(value) }
}

fn source_tile(position: Position) -> (u8, u8) {
    (position.x.to_be_bytes()[0], position.y.to_be_bytes()[0])
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

fn wrapped_distance_3d(left: Position, right: Position) -> u32 {
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
    (point_1 + point_4 - point_2 - point_3)
        .abs()
        .max((point_1 + point_2 - point_4 - point_3).abs())
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
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed span"))
}

fn read_i16(bytes: &[u8], offset: usize) -> i16 {
    read_u16(bytes, offset).cast_signed()
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed span"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed span"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::native_world::NativeWorld;

    fn load_world() -> NativeWorld {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        NativeWorld::load(
            root.join("level1-export-n"),
            root.join("level1-asset-closure.json"),
        )
        .expect("qualified owner package")
    }

    #[test]
    fn exact_level_one_reset_decodes_all_twenty_eight_villagers() {
        let world = load_world();
        let family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        assert_eq!(family.villagers().len(), 28);
        for (ordinal, villager) in family.villagers().iter().enumerate() {
            assert_eq!(villager.entity_index, 51 + ordinal as u16);
            assert_eq!(villager.action, 111);
            assert_eq!(villager.phase, 1 + ordinal as u8);
            assert_eq!(villager.actual_speed, 18);
            assert_eq!(villager.minimum_speed, 54);
            assert_eq!(villager.maximum_speed, 18);
            assert_eq!(villager.stage_var_1, 2);
            assert_eq!(villager.stage_var_2, 7);
        }
    }

    #[test]
    fn row_one_hundred_is_pinned() {
        let bytes = [
            41, 0, 22, 0, 5, 0, 22, 0, 5, 0, 0, 1, 0, 0, 0, 255, 15, 0, 0, 2, 254, 254, 255, 255,
            0, 9, 40, 0, 0, 14, 0, 2, 9, 0,
        ];
        assert_eq!(
            VillagerBehaviorConfig::from_parameter_record(&bytes),
            Ok(VillagerBehaviorConfig::LEVEL_ONE)
        );
    }

    #[test]
    fn staged_villagers_move_and_advance_in_source_order() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        let before = family.villagers()[0].clone();
        family.step_source_slot(51, world.terrain());
        let after = &family.villagers()[0];
        assert_eq!(after.phase, before.phase.wrapping_add(1));
        assert_ne!(after.position, before.position);
        assert_eq!(family.villagers()[1].phase, 2);
    }

    #[test]
    fn lethal_damage_uses_source_actions_108_then_109_then_phase_gated_removal() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        // Exercise KillTownie_23680's ordinary combat-death branch rather
        // than its nonzero dword_0x10 immediate-disable branch.
        family.villagers[0].reset_counter = 0;
        family
            .inject_damage(51, family.villagers()[0].life + 1, 152)
            .expect("lethal hit");

        family.step_source_slot(51, world.terrain());
        assert_eq!(family.villagers()[0].action, VILLAGER_DEATH_ACTION);
        assert!(family.villagers()[0].active);

        family.step_source_slot(51, world.terrain());
        assert_eq!(family.villagers()[0].action, VILLAGER_REMOVAL_ACTION);
        assert!(family.villagers()[0].active);

        while family.villagers()[0].active {
            family.step_source_slot(51, world.terrain());
        }
        assert_eq!(family.villagers()[0].phase & 7, 1);
        assert_ne!(family.villagers()[0].control_flags & 0x0000_0400, 0);
    }

    #[test]
    fn source_alias_actions_normalize_to_free_townie_action_105() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        for action in [
            VILLAGER_INITIAL_ACTION,
            VILLAGER_FREE_ALIAS_ACTION_A,
            VILLAGER_FREE_ALIAS_ACTION_B,
        ] {
            family.villagers[0].action = action;
            family.step_source_slot(51, world.terrain());
            assert_eq!(family.villagers()[0].action, VILLAGER_FREE_ACTION);
        }
    }

    #[test]
    fn group_7_stage_broadcast_releases_villagers_without_advancing_phase() {
        let world = load_world();
        let mut family =
            VillagerFamily::from_entities(world.entities(), VillagerBehaviorConfig::LEVEL_ONE)
                .expect("villager family");
        let phases = family
            .villagers()
            .iter()
            .map(|villager| villager.phase)
            .collect::<Vec<_>>();

        assert_eq!(family.release_stage_group_7(), 28);
        assert!(
            family
                .villagers()
                .iter()
                .all(|villager| villager.action == VILLAGER_FREE_ACTION)
        );
        assert_eq!(
            family
                .villagers()
                .iter()
                .map(|villager| villager.phase)
                .collect::<Vec<_>>(),
            phases
        );
        assert_eq!(family.release_stage_group_7(), 0);
    }
}
