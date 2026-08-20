//! Authored class-5 constructor and cursor dispatch.
//!
//! The source arena, free stack, RNG, activation schedule, reset, and hash
//! composition remain owned by the enclosing Level 2 economy/session. This
//! module only gives the existing family packet and model-specific callbacks
//! narrow physical work surfaces. The ten reachable non-Bee creature draws
//! remain single-sourced in `crate::goat` and `crate::villager`; the enclosing
//! Level 2 session reaches those handlers through the same source-ordered
//! cursor. This module must not grow a parallel Goat/Villager arena or RNG
//! sidecar while its class-5 authored rows are folded into the campaign. The
//! three reachable creature audio calls likewise remain the typed
//! `GoatStepEvent::Bleat` carrier from those handlers; this module owns the
//! production-route receipts without introducing a second audio journal.

use super::*;

pub(super) mod archer;
mod model3;
mod model9;
pub(super) mod trader;

#[cfg(test)]
mod tests;

impl Level2EconomyState {
    /// Allocate an authored class-5/model-3, model-4, or model-9 row through
    /// the pinned `sub_4B6F0`/`AddArchers_4BA10`/`sub_4BBB0` constructors.  The model-3 constructor
    /// also creates a sixteen-child particle chain in remc2; this bounded
    /// source row owns the root allocation and leaves that presentation
    /// child callback for the dedicated creature slice.
    pub fn allocate_authored_class5(
        &mut self,
        authored_index: u16,
        model: u8,
        position: Position,
    ) -> Option<Level2AuthoredClass5Source> {
        if model == trader::TRADER_MODEL {
            return self.allocate_authored_trader(authored_index, position);
        }
        if model == archer::ARCHER_MODEL {
            return self.allocate_authored_archer(authored_index, position);
        }
        let (
            action,
            minimum_speed,
            maximum_speed,
            actual_speed,
            max_life,
            mana,
            max_mana,
            subspell_index,
            dword_0x10,
            particle_index,
        ) = match model {
            3 => (25, 64, 16, 30, 9_000, 2_250, 4_500, 0, 0, 88),
            9 => (72, 20, 0, 20, 1_000, 500, 0, 500, 16, 220),
            _ => return None,
        };
        // The model-3 constructor is a root-plus-sixteen allocation.  Keep
        // the whole constructor transactional so an exhausted arena cannot
        // strand a root, a partial chain, a generation, or an RNG sidecar.
        let state_before = (model == 3).then(|| self.clone());
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        let entity_rng = level2_next_rng16(initial_rng);
        let source_byte_3e = match model {
            3 => {
                let phase = self.class5_model3_constructor_phase;
                self.class5_model3_constructor_phase = phase.wrapping_add(1);
                phase
            }
            4 => {
                let phase = self.class5_model4_constructor_phase;
                self.class5_model4_constructor_phase = phase.wrapping_add(1);
                phase
            }
            9 => {
                let phase = self.class5_model9_constructor_phase;
                self.class5_model9_constructor_phase = phase.wrapping_add(1);
                phase
            }
            _ => unreachable!("validated authored class-5 model"),
        };
        // AddArchers consumes the random orientation draw, then
        // SetEntityIndexAndRot(0) overwrites it with particle row 0's final
        // pose: rotation_speed/2 = 125 and speed/2 = 66.
        let (yaw, pitch, roll, fov) = if model == 4 {
            (125, 66, 66, 125)
        } else if model == 3 {
            (90, 414, 414, 108)
        } else {
            (0, 0, 0, 0)
        };
        let mut position = position;
        position.z = self.terrain.ground_height(position);
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
            if let Some(state_before) = state_before {
                *self = state_before;
            } else {
                self.release_slot(source_entity_index, None);
            }
            return None;
        }
        let mut source = Level2AuthoredClass5Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model,
            action,
            minimum_speed,
            maximum_speed,
            actual_speed,
            max_life,
            life: max_life,
            mana,
            max_mana,
            subspell_index,
            dword_0x10: if matches!(model, 3 | 4) {
                i32::from(source_entity_index % 100)
            } else {
                dword_0x10
            },
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index,
            source_byte_3e,
            source_byte_39: if model == 3 { 34 } else { 0 },
            yaw,
            pitch,
            roll,
            fov,
            shift_rot_x: if model == 4 {
                128
            } else if model == 9 {
                128
            } else {
                0
            },
            shift_rot_y: if model == 4 {
                256
            } else if model == 9 {
                128
            } else {
                0
            },
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        let model3_state = if model == 3 {
            match self.allocate_model3_chain(&mut source) {
                Some(state) => Some(state),
                None => {
                    *self = state_before.expect("model-3 allocation snapshot");
                    return None;
                }
            }
        } else {
            None
        };
        self.authored_class5.push(source);
        if let Some(model3_state) = model3_state {
            self.authored_class5_model3.push(model3_state);
        }
        Some(source)
    }

    /// Advance one authored class-5 root through the live source cursor.
    /// Creature action tables are intentionally separate; this preserves the
    /// constructor's live row and map relation without inventing combat AI.
    pub fn step_authored_class5_slot(&mut self, source_slot: u16) -> bool {
        self.step_authored_class5_slot_with_target(source_slot, None)
    }

    /// Advance one authored class-5 root with the imported target snapshot
    /// resolved by the owning reset session. The economy state does not own
    /// the imported entity arena, so the outer session supplies this
    /// source-backed relation without coupling the economy projection to the
    /// world container.
    pub(super) fn step_authored_class5_slot_with_target(
        &mut self,
        source_slot: u16,
        target: Option<EntitySnapshot>,
    ) -> bool {
        let Some(source_index) = self
            .authored_class5
            .iter()
            .position(|source| source.active && source.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        if source.model == 3 && matches!(source.action, 25 | 216..=220) {
            self.step_authored_class5_model3(&mut source, target);
        } else if source.model == 9 && matches!(source.action, 72 | 73) {
            self.step_authored_class5_model9(&mut source);
        }
        // `ApplyEvents_498A0` advances the live class-5 cursor after every
        // callback; `sub_29710` consumes this pre-increment value for its
        // 0x03/0x1f target cadence gates.
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
        self.authored_class5[source_index] = source;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }
}
