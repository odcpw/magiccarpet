//! Source-ordered Level 1 house simulation.
//!
//! The forty-seven imported class-10/model-45 rows run source action 52
//! (`AddHouse0A_2D_38330`). Fatal damage selects action 53; the following
//! source dispatch removes the BUILD footprint. Cross-system effects are
//! emitted as events so arena allocation and terrain mutation stay owned by
//! the native session and Level 1 mission.

use crate::{FlightWorld, Position, native_world::EntitySnapshot};

pub const LEVEL_ONE_HOUSE_INDICES: std::ops::RangeInclusive<u16> = 1..=47;
pub const HOUSE_CLASS: u8 = 10;
pub const HOUSE_MODEL: u8 = 45;
pub const HOUSE_BUILD_ACTION: u8 = 44;
pub const HOUSE_TERRAIN_REMOVE_ACTION: u8 = 48;
pub const HOUSE_TERRAIN_TO_LIVE_ACTION: u8 = 51;
pub const HOUSE_LIVE_ACTION: u8 = 52;
pub const HOUSE_REMOVE_ACTION: u8 = 53;

const SOURCE_DISABLED_FLAG: u32 = 0x0000_0400;
const POSSESSED_FLAG: u32 = 0x0020_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HouseParameter {
    pub life_mana_scalar: u16,
    pub flags: u8,
    pub destruction_successor: u8,
}

impl HouseParameter {
    #[must_use]
    pub const fn level_one(building_index: u8) -> Option<Self> {
        let (life_mana_scalar, flags, destruction_successor) = match building_index {
            17 => (500, 0x18, 0),
            19 => (9, 0x03, 0),
            22 => (15, 0x03, 0),
            26 => (20, 0x03, 0),
            29 => (28, 0x03, 0),
            30 => (25, 0x03, 0),
            31 => (29, 0x03, 0),
            32 => (51, 0x03, 0),
            33 => (87, 0x03, 0),
            34 => (118, 0x03, 0),
            35 => (28, 0x03, 0),
            // BLDGPRM row 37 is the authored group-13-through-32 house.
            // Its source successor byte deliberately points back to row 37.
            37 => (20, 0x08, 37),
            39 => (30, 0x03, 0),
            40 => (100, 0x03, 0),
            42 => (52, 0x03, 0),
            47 => (34, 0x03, 0),
            50 => (24, 0x03, 0),
            51..=53 => (30, 0x08, 0),
            62 => (215, 0x03, 0),
            74 => (245, 0x03, 0),
            _ => return None,
        };
        Some(Self {
            life_mana_scalar,
            flags,
            destruction_successor,
        })
    }

    #[must_use]
    pub const fn produces_villagers(self) -> bool {
        self.flags & 1 != 0
    }

    /// The Level 2 Day package uses the live BLDGPRM rows selected by its
    /// authored houses.  Keep this table separate from the sealed Level 1
    /// mapping: the Level 2 rows are all construction/possession blocked and
    /// carry their own scalar/successor values.
    #[must_use]
    pub const fn level_two(building_index: u8) -> Option<Self> {
        let (life_mana_scalar, flags, destruction_successor) = match building_index {
            26 => (20, 0x03, 0),
            29 => (28, 0x03, 0),
            30 => (25, 0x03, 0),
            44 => (195, 0x08, 45),
            50 => (24, 0x03, 0),
            55 => (60, 0x08, 55),
            59 => (53, 0x08, 59),
            60 => (41, 0x08, 60),
            64 => (410, 0x08, 64),
            _ => return None,
        };
        Some(Self {
            life_mana_scalar,
            flags,
            destruction_successor,
        })
    }

    #[must_use]
    pub const fn possession_blocked(self) -> bool {
        self.flags & 8 != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HouseState {
    pub entity_index: u16,
    pub source_id: u16,
    pub active: bool,
    pub control_flags: u32,
    pub max_life: i32,
    pub life: i32,
    pub occupants: i32,
    pub rng: u16,
    pub position: Position,
    pub collision_vertical_offset: i16,
    pub collision_half_width: i16,
    pub collision_half_depth: i16,
    pub collision_half_height: i16,
    pub action: u8,
    pub phase: u8,
    pub xtype: u8,
    /// Source byte `0x3b`: action 44 selects a -25 rather than -1 tail.
    pub build_tail: u8,
    pub building_index: u8,
    pub capacity: i16,
    pub owner_entity_index: u16,
    pub pending_damage: i32,
    pub pending_attacker: i16,
    pub pending_possession_owner: u16,
    pub pending_possession_payload: u32,
    pub parameter: HouseParameter,
}

impl HouseState {
    /// Decode one validated exported source row.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, record size, action, or the pinned
    /// BLDGPRM mapping differs from the qualified Level 1 reset.
    pub fn from_snapshot(snapshot: &EntitySnapshot) -> Result<Self, String> {
        if !LEVEL_ONE_HOUSE_INDICES.contains(&snapshot.index)
            || snapshot.class != HOUSE_CLASS
            || snapshot.model != HOUSE_MODEL
        {
            return Err(format!(
                "entity {} is {}/{}, not a Level 1 house {HOUSE_CLASS}/{HOUSE_MODEL}",
                snapshot.index, snapshot.class, snapshot.model
            ));
        }
        let bytes = snapshot.record_bytes();
        if bytes.len() != 176 {
            return Err(format!(
                "exported house entity {} must contain 176 bytes, got {}",
                snapshot.index,
                bytes.len()
            ));
        }
        let building_index = bytes[74];
        let parameter = HouseParameter::level_one(building_index).ok_or_else(|| {
            format!(
                "house entity {} uses unqualified BLDGPRM row {building_index}",
                snapshot.index
            )
        })?;
        if snapshot.action != HOUSE_LIVE_ACTION {
            return Err(format!(
                "house entity {} reset action is {}, expected {HOUSE_LIVE_ACTION}",
                snapshot.index, snapshot.action
            ));
        }
        Ok(Self {
            entity_index: snapshot.index,
            source_id: snapshot.source_id,
            active: snapshot.active,
            control_flags: snapshot.control_flags,
            max_life: snapshot.max_life,
            life: snapshot.life,
            occupants: read_i32(bytes, 20),
            rng: snapshot.entity_rng,
            position: snapshot.position,
            collision_vertical_offset: read_i16(bytes, 86),
            collision_half_width: read_i16(bytes, 88),
            collision_half_depth: read_i16(bytes, 90),
            collision_half_height: read_i16(bytes, 92),
            action: snapshot.action,
            phase: snapshot.phase,
            xtype: bytes[69],
            build_tail: bytes[59],
            building_index,
            capacity: read_i16(bytes, 136),
            owner_entity_index: read_u16(bytes, 152),
            pending_damage: read_i32(bytes, 98),
            pending_attacker: read_i16(bytes, 102),
            pending_possession_payload: read_u32(bytes, 104),
            pending_possession_owner: read_u16(bytes, 108),
            parameter,
        })
    }

    /// Decode one of Level 2's ordinary class-10/model-45 reset houses using
    /// the package-selected BLDGPRM row.  The common state layout and all
    /// action-51/52/53 behavior remain the shared source implementation.
    pub fn from_level_two_snapshot(snapshot: &EntitySnapshot) -> Result<Self, String> {
        if !(1..=22).contains(&snapshot.index)
            || snapshot.class != HOUSE_CLASS
            || snapshot.model != HOUSE_MODEL
        {
            return Err(format!(
                "entity {} is {}/{}, not a Level 2 ordinary house",
                snapshot.index, snapshot.class, snapshot.model
            ));
        }
        if snapshot.action != HOUSE_LIVE_ACTION {
            return Err(format!(
                "Level 2 house entity {} reset action is {}, expected {HOUSE_LIVE_ACTION}",
                snapshot.index, snapshot.action
            ));
        }
        let bytes = snapshot.record_bytes();
        if bytes.len() != 176 {
            return Err(format!(
                "Level 2 house entity {} must contain 176 bytes, got {}",
                snapshot.index,
                bytes.len()
            ));
        }
        let building_index = bytes[74];
        let parameter = HouseParameter::level_two(building_index).ok_or_else(|| {
            format!(
                "Level 2 house entity {} uses unqualified BLDGPRM row {building_index}",
                snapshot.index
            )
        })?;
        Ok(Self {
            entity_index: snapshot.index,
            source_id: snapshot.source_id,
            active: snapshot.active,
            control_flags: snapshot.control_flags,
            max_life: snapshot.max_life,
            life: snapshot.life,
            occupants: read_i32(bytes, 20),
            rng: snapshot.entity_rng,
            position: snapshot.position,
            collision_vertical_offset: read_i16(bytes, 86),
            collision_half_width: read_i16(bytes, 88),
            collision_half_depth: read_i16(bytes, 90),
            collision_half_height: read_i16(bytes, 92),
            action: snapshot.action,
            phase: snapshot.phase,
            xtype: bytes[69],
            build_tail: bytes[59],
            building_index,
            capacity: read_i16(bytes, 136),
            owner_entity_index: read_u16(bytes, 152),
            pending_damage: read_i32(bytes, 98),
            pending_attacker: read_i16(bytes, 102),
            pending_possession_payload: read_u32(bytes, 104),
            pending_possession_owner: read_u16(bytes, 108),
            parameter,
        })
    }

    /// Dispatch one of the three BUILD lifecycle entries owned by the class-10
    /// table. Returns `true` when the action is a BUILD entry and was handled.
    pub fn dispatch_build_entry<W: FlightWorld>(&mut self, world: &W) -> bool {
        match self.action {
            HOUSE_BUILD_ACTION => step_build_44(self),
            HOUSE_TERRAIN_REMOVE_ACTION | HOUSE_TERRAIN_TO_LIVE_ACTION => {
                step_build_48_or_51(self, world);
            }
            _ => return false,
        }
        true
    }

    /// Dispatch one complete source action for a standalone live house.
    ///
    /// Imported reset houses are owned by [`HouseFamily`], while authored
    /// group houses are allocated later by the shared mission arena. Both use
    /// this exact action body so BUILD-to-live timing, damage, resident
    /// requests, destruction, phase wrapping, and disabled state cannot
    /// diverge between the two ownership paths.
    #[must_use]
    pub fn step_source_action<W: FlightWorld>(&mut self, world: &W) -> Vec<HouseStepEvent> {
        self.step_source_action_with_action_51_block(world, false)
    }

    #[must_use]
    pub fn step_source_action_with_action_51_block<W: FlightWorld>(
        &mut self,
        world: &W,
        action_51_blocked: bool,
    ) -> Vec<HouseStepEvent> {
        let mut events = Vec::new();
        let dispatched_action = self.action;
        let life_before_dispatch = self.life;
        let terrain_action_blocked = action_51_blocked
            && matches!(
                dispatched_action,
                HOUSE_TERRAIN_REMOVE_ACTION | HOUSE_TERRAIN_TO_LIVE_ACTION
            );
        if terrain_action_blocked {
            // `IsNextEvent0A_2A_37740` returns before life, ownership, or
            // terrain work; the outer dispatcher still advances phase.
        } else if self.dispatch_build_entry(world) {
            if dispatched_action == HOUSE_TERRAIN_TO_LIVE_ACTION {
                let remaining = life_before_dispatch.wrapping_sub(1);
                if remaining > 0 {
                    events.push(HouseStepEvent::ConstructionPass {
                        entity_index: self.entity_index,
                        source_id: self.source_id,
                        position: self.position,
                        building_index: self.building_index,
                        remaining,
                        first_pass: life_before_dispatch == self.max_life,
                    });
                } else {
                    events.push(HouseStepEvent::ConstructionCompleted {
                        entity_index: self.entity_index,
                        source_id: self.source_id,
                        position: self.position,
                        building_index: self.building_index,
                        collision_half_width: self.collision_half_width,
                        collision_half_depth: self.collision_half_depth,
                    });
                }
            }
        } else {
            match self.action {
                HOUSE_LIVE_ACTION => {
                    step_live_house(self, &mut events);
                    self.position.z = world.ground_height(self.position);
                }
                HOUSE_REMOVE_ACTION => {
                    events.push(HouseStepEvent::RemoveFootprint {
                        entity_index: self.entity_index,
                        attacker_entity_index: self.pending_attacker.cast_unsigned(),
                        position: self.position,
                        building_index: self.building_index,
                        occupants: self.occupants,
                        rng: self.rng,
                        xtype: self.xtype,
                    });
                    self.active = false;
                    self.control_flags |= SOURCE_DISABLED_FLAG;
                }
                _ => {}
            }
        }
        self.phase = self.phase.wrapping_add(1);
        events
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HouseStepEvent {
    ConstructionPass {
        entity_index: u16,
        source_id: u16,
        position: Position,
        building_index: u8,
        remaining: i32,
        first_pass: bool,
    },
    ConstructionCompleted {
        entity_index: u16,
        source_id: u16,
        position: Position,
        building_index: u8,
        collision_half_width: i16,
        collision_half_depth: i16,
    },
    Damaged {
        entity_index: u16,
        attacker_entity_index: u16,
        life: i32,
    },
    ResidentRequested {
        entity_index: u16,
        attacker_entity_index: u16,
        position: Position,
        model: u8,
        action: u8,
    },
    Possessed {
        entity_index: u16,
        owner_entity_index: u16,
        payload: u32,
    },
    DestructionStarted {
        entity_index: u16,
        attacker_entity_index: u16,
    },
    RemoveFootprint {
        entity_index: u16,
        attacker_entity_index: u16,
        position: Position,
        building_index: u8,
        occupants: i32,
        rng: u16,
        xtype: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseFamily {
    houses: Box<[HouseState]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HouseResidentEntry {
    Entered,
    Full,
    NotLive,
}

impl HouseFamily {
    /// Decode all forty-seven initial Level 1 houses.
    ///
    /// # Errors
    ///
    /// Returns the first missing or malformed house-row error.
    pub fn from_entities(entities: &[EntitySnapshot]) -> Result<Self, String> {
        let mut houses = Vec::with_capacity(47);
        for expected_index in LEVEL_ONE_HOUSE_INDICES {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Level 1 house entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "house pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            houses.push(HouseState::from_snapshot(snapshot)?);
        }
        Ok(Self {
            houses: houses.into_boxed_slice(),
        })
    }

    /// Decode the twenty-two ordinary Level 2 Day houses in source-slot order.
    pub fn from_level_two_entities(entities: &[EntitySnapshot]) -> Result<Self, String> {
        let mut houses = Vec::with_capacity(22);
        for expected_index in 1..=22 {
            let snapshot = entities
                .get(usize::from(expected_index))
                .ok_or_else(|| format!("missing Level 2 house entity {expected_index}"))?;
            if snapshot.index != expected_index {
                return Err(format!(
                    "Level 2 house pool order mismatch: expected {expected_index}, got {}",
                    snapshot.index
                ));
            }
            houses.push(HouseState::from_level_two_snapshot(snapshot)?);
        }
        Ok(Self {
            houses: houses.into_boxed_slice(),
        })
    }

    #[must_use]
    pub fn houses(&self) -> &[HouseState] {
        &self.houses
    }

    /// Install the class-10/model-45 row produced by Builder action 96.
    ///
    /// `NewEvent_4A050` may reuse a reclaimed source slot, so an inactive
    /// projection at the same index is replaced. A live duplicate is an
    /// authority error. The family remains source-slot ordered even after
    /// reclamation and reuse.
    pub fn insert_builder_house(&mut self, house: HouseState) -> Result<(), String> {
        if let Some(index) = self
            .houses
            .iter()
            .position(|existing| existing.entity_index == house.entity_index)
        {
            if self.houses[index].active {
                return Err(format!(
                    "Builder house source slot {} is already live",
                    house.entity_index
                ));
            }
            self.houses[index] = house;
            return Ok(());
        }

        let mut houses = self.houses.to_vec();
        let insertion = houses
            .binary_search_by_key(&house.entity_index, |existing| existing.entity_index)
            .unwrap_or_else(|index| index);
        houses.insert(insertion, house);
        self.houses = houses.into_boxed_slice();
        Ok(())
    }

    /// Apply the `sub_49A30` BUILD-row initialization to a constructor row
    /// that was already inserted before the Builder sound.
    pub fn initialize_builder_house(&mut self, house: HouseState) -> Result<(), String> {
        let existing = self
            .houses
            .iter_mut()
            .find(|existing| existing.entity_index == house.entity_index && existing.active)
            .ok_or_else(|| {
                format!(
                    "Builder house source slot {} is not live for initialization",
                    house.entity_index
                )
            })?;
        *existing = house;
        Ok(())
    }

    /// Apply the final action-51 write after BUILD-row initialization.
    pub fn set_builder_house_action(
        &mut self,
        source_entity_index: u16,
        action: u8,
    ) -> Result<(), String> {
        let house = self
            .houses
            .iter_mut()
            .find(|house| house.entity_index == source_entity_index && house.active)
            .ok_or_else(|| {
                format!("Builder house source slot {source_entity_index} is not live for action")
            })?;
        house.action = action;
        Ok(())
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
        let house = self
            .houses
            .iter_mut()
            .find(|house| house.entity_index == entity_index && house.active)
            .ok_or_else(|| format!("house entity {entity_index} is not live"))?;
        if house.pending_attacker != 0 {
            house.pending_damage = house.pending_damage.wrapping_add(damage);
        } else {
            house.pending_damage = damage;
        }
        house.pending_attacker = attacker;
        Ok(())
    }

    pub fn submit_possession(
        &mut self,
        entity_index: u16,
        owner_entity_index: u16,
        payload: u32,
    ) -> Result<(), String> {
        if owner_entity_index == 0 {
            return Err("possession owner must be nonzero".to_owned());
        }
        let house = self
            .houses
            .iter_mut()
            .find(|house| house.entity_index == entity_index && house.active)
            .ok_or_else(|| format!("house entity {entity_index} is not live"))?;
        if house.parameter.possession_blocked() {
            return Err(format!(
                "house entity {entity_index} rejects possession by BLDGPRM"
            ));
        }
        house.pending_possession_owner = owner_entity_index;
        house.pending_possession_payload = payload;
        Ok(())
    }

    /// Atomically admit one resident into the authoritative live house.
    ///
    /// A dead/unknown house and a full house are ordinary source-level
    /// rejections. Neither branch mutates the family. Success performs the
    /// one and only occupant increment.
    #[must_use]
    pub fn try_enter_resident(&mut self, entity_index: u16) -> HouseResidentEntry {
        let Some(house) = self
            .houses
            .iter_mut()
            .find(|house| house.entity_index == entity_index && house.active)
        else {
            return HouseResidentEntry::NotLive;
        };
        if house.occupants >= i32::from(house.capacity) {
            return HouseResidentEntry::Full;
        }
        house.occupants = house.occupants.wrapping_add(1);
        HouseResidentEntry::Entered
    }

    pub fn commit_removal_state(
        &mut self,
        entity_index: u16,
        final_rng: u16,
        final_occupants: i32,
    ) -> Result<(), String> {
        let house = self
            .houses
            .iter_mut()
            .find(|house| house.entity_index == entity_index)
            .ok_or_else(|| format!("house entity {entity_index} is unknown"))?;
        if house.action != HOUSE_REMOVE_ACTION || house.active {
            return Err(format!(
                "house entity {entity_index} is not awaiting committed removal"
            ));
        }
        house.rng = final_rng;
        house.occupants = final_occupants;
        Ok(())
    }

    #[must_use]
    pub fn step_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
    ) -> Vec<HouseStepEvent> {
        self.try_step_source_slot(source_slot, world, |_| true)
    }

    #[must_use]
    pub fn step_source_slot_with_action_51_block<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        action_51_blocked: bool,
    ) -> Vec<HouseStepEvent> {
        let Some(index) = self
            .houses
            .iter()
            .position(|house| house.entity_index == source_slot && house.active)
        else {
            return Vec::new();
        };
        let mut house = self.houses[index];
        let events = house.step_source_action_with_action_51_block(world, action_51_blocked);
        self.houses[index] = house;
        events
    }

    /// Stage one source dispatch and commit it only when all cross-system
    /// allocations can be accepted.
    ///
    /// Action 52 can request one resident and action 53 can fan out into many
    /// row-major requests in the terrain bridge. Keeping the house mutation
    /// staged lets that bridge reject allocation atomically: no phase, RNG,
    /// ownership, mailbox, footprint, or event state is committed.
    #[must_use]
    pub fn try_step_source_slot<W: FlightWorld>(
        &mut self,
        source_slot: u16,
        world: &W,
        mut accept: impl FnMut(&[HouseStepEvent]) -> bool,
    ) -> Vec<HouseStepEvent> {
        let Some(index) = self
            .houses
            .iter()
            .position(|house| house.entity_index == source_slot && house.active)
        else {
            return Vec::new();
        };
        let mut house = self.houses[index];
        let events = house.step_source_action(world);
        if !accept(&events) {
            return Vec::new();
        }
        self.houses[index] = house;
        events
    }
}

/// `AddTerrainMod0A_2A_37BC0`: action 44 has its own 19-dispatch build
/// countdown and negative reclamation tail. It is not an alias of 48/51.
fn step_build_44(house: &mut HouseState) {
    const BUILD_INITIALIZED_FLAG: u32 = 2;
    if house.control_flags & BUILD_INITIALIZED_FLAG == 0 {
        house.occupants = 19;
        house.control_flags |= BUILD_INITIALIZED_FLAG;
    }
    if house.occupants <= 0 {
        house.occupants = house.occupants.wrapping_add(1);
        if house.occupants == 0 {
            house.active = false;
            house.control_flags |= SOURCE_DISABLED_FLAG;
        }
        return;
    }
    house.occupants -= 1;
    if house.occupants == 0 {
        house.occupants = if house.build_tail != 0 { -25 } else { -1 };
    }
}

/// `ApplyTerrainModification_37240`: actions 48 and 51 share the terrain
/// handler, then deliberately split at completion. Action 48 signals/removes
/// the temporary BUILD row. Action 51 turns that same row into a live house.
fn step_build_48_or_51<W: FlightWorld>(house: &mut HouseState, world: &W) {
    house.life = house.life.wrapping_sub(1);
    if house.life > 0 {
        return;
    }
    if house.action == HOUSE_TERRAIN_REMOVE_ACTION {
        house.active = false;
        house.control_flags |= SOURCE_DISABLED_FLAG;
        return;
    }

    house.life = i32::from(house.parameter.life_mana_scalar).wrapping_mul(1_000);
    house.action = HOUSE_LIVE_ACTION;
    if house.owner_entity_index == 0 {
        house.control_flags |= 1;
    } else {
        house.control_flags &= !1;
    }
    house.position.z = world.ground_height(house.position);
}

fn step_live_house(house: &mut HouseState, events: &mut Vec<HouseStepEvent>) {
    if house.life < 0 {
        house.action = HOUSE_REMOVE_ACTION;
        events.push(HouseStepEvent::DestructionStarted {
            entity_index: house.entity_index,
            attacker_entity_index: house.pending_attacker.cast_unsigned(),
        });
        return;
    }
    if house.pending_attacker != 0 {
        let attacker_entity_index = house.pending_attacker.cast_unsigned();
        house.life = house.life.wrapping_sub(house.pending_damage);
        if house.life < 0 {
            house.action = HOUSE_REMOVE_ACTION;
            events.push(HouseStepEvent::DestructionStarted {
                entity_index: house.entity_index,
                attacker_entity_index,
            });
            return;
        }
        house.pending_damage = 0;
        house.pending_attacker = 0;
        events.push(HouseStepEvent::Damaged {
            entity_index: house.entity_index,
            attacker_entity_index,
            life: house.life,
        });
        if house.occupants > 2 {
            house.occupants -= 1;
            if house.parameter.produces_villagers() {
                events.push(HouseStepEvent::ResidentRequested {
                    entity_index: house.entity_index,
                    attacker_entity_index,
                    position: Position {
                        x: house
                            .position
                            .x
                            .wrapping_add(house.collision_half_width.cast_unsigned()),
                        ..house.position
                    },
                    model: 4,
                    action: 33,
                });
            }
        }
    }

    if house.pending_possession_owner != 0 {
        let owner_entity_index = house.pending_possession_owner;
        let payload = house.pending_possession_payload;
        if owner_entity_index != house.owner_entity_index
            && (payload != 0 || house.control_flags & POSSESSED_FLAG == 0)
        {
            house.owner_entity_index = owner_entity_index;
            if payload != 0 {
                house.control_flags = (house.control_flags & 0xffdf_fffe) | POSSESSED_FLAG;
            } else {
                house.control_flags &= !1;
            }
            events.push(HouseStepEvent::Possessed {
                entity_index: house.entity_index,
                owner_entity_index,
                payload,
            });
        }
        house.pending_possession_owner = 0;
        house.pending_possession_payload = 0;
    }

    if house.phase & 0x1f == 0
        && house.parameter.produces_villagers()
        && house.capacity > 5
        && i32::from(house.capacity) == house.occupants
    {
        house.rng = next_rng(house.rng);
        let threshold = i32::from(house.capacity) - (i32::from(house.capacity) >> 4) - 2;
        if i32::from(house.rng % house.capacity.cast_unsigned()) > threshold {
            house.rng = next_rng(house.rng);
            let (model, action) = match house.rng % 12 {
                0 | 1 => (4, 33),
                2 | 3 => (14, 113),
                4..=8 => (13, 105),
                9..=11 => (12, 97),
                _ => unreachable!("modulo twelve"),
            };
            events.push(HouseStepEvent::ResidentRequested {
                entity_index: house.entity_index,
                attacker_entity_index: 0,
                position: Position {
                    x: house
                        .position
                        .x
                        .wrapping_add(house.collision_half_width.cast_unsigned()),
                    ..house.position
                },
                model,
                action,
            });
        }
    }
}

const fn next_rng(seed: u16) -> u16 {
    seed.wrapping_mul(9_377).wrapping_add(9_439)
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
    use crate::{
        level1_mission::CastleBuildFootprint,
        native_world::{NativeWorld, NativeWorldInspection, SessionTerrain},
    };

    fn load_world() -> NativeWorld {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        NativeWorld::load(
            root.join("level1-export-n"),
            root.join("level1-asset-closure.json"),
        )
        .expect("qualified owner package")
    }

    fn load_level_two_world() -> NativeWorldInspection {
        let package = std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        );
        NativeWorld::inspect_level2(package).expect("qualified Level 2 owner package")
    }

    #[test]
    fn exact_level_one_reset_decodes_all_forty_seven_houses() {
        let world = load_world();
        let family = HouseFamily::from_entities(world.entities()).expect("house family");
        assert_eq!(family.houses().len(), 47);
        assert_eq!(
            (
                family.houses()[0].entity_index,
                family.houses()[0].building_index,
                family.houses()[0].life,
                family.houses()[0].occupants,
                family.houses()[0].capacity,
            ),
            (1, 30, 25_000, 2, 48)
        );
        assert_eq!(
            (
                family.houses()[1].building_index,
                family.houses()[1].parameter,
                family.houses()[1].capacity,
            ),
            (
                17,
                HouseParameter {
                    life_mana_scalar: 500,
                    flags: 0x18,
                    destruction_successor: 0,
                },
                576,
            )
        );

        let synthetic = world.entities()[1]
            .clone()
            .with_record_byte(65, 3)
            .with_record_byte(69, 7);
        assert_eq!(
            HouseState::from_snapshot(&synthetic)
                .expect("synthetic nonzero xtype")
                .xtype,
            7,
            "xtype is serialized at source offset 65 plus the four-byte snapshot prefix"
        );
    }

    #[test]
    fn damage_releases_one_occupant_and_fatal_damage_defers_removal_one_dispatch() {
        let world = load_world();
        let mut family = HouseFamily::from_entities(world.entities()).expect("house family");
        family.houses[0].occupants = 47;
        assert_eq!(family.try_enter_resident(1), HouseResidentEntry::Entered);
        assert_eq!(family.houses()[0].occupants, 48);
        let full = family.clone();
        assert_eq!(family.try_enter_resident(1), HouseResidentEntry::Full);
        assert_eq!(family, full, "full rejection must not mutate the house");
        family.houses[0].active = false;
        let dead = family.clone();
        assert_eq!(family.try_enter_resident(1), HouseResidentEntry::NotLive);
        assert_eq!(family, dead, "dead rejection must not mutate the house");
        family = HouseFamily::from_entities(world.entities()).expect("house family");
        family.houses[0].occupants = 4;
        family.inject_damage(1, 100, 48).expect("damage");
        let events = family.step_source_slot(1, &world);
        assert_eq!(family.houses()[0].life, 24_900);
        assert_eq!(family.houses()[0].occupants, 3);
        assert!(matches!(
            events.as_slice(),
            [
                HouseStepEvent::Damaged { .. },
                HouseStepEvent::ResidentRequested {
                    model: 4,
                    action: 33,
                    ..
                }
            ]
        ));

        let lethal = family.houses()[0].life + 1;
        family.inject_damage(1, lethal, 48).expect("lethal damage");
        let events = family.step_source_slot(1, &world);
        assert_eq!(family.houses()[0].action, HOUSE_REMOVE_ACTION);
        assert!(family.houses()[0].active);
        assert!(matches!(
            events.as_slice(),
            [HouseStepEvent::DestructionStarted { .. }]
        ));

        let events = family.step_source_slot(1, &world);
        assert!(!family.houses()[0].active);
        assert!(matches!(
            events.as_slice(),
            [HouseStepEvent::RemoveFootprint { .. }]
        ));

        // The allocation gate encloses the compare-event mailbox mutations,
        // resident request, phase, and ground resample in one transaction.
        let mut rejected = HouseFamily::from_entities(world.entities()).expect("house family");
        rejected.houses[0].occupants = 4;
        rejected.inject_damage(1, 100, 48).expect("damage");
        let before = rejected.clone();
        assert!(
            rejected
                .try_step_source_slot(1, &world, |events| {
                    assert!(matches!(
                        events,
                        [
                            HouseStepEvent::Damaged { .. },
                            HouseStepEvent::ResidentRequested { .. }
                        ]
                    ));
                    false
                })
                .is_empty()
        );
        assert_eq!(rejected, before);

        // Every GetRandManaSphere branch consumes the eligibility draw first
        // and the model draw second, in source order.
        for (seed, expected) in [
            (46, (4, 33)),
            (128, (14, 113)),
            (14, (13, 105)),
            (144, (12, 97)),
        ] {
            let mut population =
                HouseFamily::from_entities(world.entities()).expect("house family");
            population.houses[0].phase = 0;
            population.houses[0].occupants = 48;
            population.houses[0].capacity = 48;
            population.houses[0].rng = seed;
            let events = population.step_source_slot(1, &world);
            assert!(matches!(
                events.as_slice(),
                [HouseStepEvent::ResidentRequested {
                    model,
                    action,
                    attacker_entity_index: 0,
                    ..
                }] if (*model, *action) == expected
            ));
            assert_eq!(
                population.houses[0].rng,
                next_rng(next_rng(seed)),
                "eligibility and resident-selection draws"
            );
        }
    }

    #[test]
    fn add_house_rng_gate_receipt_preserves_0257_0258_order_and_rejection() {
        // Pinned remc2 6806c72, EventsFunctions.cpp:28051-28052:
        // AddHouse0A_2D_38330 advances RNG-L2-0257, then applies the
        // RNG-L2-0258 eligibility comparison. Only the eligible branch may
        // enter GetRandManaSphere_38270; this test is a house-gate receipt,
        // not a Level 2 reachability claim for RNG-L2-0255..0256.
        let world = load_world();
        let mut eligible = HouseFamily::from_entities(world.entities()).expect("house family");
        eligible.houses[0].phase = 0;
        eligible.houses[0].occupants = 48;
        eligible.houses[0].capacity = 48;
        eligible.houses[0].rng = 46;
        let events = eligible.step_source_slot(1, &world);
        assert!(matches!(
            events.as_slice(),
            [HouseStepEvent::ResidentRequested {
                attacker_entity_index: 0,
                model: 4,
                action: 33,
                ..
            }]
        ));
        assert_eq!(eligible.houses[0].rng, next_rng(next_rng(46)));

        // Seed zero fails the post-0257 eligibility comparison. The rejected
        // branch consumes no GetRandManaSphere draw and emits no request.
        let mut rejected = HouseFamily::from_entities(world.entities()).expect("house family");
        rejected.houses[0].phase = 0;
        rejected.houses[0].occupants = 48;
        rejected.houses[0].capacity = 48;
        rejected.houses[0].rng = 0;
        assert!(rejected.step_source_slot(1, &world).is_empty());
        assert_eq!(rejected.houses[0].rng, next_rng(0));
    }

    #[test]
    fn qualified_level_two_house_lifecycle_emits_0255_0256_in_source_order() {
        // Pinned remc2 6806c72: the class-10 table dispatches action 52 to
        // AddHouse0A_2D_38330 and action 53 to RemoveCastleStage_385C0
        // (EventsFunctions.cpp:1601-1655). Both call sites enter
        // GetRandManaSphere_38270 only under BLDGPRM.byte_2 & 1
        // (EventsFunctions.cpp:28043-28059 and :28112-28124).
        //
        // The qualified reset has 22 live rows in slots 1..22. The entity's
        // source byte-0x46 BUILD selectors vary, while the raw entity
        // parameter relation is 60 (normalized to Some(59) by the typed
        // parser); the selected BLDGPRM rows include both producer-bit and
        // non-producer-bit houses. The ordinary action-52 and following
        // action-53 routes must therefore cover both positive draws and
        // no-forbidden-draw negatives.
        let world = load_level_two_world();
        let mut family = HouseFamily::from_level_two_entities(world.entities())
            .expect("qualified Level 2 ordinary house family");
        assert_eq!(family.houses().len(), 22);

        const EXPECTED_BUILDING_SELECTORS: [u8; 22] = [
            55, 59, 59, 59, 59, 60, 60, 60, 60, 50, 44, 26, 26, 30, 29, 50, 50, 29, 50, 26, 50, 64,
        ];
        let sphere_selection = |rng: u16| match rng % 12 {
            0 | 1 => (4, 33),
            2 | 3 => (14, 113),
            4..=8 => (13, 105),
            9..=11 => (12, 97),
            _ => unreachable!("modulo twelve"),
        };
        let house_snapshots = family.houses().to_vec();
        for (ordinal, house) in house_snapshots.into_iter().enumerate() {
            let snapshot = &world.entities()[usize::from(house.entity_index)];
            assert_eq!(
                (
                    snapshot.active,
                    snapshot.class,
                    snapshot.model,
                    snapshot.action,
                    house.entity_index,
                    house.building_index,
                    snapshot.parameter_relation,
                    snapshot.record_bytes()[164],
                    snapshot.level_gate,
                    house.parameter.flags,
                ),
                (
                    true,
                    HOUSE_CLASS,
                    HOUSE_MODEL,
                    HOUSE_LIVE_ACTION,
                    (ordinal + 1) as u16,
                    EXPECTED_BUILDING_SELECTORS[ordinal],
                    Some(59),
                    60,
                    EXPECTED_BUILDING_SELECTORS[ordinal].cast_signed(),
                    if house.parameter.produces_villagers() {
                        0x03
                    } else {
                        0x08
                    },
                )
            );

            // Exercise the real action-52 source visit at the exact periodic
            // gate with a full house and a fixed source-local seed. Producer
            // rows consume the eligibility draw first, then the
            // GetRandManaSphere draw; non-producer rows consume neither.
            let seed = if house.parameter.produces_villagers() {
                (0..=u16::MAX)
                    .find(|seed| {
                        let after_eligibility = next_rng(*seed);
                        i32::from(after_eligibility % house.capacity.cast_unsigned())
                            > i32::from(house.capacity) - (i32::from(house.capacity) >> 4) - 2
                    })
                    .expect("a qualified producer has an eligible RNG seed")
            } else {
                0x1200_u16.wrapping_add(house.entity_index)
            };
            family.houses[ordinal].phase = 0;
            family.houses[ordinal].occupants = i32::from(house.capacity);
            family.houses[ordinal].rng = seed;
            let events = family.step_source_slot(house.entity_index, world.terrain());
            if house.parameter.produces_villagers() {
                let expected = sphere_selection(next_rng(next_rng(seed)));
                assert!(matches!(
                    events.as_slice(),
                    [HouseStepEvent::ResidentRequested {
                        attacker_entity_index: 0,
                        model,
                        action,
                        ..
                    }]
                    if (*model, *action) == expected
                ));
                assert_eq!(
                    family.houses()[ordinal].rng,
                    next_rng(next_rng(seed)),
                    "0257 then 0255/0256 source order"
                );
            } else {
                assert!(events.is_empty(), "non-producing house emitted an event");
                assert_eq!(family.houses()[ordinal].rng, seed);
            }
            assert_eq!(family.houses()[ordinal].phase, 1);
        }

        // A producer row with a rejected 0258 comparison consumes only 0257.
        let rejected_ordinal = 9;
        let rejected_house = family.houses()[rejected_ordinal];
        let rejected_seed = (0..=u16::MAX)
            .find(|seed| {
                let after_eligibility = next_rng(*seed);
                i32::from(after_eligibility % rejected_house.capacity.cast_unsigned())
                    <= i32::from(rejected_house.capacity)
                        - (i32::from(rejected_house.capacity) >> 4)
                        - 2
            })
            .expect("a qualified producer has a rejected RNG seed");
        family.houses[rejected_ordinal].phase = 0;
        family.houses[rejected_ordinal].occupants = i32::from(rejected_house.capacity);
        family.houses[rejected_ordinal].rng = rejected_seed;
        assert!(
            family
                .step_source_slot(rejected_house.entity_index, world.terrain())
                .is_empty()
        );
        assert_eq!(
            family.houses()[rejected_ordinal].rng,
            next_rng(rejected_seed),
            "rejected 0258 branch consumes no 0255/0256 draw"
        );

        // Follow a qualified producer through the ordinary lethal action-52
        // transition and action-53 event. The Level 2 terrain consumer uses
        // the event fields below to execute RemoveCastleStage's row-major
        // removal loop. Two non-empty, height-sentinel cells force exactly
        // the two source GetRandManaSphere calls while avoiding unrelated
        // height-reduction draws. The qualified owner-footprint route is
        // covered separately by the Level 2 ordinary-session tests; this
        // narrow production receipt isolates the resident RNG branch.
        let mut removal = HouseFamily::from_level_two_entities(world.entities())
            .expect("qualified Level 2 removal family");
        let source_slot = 10;
        let removal_house = removal.houses()[usize::from(source_slot - 1)];
        assert!(removal_house.parameter.produces_villagers());
        let seed = 0x4a31;
        removal.houses[usize::from(source_slot - 1)].occupants = 6;
        removal.houses[usize::from(source_slot - 1)].rng = seed;
        removal
            .inject_damage(source_slot, removal_house.life + 1, 111)
            .expect("qualified Level 2 lethal house damage");
        let destruction = removal.step_source_slot(source_slot, world.terrain());
        assert!(matches!(
            destruction.as_slice(),
            [HouseStepEvent::DestructionStarted {
                entity_index: 10,
                ..
            }]
        ));
        assert_eq!(
            removal.houses()[usize::from(source_slot - 1)].action,
            HOUSE_REMOVE_ACTION
        );
        let footprint_removal = removal.step_source_slot(source_slot, world.terrain());
        let (position, building_index, occupants, event_rng) = match footprint_removal.as_slice() {
            [
                HouseStepEvent::RemoveFootprint {
                    position,
                    building_index,
                    occupants,
                    rng,
                    entity_index: 10,
                    ..
                },
            ] => (*position, *building_index, *occupants, *rng),
            other => {
                assert!(false, "unexpected action-53 receipt: {other:?}");
                return;
            }
        };
        assert_eq!((occupants, event_rng), (6, seed));
        assert!(!removal.houses()[usize::from(source_slot - 1)].active);

        let footprint =
            CastleBuildFootprint::from_owner_cells(building_index, 2, 1, &[0, 0xff, 0, 0xff])
                .expect("source-shaped action-53 receipt footprint");
        let mut terrain = SessionTerrain::from_inspection(&world);
        let outcome = terrain
            .apply_house_removal(position, event_rng, occupants, true, &footprint)
            .expect("qualified producer action-53 terrain consumer");
        let first_draw = next_rng(seed);
        let second_draw = next_rng(first_draw);
        assert_eq!(outcome.final_rng, second_draw, "0255/0256 action-53 order");
        assert_eq!(outcome.final_occupants, 4);
        assert_eq!(outcome.residents.len(), 2);
        assert_eq!(
            outcome
                .residents
                .iter()
                .map(|resident| (resident.model, resident.action))
                .collect::<Vec<_>>(),
            vec![sphere_selection(first_draw), sphere_selection(second_draw)]
        );

        // The same action-53 consumer is a no-op for the forbidden RNG draw
        // when the qualified BLDGPRM row lacks bit 0, even with six occupants.
        let mut nonproducing = HouseFamily::from_level_two_entities(world.entities())
            .expect("qualified Level 2 non-producing removal family");
        let nonproducing_house = nonproducing.houses()[0];
        let produces_residents = nonproducing_house.parameter.produces_villagers();
        assert!(!produces_residents);
        nonproducing.houses[0].occupants = 6;
        nonproducing.houses[0].rng = seed;
        nonproducing
            .inject_damage(1, nonproducing_house.life + 1, 111)
            .expect("qualified Level 2 non-producing lethal damage");
        let _ = nonproducing.step_source_slot(1, world.terrain());
        let nonproducing_event = nonproducing.step_source_slot(1, world.terrain());
        let (position, building_index, occupants, event_rng) = match nonproducing_event.as_slice() {
            [
                HouseStepEvent::RemoveFootprint {
                    position,
                    building_index,
                    occupants,
                    rng,
                    entity_index: 1,
                    ..
                },
            ] => (*position, *building_index, *occupants, *rng),
            other => {
                assert!(
                    false,
                    "unexpected non-producing action-53 receipt: {other:?}"
                );
                return;
            }
        };
        let footprint =
            CastleBuildFootprint::from_owner_cells(building_index, 2, 1, &[0, 0xff, 0, 0xff])
                .expect("non-producing action-53 receipt footprint");
        let outcome = SessionTerrain::from_inspection(&world)
            .apply_house_removal(
                position,
                event_rng,
                occupants,
                produces_residents,
                &footprint,
            )
            .expect("qualified non-producing action-53 terrain consumer");
        assert_eq!(outcome.final_rng, seed);
        assert_eq!(outcome.final_occupants, 4);
        assert!(outcome.residents.is_empty());
    }

    #[test]
    fn action_51_emits_post_decrement_passes_then_completion_after_state_transition() {
        let world = load_world();
        let mut family = HouseFamily::from_entities(world.entities()).expect("house family");
        let mut house = family.houses()[0];
        house.action = HOUSE_TERRAIN_TO_LIVE_ACTION;
        house.life = 2;
        house.max_life = 2;
        house.owner_entity_index = 152;
        house.control_flags |= 1;
        family.initialize_builder_house(house).unwrap();

        let blocked_phase = family.houses()[0].phase;
        assert!(
            family
                .step_source_slot_with_action_51_block(house.entity_index, &world, true)
                .is_empty()
        );
        assert_eq!(family.houses()[0].life, 2);
        assert_eq!(family.houses()[0].action, HOUSE_TERRAIN_TO_LIVE_ACTION);
        assert_eq!(family.houses()[0].phase, blocked_phase.wrapping_add(1));

        let first = family.step_source_slot(house.entity_index, &world);
        assert!(matches!(
            first.as_slice(),
            [HouseStepEvent::ConstructionPass { remaining: 1, .. }]
        ));
        assert_eq!(family.houses()[0].action, HOUSE_TERRAIN_TO_LIVE_ACTION);
        assert_eq!(family.houses()[0].life, 1);

        let final_events = family.step_source_slot(house.entity_index, &world);
        assert!(matches!(
            final_events.as_slice(),
            [HouseStepEvent::ConstructionCompleted { .. }]
        ));
        let live = family.houses()[0];
        assert_eq!(live.action, HOUSE_LIVE_ACTION);
        assert_eq!(
            live.life,
            i32::from(live.parameter.life_mana_scalar) * 1_000
        );
        assert_eq!(live.owner_entity_index, 152);
        assert_eq!(live.control_flags & 1, 0);
    }

    #[test]
    fn possession_is_consumed_only_by_eligible_house_source_dispatch() {
        let world = load_world();
        let mut family = HouseFamily::from_entities(world.entities()).expect("house family");
        family.submit_possession(1, 48, 0).expect("possession");
        assert!(family.step_source_slot(8, &world).is_empty());
        let events = family.step_source_slot(1, &world);
        assert_eq!(family.houses()[0].owner_entity_index, 48);
        assert!(matches!(
            events.as_slice(),
            [HouseStepEvent::Possessed {
                entity_index: 1,
                owner_entity_index: 48,
                payload: 0
            }]
        ));
        assert!(family.submit_possession(2, 48, 0).is_err());

        // Neutral payload cannot steal an already-possessed house, but the
        // mailbox is still consumed. A nonzero payload accepts the new owner
        // and installs the possessed/neutral control-bit combination.
        family.houses[0].control_flags |= POSSESSED_FLAG;
        family
            .submit_possession(1, 49, 0)
            .expect("neutral possession mailbox");
        assert!(family.step_source_slot(1, &world).is_empty());
        assert_eq!(family.houses[0].owner_entity_index, 48);
        assert_eq!(family.houses[0].pending_possession_owner, 0);
        family
            .submit_possession(1, 49, 7)
            .expect("strong possession mailbox");
        assert!(matches!(
            family.step_source_slot(1, &world).as_slice(),
            [HouseStepEvent::Possessed {
                owner_entity_index: 49,
                payload: 7,
                ..
            }]
        ));
        assert_eq!(
            family.houses[0].control_flags & (POSSESSED_FLAG | 1),
            POSSESSED_FLAG
        );

        // BUILD entries retain the exact dispatch-table shape: 44 owns its
        // 19/-1 staged tail, 48 disables at completion, and 51 becomes 52.
        let mut build_44 = HouseFamily::from_entities(world.entities()).expect("house family");
        build_44.houses[0].action = HOUSE_BUILD_ACTION;
        build_44.houses[0].control_flags &= !2;
        build_44.houses[0].build_tail = 0;
        for _ in 0..19 {
            assert!(build_44.step_source_slot(1, &world).is_empty());
        }
        assert_eq!(build_44.houses[0].occupants, -1);
        let _ = build_44.step_source_slot(1, &world);
        assert!(!build_44.houses[0].active);

        let mut build_48 = HouseFamily::from_entities(world.entities()).expect("house family");
        build_48.houses[0].action = HOUSE_TERRAIN_REMOVE_ACTION;
        build_48.houses[0].life = 1;
        let _ = build_48.step_source_slot(1, &world);
        assert!(!build_48.houses[0].active);

        let mut build_51 = HouseFamily::from_entities(world.entities()).expect("house family");
        build_51.houses[0].action = HOUSE_TERRAIN_TO_LIVE_ACTION;
        build_51.houses[0].life = 1;
        build_51.houses[0].owner_entity_index = 0;
        build_51.houses[0].control_flags &= !1;
        build_51.houses[0].position.z = 0;
        let _ = build_51.step_source_slot(1, &world);
        assert_eq!(build_51.houses[0].action, HOUSE_LIVE_ACTION);
        assert_eq!(
            build_51.houses[0].life,
            i32::from(build_51.houses[0].parameter.life_mana_scalar) * 1_000
        );
        assert_eq!(build_51.houses[0].control_flags & 1, 1);
        assert_eq!(
            build_51.houses[0].position.z,
            world.ground_height(build_51.houses[0].position)
        );
    }
}
