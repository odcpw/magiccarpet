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
pub(super) mod dragon;
pub(super) mod firefly;
mod model3;
mod model9;
pub(super) mod trader;

#[cfg(test)]
mod tests;

/// Consume the one source-shaped damage mailbox visit for a Day Bee.
///
/// The pinned `sub_1F660 -> sub_1BF90` path clears only the attacker word;
/// the damage dword remains the source's last applied payload.  Returning
/// `Some(lethal)` tells the enclosing cursor to skip the ordinary action-17
/// body: the source branches directly to action 18 or action 20 from this
/// callback, so no target scan or extra RNG draw is permitted on the same
/// visit.
impl Level2EconomyState {
    pub(super) fn consume_authored_bee_damage_mailbox(
        &mut self,
        source: &mut Level2AuthoredBeeSource,
    ) -> Option<bool> {
        if source.action != 17 || source.pending_attacker_entity_index == 0 {
            return None;
        }

        let attacker_entity_index = source.pending_attacker_entity_index;
        let damage = source.pending_damage;
        let damage_i16 = i16::from_ne_bytes((damage as u16).to_ne_bytes());
        source.life = source.life.wrapping_sub(damage_i16);
        source.damage_attacker_entity_index = attacker_entity_index;
        // `sub_1BF90` clears word 0x62 but retains dword 0x5E for the
        // source-visible post-hit payload and later lethal bookkeeping.
        source.pending_attacker_entity_index = 0;

        let lethal = source.life < 0;
        if lethal {
            // The source copies word 0x26 to word 0x24 before entering the
            // action-20 pre-kill callback.
            source.kill_credit_entity_index = attacker_entity_index;
            source.action = 20;
        } else {
            // `sub_1BF90` writes the qualifying attacker into word 0x96 and
            // selects action 18.  `sub_1F660` then consumes exactly one
            // source-local LCG step for the action-18 sound tail.
            source.target_entity_index = Some(attacker_entity_index);
            source.action = 18;
            source.entity_rng = level2_next_rng16(source.entity_rng);
            source.last_sound_id = (source.entity_rng & 1) + 12;
            source.dword_0x10 = 1;
        }
        self.events.push(Level2EconomyEvent::DamageApplied {
            target_entity_index: source.entity_id,
            attacker_entity_index,
            damage,
            lethal,
        });
        Some(lethal)
    }

    /// Record one `PreKillEntity_1C890` creature-credit increment.  The
    /// source owns this counter on the qualifying class-3/model-0 player
    /// carrier; the typed Level 2 package does not serialize that unrelated
    /// Type_str_164 byte, so this reset-scoped map is its lossless projection.
    pub fn record_authored_creature_kill(&mut self, killer_entity_index: u16) -> bool {
        if killer_entity_index == 0 {
            return false;
        }
        let count = self
            .authored_bee_kill_census
            .entry(killer_entity_index)
            .or_default();
        *count = count.wrapping_add(1);
        true
    }

    /// Frozen compatibility name for existing Bee-specific callers/tests.
    pub fn record_authored_bee_kill(&mut self, killer_entity_index: u16) -> bool {
        self.record_authored_creature_kill(killer_entity_index)
    }

    #[must_use]
    pub fn authored_bee_kill_census(&self, killer_entity_index: u16) -> u32 {
        self.authored_bee_kill_census
            .get(&killer_entity_index)
            .copied()
            .unwrap_or(0)
    }

    /// Run the phase-gated `sub_1F850 -> KillEntity_1C930` tail for an
    /// action-21 Bee.  Phases 1..7 remain live and consume no allocations or
    /// RNG; the source's byte-0x3E epilogue advances after this helper and
    /// admits the exact output/reclaim branch only at phase 0 modulo 8.
    pub(super) fn step_authored_bee_death_tail(
        &mut self,
        source: &mut Level2AuthoredBeeSource,
        terrain: &SessionTerrain,
    ) {
        if source.source_byte_3e & 7 != 0 {
            return;
        }

        let _ = self.spawn_mana_sphere_from_death(
            source.source_entity_index,
            source.position,
            source.yaw,
            source.entity_rng,
            source.mana,
            source.entity_id,
            terrain,
        );
        // The Bee constructor has no suppression bit, so the ordered
        // class-10/model-1 burst follows the mana sphere allocation.
        let _ = self.spawn_death_burst_from_death_with_id(
            source.source_entity_index,
            source.entity_id,
            source.position,
        );
        source.action = 21;
        source.drawing_enabled = false;
        source.active = false;
        source.life = -1;
        source.target_entity_index = None;
        self.deferred_authored_bee_reclaims
            .insert(source.source_entity_index);
    }
}

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
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if model == trader::TRADER_MODEL {
            return self.allocate_authored_trader(authored_index, position, terrain);
        }
        if model == archer::ARCHER_MODEL {
            return self.allocate_authored_archer(authored_index, position, terrain);
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
        position.z = terrain.ground_height(position);
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
    pub fn step_authored_class5_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
    ) -> bool {
        self.step_authored_class5_slot_with_target(source_slot, None, terrain)
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
        terrain: &SessionTerrain,
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
            self.step_authored_class5_model3(&mut source, target, terrain);
        } else if source.model == 9 && matches!(source.action, 72 | 73) {
            self.step_authored_class5_model9(&mut source, terrain);
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
