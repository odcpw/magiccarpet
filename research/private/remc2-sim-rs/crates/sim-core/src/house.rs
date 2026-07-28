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
        let (life_mana_scalar, flags) = match building_index {
            17 => (500, 0x18),
            26 => (20, 0x03),
            29 => (28, 0x03),
            30 => (25, 0x03),
            31 => (29, 0x03),
            34 => (118, 0x03),
            42 => (52, 0x03),
            47 => (34, 0x03),
            50 => (24, 0x03),
            51..=53 => (30, 0x08),
            _ => return None,
        };
        Some(Self {
            life_mana_scalar,
            flags,
            destruction_successor: 0,
        })
    }

    #[must_use]
    pub const fn produces_villagers(self) -> bool {
        self.flags & 1 != 0
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HouseStepEvent {
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
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseFamily {
    houses: Box<[HouseState]>,
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

    #[must_use]
    pub fn houses(&self) -> &[HouseState] {
        &self.houses
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

    pub fn add_occupant(&mut self, entity_index: u16) -> Result<(), String> {
        let house = self
            .houses
            .iter_mut()
            .find(|house| house.entity_index == entity_index && house.active)
            .ok_or_else(|| format!("house entity {entity_index} is not live"))?;
        house.occupants = house.occupants.wrapping_add(1);
        Ok(())
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
        let Some(index) = self
            .houses
            .iter()
            .position(|house| house.entity_index == source_slot && house.active)
        else {
            return Vec::new();
        };
        let house = &mut self.houses[index];
        let mut events = Vec::new();
        match house.action {
            HOUSE_LIVE_ACTION => {
                step_live_house(house, &mut events);
                house.position.z = world.ground_height(house.position);
            }
            HOUSE_REMOVE_ACTION => {
                events.push(HouseStepEvent::RemoveFootprint {
                    entity_index: house.entity_index,
                    attacker_entity_index: house.pending_attacker.cast_unsigned(),
                    position: house.position,
                    building_index: house.building_index,
                    occupants: house.occupants,
                    rng: house.rng,
                });
                house.active = false;
                house.control_flags |= SOURCE_DISABLED_FLAG;
            }
            _ => {}
        }
        house.phase = house.phase.wrapping_add(1);
        events
    }
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
    }

    #[test]
    fn damage_releases_one_occupant_and_fatal_damage_defers_removal_one_dispatch() {
        let world = load_world();
        let mut family = HouseFamily::from_entities(world.entities()).expect("house family");
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
    }
}
