//! Exact bounded dynamic-light service used by the Level 1 Night map.
//!
//! The source owns fifty persistent records. Retiring a record clears only
//! its active byte; the remaining payload and the previous 25-cell footprint
//! stay observable until a later admission reuses that exact slot.

use crate::Position;

pub const DYNAMIC_LIGHT_CAPACITY: usize = 50;
pub const DYNAMIC_LIGHT_HISTORY_CELLS: usize = 25;
pub const DYNAMIC_LIGHT_RESET_RNG: u32 = 0x0003_a013;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicLightSlot {
    pub active: bool,
    pub life_scaled: bool,
    pub intensity: u8,
    pub jitter_range: u8,
    pub position: Position,
    pub source_entity_index: u16,
    pub linked: bool,
    pub history: [u8; DYNAMIC_LIGHT_HISTORY_CELLS],
}

impl Default for DynamicLightSlot {
    fn default() -> Self {
        Self {
            active: false,
            life_scaled: false,
            intensity: 0,
            jitter_range: 0,
            position: Position::default(),
            source_entity_index: 0,
            linked: false,
            history: [0; DYNAMIC_LIGHT_HISTORY_CELLS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicLightLinkedEntity {
    pub position: Position,
    pub life: i32,
    pub disabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicLightingState {
    rng: u32,
    active_count: u8,
    slots: [DynamicLightSlot; DYNAMIC_LIGHT_CAPACITY],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicLightingExecutionReceipt {
    pub state_hash: u64,
    pub shading_hash: u64,
    pub active_slot_mask: u64,
    pub shading_bytes: u32,
    pub active_count: u8,
}

impl DynamicLightingExecutionReceipt {
    #[must_use]
    pub fn is_complete_for(self, expected_state_hash: u64) -> bool {
        self.state_hash == expected_state_hash
            && self.shading_bytes == 65_536
            && self.active_slot_mask.count_ones() == u32::from(self.active_count)
            && usize::from(self.active_count) <= DYNAMIC_LIGHT_CAPACITY
    }
}

impl Default for DynamicLightingState {
    fn default() -> Self {
        Self::reset()
    }
}

impl DynamicLightingState {
    #[must_use]
    pub const fn reset() -> Self {
        Self {
            rng: DYNAMIC_LIGHT_RESET_RNG,
            active_count: 0,
            slots: [DynamicLightSlot {
                active: false,
                life_scaled: false,
                intensity: 0,
                jitter_range: 0,
                position: Position { x: 0, y: 0, z: 0 },
                source_entity_index: 0,
                linked: false,
                history: [0; DYNAMIC_LIGHT_HISTORY_CELLS],
            }; DYNAMIC_LIGHT_CAPACITY],
        }
    }

    #[must_use]
    pub const fn rng(&self) -> u32 {
        self.rng
    }

    #[must_use]
    pub const fn active_count(&self) -> u8 {
        self.active_count
    }

    #[must_use]
    pub const fn slots(&self) -> &[DynamicLightSlot; DYNAMIC_LIGHT_CAPACITY] {
        &self.slots
    }

    #[must_use]
    pub fn active_slots(&self) -> impl Iterator<Item = (usize, &DynamicLightSlot)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.active)
    }

    /// Source `AddEvent2_847D0`: admission alone is option-gated. Existing
    /// records continue to be serviced while Night/Cave remains active.
    pub fn admit(
        &mut self,
        option_enabled: bool,
        night_or_cave: bool,
        source_entity_index: u16,
        position: Position,
        intensity: u8,
        jitter_range: u8,
        life_scaled: bool,
    ) -> Option<u8> {
        if !option_enabled
            || !night_or_cave
            || usize::from(self.active_count) >= DYNAMIC_LIGHT_CAPACITY
        {
            return None;
        }
        let slot_index = self.slots.iter().position(|slot| !slot.active)?;
        self.slots[slot_index] = DynamicLightSlot {
            active: true,
            life_scaled,
            intensity,
            jitter_range,
            position,
            source_entity_index,
            linked: true,
            history: [0; DYNAMIC_LIGHT_HISTORY_CELLS],
        };
        self.active_count = self.active_count.wrapping_add(1);
        Some(u8::try_from(slot_index).expect("fifty light slots fit u8"))
    }

    /// Source `sub_84880`: only the active byte and count are changed.
    pub fn retire(&mut self, slot_index: usize) {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return;
        };
        if slot.active {
            slot.active = false;
            self.active_count = self.active_count.saturating_sub(1);
        }
    }

    /// Source `sub_848A0`: remove every prior contribution before any entity
    /// update changes a linked position or retires a linked row.
    pub fn subtract_previous(&self, shading: &mut [u8], night_or_cave: bool) {
        if !night_or_cave {
            return;
        }
        assert_eq!(shading.len(), 65_536, "dynamic-light shading plane");
        for (_, slot) in self.active_slots() {
            for (history_index, map_index) in light_map_walk(slot.position).enumerate() {
                shading[map_index] = shading[map_index].wrapping_sub(slot.history[history_index]);
            }
        }
    }

    /// Source `sub_84B80`: after entity updates, retire dead/disabled links
    /// or recompute a fresh literal 25-cell footprint.
    pub fn service_recompute(
        &mut self,
        shading: &mut [u8],
        night_or_cave: bool,
        mut linked_entity: impl FnMut(u16) -> Option<DynamicLightLinkedEntity>,
        mut ground_height: impl FnMut(u16, u16) -> i16,
    ) {
        if !night_or_cave {
            return;
        }
        assert_eq!(shading.len(), 65_536, "dynamic-light shading plane");
        for slot_index in 0..DYNAMIC_LIGHT_CAPACITY {
            if !self.slots[slot_index].active {
                continue;
            }
            let linked = if self.slots[slot_index].linked {
                linked_entity(self.slots[slot_index].source_entity_index)
            } else {
                None
            };
            if self.slots[slot_index].linked
                && linked.is_none_or(|entity| entity.life < 0 || entity.disabled)
            {
                self.retire(slot_index);
                continue;
            }
            if let Some(entity) = linked {
                self.slots[slot_index].position = entity.position;
            }
            self.rng = self.rng.wrapping_mul(9_377).wrapping_add(9_439);
            let slot = &mut self.slots[slot_index];
            let jitter = if slot.jitter_range == 0 {
                0
            } else {
                i32::try_from(self.rng % u32::from(slot.jitter_range)).expect("u8 modulus fits i32")
                    - i32::from(slot.jitter_range / 2)
            };
            let life_cap = if slot.life_scaled {
                linked.map_or(0, |entity| entity.life.clamp(0, 31))
            } else {
                31
            };
            for (history_index, map_index) in light_map_walk(slot.position).enumerate() {
                let tile_x = u16::try_from(map_index & 0xff).expect("masked tile X fits u16");
                let tile_y = u16::try_from(map_index >> 8).expect("tile Y fits u16");
                let sample_x = tile_x.wrapping_shl(8).wrapping_add(128);
                let sample_y = tile_y.wrapping_shl(8).wrapping_add(128);
                let sample_z = ground_height(sample_x, sample_y);
                let delta_x =
                    i32::from(slot.position.x.cast_signed()) - i32::from(sample_x.cast_signed());
                let delta_y =
                    i32::from(slot.position.y.cast_signed()) - i32::from(sample_y.cast_signed());
                let delta_z = i32::from(slot.position.z) - i32::from(sample_z);
                let distance = delta_x
                    .wrapping_mul(delta_x)
                    .wrapping_add(delta_y.wrapping_mul(delta_y))
                    .wrapping_add(delta_z.wrapping_mul(delta_z));
                let contribution = if distance < 0x48_000 && shading[map_index] < 63 {
                    let falloff =
                        31_i32.wrapping_mul(0x48_000_i32.wrapping_sub(distance)) / 0x48_000;
                    let raw = falloff.wrapping_mul(i32::from(slot.intensity)) >> 7;
                    let available = 63_i32.wrapping_sub(i32::from(shading[map_index]));
                    jitter.wrapping_add(raw).min(life_cap).min(available).max(0)
                } else {
                    0
                };
                let contribution =
                    u8::try_from(contribution).expect("source contribution clamps to 0..31");
                shading[map_index] = shading[map_index].wrapping_add(contribution);
                slot.history[history_index] = contribution;
            }
        }
    }

    /// Complete future-driving state under a dedicated domain. This remains
    /// separate from the established source-arena v1 digest.
    #[must_use]
    pub fn hash_v1(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut append = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        append(b"mc2-level1-dynamic-lighting-v1\0");
        append(&self.rng.to_le_bytes());
        append(&[self.active_count]);
        for (slot_index, slot) in self.slots.iter().enumerate() {
            append(&[u8::try_from(slot_index).expect("fifty slots fit u8")]);
            append(&[
                u8::from(slot.active),
                u8::from(slot.life_scaled),
                slot.intensity,
                slot.jitter_range,
            ]);
            append(&slot.position.x.to_le_bytes());
            append(&slot.position.y.to_le_bytes());
            append(&slot.position.z.to_le_bytes());
            append(&slot.source_entity_index.to_le_bytes());
            append(&[u8::from(slot.linked)]);
            append(&slot.history);
        }
        hash
    }

    pub fn execution_receipt(
        &self,
        shading: &[u8],
    ) -> Result<DynamicLightingExecutionReceipt, String> {
        if shading.len() != 65_536 {
            return Err(format!(
                "dynamic-light execution shading has {} bytes, expected 65536",
                shading.len()
            ));
        }
        let mut active_slot_mask = 0_u64;
        for (slot_index, slot) in self.slots.iter().enumerate() {
            if slot.active {
                active_slot_mask |= 1_u64 << slot_index;
            }
        }
        if active_slot_mask.count_ones() != u32::from(self.active_count) {
            return Err(format!(
                "dynamic-light active count {} does not match slot mask {active_slot_mask:#014x}",
                self.active_count
            ));
        }
        Ok(DynamicLightingExecutionReceipt {
            state_hash: self.hash_v1(),
            shading_hash: byte_hash(shading),
            active_slot_mask,
            shading_bytes: u32::try_from(shading.len()).expect("fixed shading extent fits u32"),
            active_count: self.active_count,
        })
    }

    #[cfg(test)]
    pub(crate) fn slots_mut(&mut self) -> &mut [DynamicLightSlot; DYNAMIC_LIGHT_CAPACITY] {
        &mut self.slots
    }

    #[cfg(test)]
    pub(crate) fn set_rng(&mut self, rng: u32) {
        self.rng = rng;
    }
}

fn byte_hash(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, byte| {
            hash ^= u64::from(*byte);
            hash.wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn light_map_walk(position: Position) -> impl Iterator<Item = usize> {
    let center_x = position.x.to_be_bytes()[0];
    let center_y = position.y.to_be_bytes()[0];
    (0..5).flat_map(move |row| {
        let tile_y = center_y.wrapping_add_signed(i8::try_from(row).expect("row fits i8") - 2);
        (0..5).map(move |column| {
            let offset = if row % 2 == 0 {
                i8::try_from(column).expect("column fits i8") - 2
            } else {
                2 - i8::try_from(column).expect("column fits i8")
            };
            let tile_x = center_x.wrapping_add_signed(offset);
            usize::from(tile_x) + usize::from(tile_y) * 256
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_light_array_preserves_history_and_source_walk_order() {
        let mut lighting = DynamicLightingState::reset();
        let slot = lighting
            .admit(
                true,
                true,
                17,
                Position {
                    x: 0x2080,
                    y: 0x3080,
                    z: 0,
                },
                128,
                7,
                true,
            )
            .unwrap();
        assert_eq!(slot, 0);
        let mut shading = vec![0_u8; 65_536];
        lighting.service_recompute(
            &mut shading,
            true,
            |_| {
                Some(DynamicLightLinkedEntity {
                    position: Position {
                        x: 0x2080,
                        y: 0x3080,
                        z: 0,
                    },
                    life: 31,
                    disabled: false,
                })
            },
            |_, _| 0,
        );
        assert_eq!(lighting.rng(), 0x84ca_7cd2);
        assert_eq!(
            lighting.slots()[0].history,
            [
                0, 0, 3, 0, 0, 0, 17, 24, 17, 0, 3, 24, 31, 24, 3, 0, 17, 24, 17, 0, 0, 0, 3, 0, 0,
            ]
        );
        let lit = shading.clone();
        lighting.subtract_previous(&mut shading, true);
        assert!(shading.iter().all(|value| *value == 0));
        assert!(lit.iter().any(|value| *value != 0));
        let history = lighting.slots()[0].history;
        lighting.retire(0);
        assert_eq!(lighting.active_count(), 0);
        assert_eq!(lighting.slots()[0].history, history);
    }

    #[test]
    fn admission_schedule_movement_retirement_and_hash_are_source_bounded() {
        let position_a = Position {
            x: 0x2080,
            y: 0x2080,
            z: 0,
        };
        let position_b = Position {
            x: 0x6080,
            y: 0x6080,
            z: 0,
        };
        let mut rejected = DynamicLightingState::reset();
        assert_eq!(
            rejected.admit(true, false, 1, position_a, 128, 7, true),
            None
        );
        assert_eq!(
            rejected.admit(false, true, 1, position_a, 128, 7, true),
            None
        );
        assert_eq!(rejected.rng(), DYNAMIC_LIGHT_RESET_RNG);

        let reset_hash = rejected.hash_v1();
        let mut lighting = DynamicLightingState::reset();
        assert_eq!(
            lighting.admit(true, true, 1, position_a, 128, 7, true),
            Some(0)
        );
        let mut shading = vec![0_u8; 65_536];
        lighting.service_recompute(
            &mut shading,
            true,
            |_| {
                Some(DynamicLightLinkedEntity {
                    position: position_a,
                    life: 31,
                    disabled: false,
                })
            },
            |_, _| 0,
        );
        let lit_a = shading.clone();
        let live_hash = lighting.hash_v1();
        assert_ne!(live_hash, reset_hash);
        lighting.subtract_previous(&mut shading, true);
        assert!(shading.iter().all(|value| *value == 0));
        lighting.service_recompute(
            &mut shading,
            true,
            |_| {
                Some(DynamicLightLinkedEntity {
                    position: position_b,
                    life: 31,
                    disabled: false,
                })
            },
            |_, _| 0,
        );
        assert_eq!(lighting.slots()[0].position, position_b);
        assert_ne!(shading, lit_a);
        for map_index in light_map_walk(position_a) {
            assert_eq!(
                shading[map_index], 0,
                "moved light leaves no stale old-position footprint"
            );
        }

        lighting.subtract_previous(&mut shading, true);
        let history_before_retire = lighting.slots()[0].history;
        lighting.service_recompute(
            &mut shading,
            true,
            |_| {
                Some(DynamicLightLinkedEntity {
                    position: position_b,
                    life: -1,
                    disabled: false,
                })
            },
            |_, _| 0,
        );
        assert_eq!(lighting.active_count(), 0);
        assert_eq!(lighting.slots()[0].history, history_before_retire);
        assert!(shading.iter().all(|value| *value == 0));
        let retired_hash = lighting.hash_v1();
        lighting.slots_mut()[0].history[12] ^= 1;
        assert_ne!(
            lighting.hash_v1(),
            retired_hash,
            "inactive retained history remains in the future-driving domain"
        );

        let mut full = DynamicLightingState::reset();
        for slot in 0..DYNAMIC_LIGHT_CAPACITY {
            assert_eq!(
                full.admit(
                    true,
                    true,
                    u16::try_from(slot + 1).unwrap(),
                    position_a,
                    1,
                    1,
                    false,
                ),
                Some(u8::try_from(slot).unwrap())
            );
        }
        assert_eq!(full.active_count(), 50);
        assert_eq!(full.admit(true, true, 999, position_a, 1, 1, false), None);
        full.slots_mut()[7].history[3] = 19;
        full.retire(7);
        assert_eq!(
            full.admit(true, true, 999, position_b, 2, 3, false),
            Some(7)
        );
        assert_eq!(full.slots()[7].history, [0; 25]);

        let mut existing_after_option_off = DynamicLightingState::reset();
        existing_after_option_off.set_rng(DYNAMIC_LIGHT_RESET_RNG);
        existing_after_option_off
            .admit(true, true, 5, position_a, 1, 1, false)
            .unwrap();
        existing_after_option_off.service_recompute(
            &mut shading,
            true,
            |_| {
                Some(DynamicLightLinkedEntity {
                    position: position_a,
                    life: 31,
                    disabled: false,
                })
            },
            |_, _| 0,
        );
        assert_eq!(
            existing_after_option_off.rng(),
            0x84ca_7cd2,
            "service continues after the admission option is switched off"
        );
    }
}
