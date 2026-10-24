//! Legacy Level 1 world composition. Goat state and behavior live in creatures::goat.

pub use crate::native_world::creatures::goat::*;
use crate::{
    FlightWorld, Position,
    goat_map_chain::{GoatMapChains, GoatMapSeed},
    house::{HouseFamily, HouseResidentEntry, HouseState, HouseStepEvent},
    level1_mission::{HouseConstructionSourceEffect, Level1SourceEntityView},
    native_world::EntitySnapshot,
    villager::{VillagerBehaviorConfig, VillagerFamily, VillagerState, VillagerStepEffect},
};

/// Complete mutable non-player state owned by one native play session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveWorldState {
    pub(crate) goats: GoatFamily,
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

    pub(crate) fn submit_source_damage(&mut self, slot: u16, payload: u16, owner: u16) -> bool {
        if self
            .houses
            .houses()
            .iter()
            .any(|house| house.active && house.entity_index == slot)
        {
            self.houses.submit_source_damage(slot, payload, owner)
        } else if self
            .villagers
            .villagers()
            .iter()
            .any(|villager| villager.active && villager.entity_index == slot)
        {
            self.villagers.submit_source_damage(slot, payload, owner)
        } else {
            self.goats.submit_source_damage(slot, payload, owner)
        }
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

    /// Extend the compatibility-preserving v2 hash with the Goat collision
    /// depth added for the Level 2 source collision probe.
    ///
    /// The field cannot be inserted into the frozen v1 byte stream: v2 is
    /// embedded in accepted Level 1 event and render receipts. Pairing each
    /// depth with its entity identity makes this extension order-explicit.
    #[must_use]
    pub fn hash_v3(&self, global_rng: u32) -> u64 {
        let mut hash = Fnv1a64::new();
        hash.bytes(b"mc2-live-world-v3\0");
        hash.u64(self.hash_v2(global_rng));
        hash.u32(u32::try_from(self.goats().len()).expect("Goat count fits u32"));
        for goat in self.goats() {
            hash.u16(goat.entity_index);
            hash.i16(goat.collision_half_depth);
        }
        // Accepted Level 1 reset rows have model13, null links and byte39=63
        // after the source reset pass. Keep those receipts byte-identical;
        // distinguish newly retained script/link state in the current hash.
        for villager in self.villagers().iter().filter(|row| {
            row.model != 13
                || row.linked_entity_index != 0
                || row.stage != Default::default()
                || row.source_byte_39 != 63
        }) {
            hash.bytes(b"settlement-stage-v1\0");
            hash.u16(villager.entity_index);
            hash.u8(villager.model);
            hash.u16(villager.linked_entity_index);
            hash.u16(villager.stage.parent);
            hash.u16(villager.stage.relation);
            hash.i16(villager.stage.remaining);
            hash.i16(villager.stage.duration);
            hash.u8(villager.source_byte_39);
        }
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
