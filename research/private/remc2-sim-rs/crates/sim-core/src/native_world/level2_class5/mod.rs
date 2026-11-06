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
pub(crate) mod firefly;
mod model9;
pub(super) mod trader;
pub(crate) use trader::TRADER_MAX_LIFE;

#[cfg(test)]
pub(crate) use archer::ARCHER_ACTION_HIT;
pub(crate) use archer::ARCHER_MAX_LIFE;
pub(crate) use dragon::DRAGON_LIFE;

pub(crate) const BEE_MAX_LIFE: i32 = crate::level_definition::BEE_CLASS_PROPERTIES.max_life;
pub(crate) const WORM_MAX_LIFE: i32 = 9_000;

#[cfg(test)]
mod tests;

/// Consume the one source-shaped damage mailbox visit for a Day Bee.
///
/// The pinned `sub_1F660 -> sub_1BF90` path clears only the attacker word;
/// the damage dword remains the source's last applied payload.  Returning
/// `Some(lethal)` tells the enclosing cursor to skip the ordinary action
/// body: the source branches directly to action 18 or action 20 from this
/// callback, so no target scan or extra RNG draw is permitted on the same
/// visit.
impl Level2EconomyState {
    pub(super) fn consume_authored_bee_damage_mailbox(
        &mut self,
        source: &mut BeeCreatureRuntime,
    ) -> Option<bool> {
        let hit = creatures::bee::consume_action17_mailbox(source)?;
        self.events.push(Level2EconomyEvent::DamageApplied {
            target_entity_index: source.entity_id,
            attacker_entity_index: hit.attacker,
            damage: hit.damage,
            lethal: hit.lethal,
        });
        Some(hit.lethal)
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

    pub(crate) fn decrement_source_creature_population(&mut self) {
        if self.source_creature_population != 0 {
            self.source_creature_population -= 1;
        }
    }

    pub(crate) fn increment_source_creature_population(&mut self) {
        self.source_creature_population = self.source_creature_population.saturating_add(1);
    }

    /// Run the phase-gated `sub_1F850 -> KillEntity_1C930` tail for an
    /// action-21 Bee.  Phases 1..7 remain live and consume no allocations or
    /// RNG; the source's byte-0x3E epilogue advances after this helper and
    /// admits the exact output/reclaim branch only at phase 0 modulo 8.
    pub(super) fn step_authored_bee_death_tail(
        &mut self,
        source: &mut BeeCreatureRuntime,
        terrain: &SessionTerrain,
    ) {
        if !creatures::bee::death_conversion_requested(source) {
            return;
        }

        let _ = self.spawn_mana_sphere_from_death(
            source.source_entity_index,
            source.position,
            source.yaw,
            source.entity_rng,
            source.mana,
            self.source_owner(source.source_entity_index),
            terrain,
        );
        if source.control_flags & 0x0010_0000 == 0 {
            let _ = self.spawn_death_burst_from_death_with_id(
                source.source_entity_index,
                source.entity_id,
                source.position,
            );
        }
        self.deferred_bee_reclaims
            .insert(source.source_entity_index);
    }
}

impl Level2EconomyState {
    /// Allocate an authored class-5/model-1, model-3, model-4, or model-9 row through
    /// the pinned `AddCreature_4B490`/`sub_4B6F0`/`AddArchers_4BA10`/`sub_4BBB0`
    /// constructors. The model-3 constructor
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
        self.allocate_authored_class5_with_max_life(authored_index, model, position, None, terrain)
    }

    /// Allocate a semantic Goat through the ordinary model-1 constructor.
    pub fn allocate_authored_goat(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if max_life <= 0 {
            return None;
        }
        self.allocate_authored_class5_with_max_life(
            authored_index,
            crate::goat::GOAT_MODEL,
            position,
            Some(max_life),
            terrain,
        )
    }

    /// Allocate a semantic Skeleton through the ordinary model-9 constructor
    /// while preserving the package's source-width and lifecycle fields.
    pub fn allocate_authored_skeleton(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if max_life <= 0 {
            return None;
        }
        self.allocate_authored_class5_with_max_life(
            authored_index,
            9,
            position,
            Some(max_life),
            terrain,
        )
    }

    /// Allocate a semantic Worm root and its sixteen copied body rows through
    /// the ordinary model-3 constructor.
    pub fn allocate_authored_worm(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if max_life <= 0 {
            return None;
        }
        self.allocate_authored_class5_with_max_life(
            authored_index,
            3,
            position,
            Some(max_life),
            terrain,
        )
    }

    fn allocate_authored_class5_with_max_life(
        &mut self,
        authored_index: u16,
        model: u8,
        position: Position,
        max_life_override: Option<i32>,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if model == dragon::DRAGON_MODEL {
            return self.allocate_authored_dragon(
                authored_index,
                position,
                max_life_override.unwrap_or(dragon::DRAGON_LIFE),
                terrain,
            );
        }
        if model == trader::TRADER_MODEL {
            return max_life_override.and_then(|max_life| {
                self.allocate_authored_trader(authored_index, position, max_life, terrain)
            });
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
            crate::goat::GOAT_MODEL => (
                crate::goat::GOAT_CONSTRUCTOR_ACTION,
                crate::goat::GOAT_CONSTRUCTOR_MINIMUM_SPEED,
                crate::goat::GOAT_CONSTRUCTOR_MAXIMUM_SPEED,
                crate::goat::GOAT_CONSTRUCTOR_MAXIMUM_SPEED,
                max_life_override.unwrap_or(crate::goat::GOAT_CONSTRUCTOR_LIFE),
                crate::goat::GOAT_CONSTRUCTOR_MANA,
                0,
                0,
                0,
                crate::goat::GOAT_RENDER_PARAMETER_INDEX,
            ),
            3 => (
                25,
                64,
                16,
                30,
                max_life_override.unwrap_or(WORM_MAX_LIFE),
                2_250,
                4_500,
                0,
                0,
                88,
            ),
            9 => (
                72,
                crate::level_definition::SKELETON_CLASS_PROPERTIES.minimum_speed,
                0,
                crate::level_definition::SKELETON_CLASS_PROPERTIES.minimum_speed,
                max_life_override
                    .unwrap_or(crate::level_definition::SKELETON_CLASS_PROPERTIES.max_life),
                crate::level_definition::SKELETON_CLASS_PROPERTIES.max_life >> 1,
                0,
                crate::level_definition::SKELETON_CLASS_PROPERTIES.initial_subspell,
                16,
                super::campaign_shared_families::MC2_SKELETON_ASSETS.initial_particle,
            ),
            _ => return None,
        };
        // Retain a snapshot only for failure before the model-3 root itself
        // is published. Once the root exists, sub_4B6F0 keeps any prefix of
        // body rows created before NewEvent exhaustion.
        let state_before = (model == 3).then(|| self.clone());
        let source_entity_index = self.allocate_slot()?;
        let initial_rng = self.new_event_rng(source_entity_index);
        // `AddCreature_4B490` performs no constructor RNG draw. The other
        // constructors consume their source-local orientation draw here.
        let entity_rng = if model == crate::goat::GOAT_MODEL {
            initial_rng
        } else {
            level2_next_rng16(initial_rng)
        };
        let source_byte_3e = match model {
            crate::goat::GOAT_MODEL => {
                let phase = self.class5_model1_constructor_phase;
                self.class5_model1_constructor_phase = phase.wrapping_add(1);
                phase
            }
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
            dword_0x10: if matches!(model, 1 | 3 | 4) {
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
            source_byte_39: if model == 3 {
                34
            } else if model == crate::goat::GOAT_MODEL {
                crate::goat::GOAT_DESCRIPTOR_CADENCE.cast_unsigned()
            } else {
                0
            },
            yaw: if model == 3 {
                i16::try_from(entity_rng & 0x07ff).expect("11-bit Worm yaw") - 1
            } else {
                yaw
            },
            pitch: if model == 3 {
                i16::try_from(entity_rng & 0x07ff).expect("11-bit Worm pitch") - 1
            } else {
                pitch
            },
            roll: if model == 3 {
                i16::try_from(entity_rng & 0x07ff).expect("11-bit Worm roll") - 1
            } else {
                roll
            },
            fov: if model == 3 { 0 } else { fov },
            shift_rot_x: if model == crate::goat::GOAT_MODEL {
                crate::goat::GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT
            } else if model == 4 {
                128
            } else if model == 9 {
                128
            } else {
                0
            },
            shift_rot_y: if model == crate::goat::GOAT_MODEL {
                crate::goat::GOAT_CONSTRUCTOR_COLLISION_HALF_EXTENT
            } else if model == 4 {
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
            Some(self.allocate_model3_chain(&mut source))
        } else {
            None
        };
        if model == 3 {
            // `sub_4B6F0` links every body before it links the root. Reinsert
            // the already-tracked root so the final authoritative same-tile
            // topology is root -> last body -> ... -> first body.
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_entity_index,
                    active: false,
                    x: position.x,
                    y: position.y,
                })
                .expect("Worm root unlinks from the map chain");
            self.map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: source_entity_index,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .expect("Worm root relinks at the map-chain head");
        }
        if model == crate::goat::GOAT_MODEL {
            self.source_owner.insert(source_entity_index, 0);
            self.authored_class5_runtime
                .push(Level2AuthoredClass5RuntimeState {
                    source_entity_index,
                    byte_0x38: 1,
                    xtype: 3,
                    descriptor_index: crate::goat::GOAT_DESCRIPTOR_INDEX,
                    pending_damage: 0,
                    linked_head: 0,
                });
        }
        self.authored_class5.push(source);
        if let Some(model3_state) = model3_state {
            self.authored_class5_model3.push(model3_state);
        }
        Some(source)
    }

    /// Copy only Goat-owned mutable fields into its generic NewEvent/map
    /// projection. The Goat family remains behavior authority; the generic
    /// class-5 row remains allocation, cursor, hash, and map identity.
    pub(crate) fn sync_authored_goat_projection(&mut self, goat: &GoatState) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.model == crate::goat::GOAT_MODEL
                && source.source_entity_index == goat.entity_index
        }) else {
            // Reset-imported Goats have no generic NewEvent projection.
            return true;
        };

        let drawing_enabled = goat.control_flags & ENTITY_DRAW_DISABLED_BIT == 0;
        let source = &mut self.authored_class5[source_index];
        source.entity_rng = goat.rng;
        source.action = goat.action;
        source.minimum_speed = goat.minimum_speed;
        source.maximum_speed = goat.maximum_speed;
        source.actual_speed = goat.actual_speed;
        source.max_life = goat.max_life;
        source.life = goat.life;
        source.mana = goat.mana;
        source.pending_word_0x62 = goat.pending_attacker.cast_unsigned();
        source.word_0x26 = goat.current_attacker.cast_unsigned();
        source.word_0x24 = goat.kill_credit_entity.cast_unsigned();
        source.word_0x96_150 = goat.target_entity_index.unwrap_or(0);
        source.source_byte_3e = goat.phase;
        source.source_byte_39 = goat.proximity_countdown.cast_unsigned();
        source.yaw = goat.yaw;
        source.pitch = goat.pitch;
        source.roll = goat.roll;
        source.position = goat.position;
        source.drawing_enabled = drawing_enabled;
        // Hide is not reclaim. Keep the numeric occupant allocated until the
        // following UpdateEntities prologue, including later group producers
        // which rebuild NewEvent's free arena in this same pass.
        source.active = goat.active;
        self.source_rng.insert(goat.entity_index, goat.rng);
        if let Some(runtime) = self.authored_class5_runtime_mut(goat.entity_index) {
            runtime.pending_damage = goat.pending_damage;
        }
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: goat.entity_index,
                active: goat.active,
                x: goat.position.x,
                y: goat.position.y,
            })
            .is_ok()
    }

    pub(super) fn reclaim_hidden_goat_rows_at_source_boundary(&mut self) {
        let mut hidden = self
            .authored_class5
            .iter()
            .filter(|source| source.model == crate::goat::GOAT_MODEL && !source.drawing_enabled)
            .map(|source| (source.source_entity_index, source.entity_rng))
            .collect::<Vec<_>>();
        hidden.sort_by_key(|(slot, _)| *slot);
        for (slot, rng) in hidden {
            self.authored_class5
                .retain(|source| source.source_entity_index != slot);
            self.authored_class5_runtime
                .retain(|source| source.source_entity_index != slot);
            let _ = self.map_chains.sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: false,
                x: 0,
                y: 0,
            });
            self.release_slot(slot, Some(rng));
        }
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
