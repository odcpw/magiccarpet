//! Authored class-5/model-19 Firefly construction, patrol, roaming, and charge.
//!
//! The ordinary actions 159, 153, 154, and 155 plus the 156/157 death tail are
//! implemented here. Fireball emission, death conversion, and reclaim use the
//! shared dynamic arena rather than a Level-1-only creature path.

use super::super::*;
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena, SourceEntityHandle};
use crate::goat::GoatPlayerTarget;
use crate::native_world::{ParticleParameter, RuntimeSnapshot, SessionTerrain};

pub(crate) const FIREFLY_MODEL: u8 = 19;
pub(crate) const FIREFLY_ROAM_ACTION: u8 = 153;
pub(crate) const FIREFLY_ATTACK_ACTION: u8 = 154;
pub(crate) const FIREFLY_LINKED_ACTION: u8 = 155;
pub(crate) const FIREFLY_PRE_KILL_ACTION: u8 = 156;
pub(crate) const FIREFLY_KILL_ACTION: u8 = 157;
pub(crate) const FIREFLY_STAGE_ACTION: u8 = 159;
pub(crate) const FIREFLY_MINIMUM_SPEED: i16 = 76;
pub(crate) const FIREFLY_MAXIMUM_SPEED: i16 = 8;
pub(crate) const FIREFLY_LIFE: i32 = 600;
pub(crate) const FIREFLY_MANA: i32 = 300;
pub(crate) const FIREFLY_SUBSPELL: u16 = 300;
pub(crate) const FIREFLY_DESCRIPTOR: u16 = 88;
pub(crate) const FIREFLY_PARTICLE: u16 = 287;
pub(crate) const FIREFLY_SHIFT_X: i16 = 85;
pub(crate) const FIREFLY_SHIFT_Y: i16 = 51;

/// Source `sub_3A5B0`'s Army child construction, using the canonical typed
/// Firefly owner and the slot selected by SourceArena exactly once.
pub(crate) fn allocate_summoned(
    arena: &mut SourceArena,
    _runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    position: Position,
    parent_slot: u16,
    owner_id: u16,
    holder_slot: u16,
) -> std::result::Result<Option<SourceEntityHandle>, String> {
    particles
        .get(usize::from(FIREFLY_PARTICLE))
        .and_then(ParticleParameter::animation_limit)
        .ok_or_else(|| "Firefly Army summon requires retained particle287".to_owned())?;

    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let route = FireflyStageRoute {
        stage_var1: 0,
        stage_var2: 13,
        target: Position::default(),
    };
    arena
        .economy
        .initialize_authored_firefly_at_slot(
            slot,
            None,
            position,
            Some(route),
            FIREFLY_LIFE,
            terrain,
        )
        .ok_or_else(|| format!("Firefly Army summon constructor failed at slot{slot}"))?;

    let source = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|source| source.active && source.source_entity_index == slot)
        .ok_or_else(|| format!("Firefly Army summon lost typed source at slot{slot}"))?;
    source.action = FIREFLY_STAGE_ACTION;
    source.mana = 0;
    source.max_mana = 0;
    source.word_0x96_150 = 0;
    let source_position = source.position;

    let parent_generation = arena.slot_generation(parent_slot);
    let runtime_state = arena
        .economy
        .authored_fireflies
        .iter_mut()
        .find(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("Firefly Army summon lost runtime state at slot{slot}"))?;
    runtime_state.stage_var1 = 0;
    runtime_state.stage_var2 = 13;
    runtime_state.stage_target = Position::default();
    runtime_state.parent_entity_index = parent_slot;
    runtime_state.parent_generation = parent_generation;
    runtime_state.word_0x2e = 250;
    arena.economy.source_owner.insert(slot, owner_id);

    crate::world::effects::sync_position(arena, terrain, slot, source_position, true, false)?;
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("Firefly Army summon lost allocated slot{slot}"))?;
    if !arena.bind_dynamic_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Firefly,
        CreatureRootOrigin::SummonedSpell { holder_slot },
    ) {
        return Err(format!(
            "Firefly Army summon at slot{slot} lost generation-bound origin"
        ));
    }
    Ok(Some(handle))
}

/// Run action 159's parent-bound StageVar 13, 16 and 17 branches.
///
/// `sub_1E320` first moves a boss-launched child towards its launch target;
/// `sub_1E580` then consumes its parent and lifetime on scheduled visits,
/// chooses a class-3 model-0/1 target from the frozen source list, and hides
/// the root only after publishing its real class-10 expiry children. The
/// source scheduler still owns the byte-0x3E tail and next-pass reclaim.
pub(crate) fn step_summoned_stage_slot(
    arena: &mut SourceArena,
    source_slot: u16,
    players: &[GoatPlayerTarget],
    pass_start_class3_slots: &[u16],
    frozen_model19_slots: &[u16],
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
) -> std::result::Result<bool, String> {
    let Some(source_index) = arena.economy.authored_class5.iter().position(|source| {
        source.active
            && source.drawing_enabled
            && source.source_entity_index == source_slot
            && source.model == FIREFLY_MODEL
            && source.action == FIREFLY_STAGE_ACTION
    }) else {
        return Ok(false);
    };
    let Some(state_index) = arena
        .economy
        .authored_fireflies
        .iter()
        .position(|state| state.source_entity_index == source_slot)
    else {
        return Err(format!(
            "summoned Firefly slot {source_slot} has no runtime sidecar"
        ));
    };

    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_fireflies[state_index];
    if !matches!(state.stage_var2, 13 | 16 | 17) {
        return Err(format!(
            "parent-bound Firefly slot {source_slot} has unsupported StageVar2 {}",
            state.stage_var2
        ));
    }

    if state.stage_var2 == 17 {
        if step_firefly_motion(&mut source, &mut state, terrain) {
            arena.economy.decrement_source_creature_population();
        }
        if source.life < 0 {
            source.drawing_enabled = false;
        } else {
            // Unlike sub_1E580, sub_1E320 accepts a target with zero life.
            let target = arena
                .entities
                .get(usize::from(source.word_0x96_150))
                .filter(|row| {
                    source.word_0x96_150 != 0
                        && row.active
                        && row.life >= 0
                        && !row.drawing_disabled()
                });
            if let Some(target) = target {
                let position =
                    firefly_stage_position(source.word_0x96_150, target.position, players);
                source.roll =
                    i16::try_from(crate::heading_for_positions(source.position, position))
                        .expect("Firefly launch heading fits i16");
                source.actual_speed = source.actual_speed.wrapping_sub(8);
                if source.actual_speed <= 16 {
                    source.actual_speed = FIREFLY_MINIMUM_SPEED;
                    state.stage_var2 = 16;
                }
            } else {
                state.stage_var2 = 16;
            }
        }
        arena
            .economy
            .source_rng
            .insert(source_slot, source.entity_rng);
        arena.economy.authored_class5[source_index] = source;
        arena.economy.authored_fireflies[state_index] = state;
        crate::world::effects::sync_position(
            arena,
            terrain,
            source_slot,
            source.position,
            source.drawing_enabled,
            false,
        )?;
        return Ok(true);
    }

    let parent_slot = state.parent_entity_index;
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| {
            parent_slot != 0
                && arena.slot_generation(parent_slot) == state.parent_generation
                && parent.active
                && parent.life >= 0
                && !parent.drawing_disabled()
        })
        .map(|parent| FireflyStageTarget {
            source_id: parent.source_id,
            position: firefly_stage_position(parent_slot, parent.position, players),
        });

    if parent.is_some() && state.stage_var2 == 13 {
        state.word_0x2e = state.word_0x2e.wrapping_sub(1);
    } else if parent.is_none() {
        state.word_0x2e = 0;
    }

    if state.word_0x2e.cast_signed() <= 0 {
        arena.economy.authored_fireflies[state_index] = state;
        let handle = arena
            .handle(source_slot)
            .ok_or_else(|| format!("summoned Firefly slot {source_slot} lost its handle"))?;
        if state.stage_var2 == 13 {
            crate::goat::runtime::spawn_expiry_visual(arena, terrain, source.position, particles)?;
        }
        let _ = crate::world::effects::allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            source.position,
            particles,
        )?;

        // Effect constructors can consume replacement slots. Only hide the
        // Firefly generation that entered this callback; never disable a new
        // occupant installed at the same numeric slot by an effect producer.
        if arena.resolve(handle).is_some()
            && let Some(current_index) = arena.economy.authored_class5.iter().position(|current| {
                current.active
                    && current.source_entity_index == source_slot
                    && current.model == FIREFLY_MODEL
            })
        {
            let (position, entity_rng) = {
                let current = &mut arena.economy.authored_class5[current_index];
                // Keep the row allocated through the scheduled same-pass
                // byte-0x3E tail. The following source boundary retires it.
                current.drawing_enabled = false;
                (current.position, current.entity_rng)
            };
            arena.economy.source_rng.insert(source_slot, entity_rng);
            crate::world::effects::sync_position(
                arena,
                terrain,
                source_slot,
                position,
                false,
                false,
            )?;
            return Ok(true);
        }
        return Ok(false);
    }
    let parent = parent.ok_or_else(|| {
        format!("summoned Firefly slot {source_slot} lost its parent before expiry")
    })?;

    let mut target = firefly_live_stage_target(arena, source.word_0x96_150, players);
    if target.is_none() {
        source.word_0x96_150 = 0;
        if source.source_byte_3e & 7 == 0 {
            target = nearest_firefly_stage_target(
                arena,
                parent_slot,
                parent,
                pass_start_class3_slots,
                players,
            );
            source.word_0x96_150 = target.map_or(0, |(slot, _)| slot);
        }
    }

    let acquired_target = target.is_some();
    let movement_target = target.map_or_else(
        || {
            source.word_0x96_150 = parent_slot;
            parent.position
        },
        |(_, target)| target.position,
    );
    if step_firefly_motion(&mut source, &mut state, terrain) {
        arena.economy.decrement_source_creature_population();
    }

    if !acquired_target {
        source.word_0x96_150 = 0;
        state.word_0x2e = state.word_0x2e.wrapping_sub(4);
    }

    if source.source_byte_3e & 7 == 0 {
        if !state.movement_blocked {
            source.roll = i16::try_from(crate::heading_for_positions(
                source.position,
                movement_target,
            ))
            .expect("Firefly StageVar heading fits i16");
            if source.source_byte_3e & 0x3f == 0 {
                apply_firefly_wander(&mut source);
            }
        }
        if let Some(peer) = frozen_model19_slots.iter().find_map(|peer_slot| {
            arena.economy.authored_class5.iter().find(|peer| {
                peer.active
                    && peer.drawing_enabled
                    && peer.life >= 0
                    && peer.model == FIREFLY_MODEL
                    && peer.source_entity_index == *peer_slot
                    && peer.source_entity_index != source_slot
                    && arena.economy.source_owner(*peer_slot)
                        != arena.economy.source_owner(source_slot)
                    && (i32::from(
                        peer.position
                            .x
                            .wrapping_sub(source.position.x)
                            .cast_signed(),
                    ))
                    .abs()
                        < FIREFLY_SEPARATION
                    && (i32::from(
                        peer.position
                            .y
                            .wrapping_sub(source.position.y)
                            .cast_signed(),
                    ))
                    .abs()
                        < FIREFLY_SEPARATION
            })
        }) {
            source.roll =
                i16::try_from(crate::heading_for_positions(peer.position, source.position))
                    .expect("Firefly separation heading fits i16");
        }
    }

    if acquired_target
        && source.source_byte_3e & 7 == 0
        && let Some((target_slot, target)) = target
        && firefly_distance_3d(source.position, target.position) < FIREFLY_RANGE.into()
    {
        source.word_0x96_150 = target_slot;
        source.action = FIREFLY_ATTACK_ACTION;
    }

    arena
        .economy
        .source_rng
        .insert(source_slot, source.entity_rng);
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_fireflies[state_index] = state;
    crate::world::effects::sync_position(
        arena,
        terrain,
        source_slot,
        source.position,
        source.active,
        false,
    )?;
    Ok(source.active)
}

/// Let the normal source-boundary reclaimer release Fireflies hidden during
/// the preceding action callback. This is called immediately before that
/// reclaimer so the row survives its same-pass scheduler tail.
pub(crate) fn retire_hidden_summoned_stage_at_source_boundary(arena: &mut SourceArena) {
    let hidden_slots = arena
        .economy
        .authored_fireflies
        .iter()
        .filter(|state| matches!(state.stage_var2, 13 | 16 | 17))
        .map(|state| state.source_entity_index)
        .collect::<std::collections::BTreeSet<_>>();
    for source in arena.economy.authored_class5.iter_mut().filter(|source| {
        source.active
            && source.model == FIREFLY_MODEL
            && !source.drawing_enabled
            && hidden_slots.contains(&source.source_entity_index)
    }) {
        source.active = false;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FireflyStageTarget {
    source_id: u16,
    position: Position,
}

fn firefly_stage_position(
    source_slot: u16,
    fallback: Position,
    players: &[GoatPlayerTarget],
) -> Position {
    players
        .iter()
        .find(|player| player.entity_index == source_slot)
        .map_or(fallback, |player| player.position)
}

fn firefly_live_stage_target(
    arena: &SourceArena,
    target_slot: u16,
    players: &[GoatPlayerTarget],
) -> Option<(u16, FireflyStageTarget)> {
    let row = arena.entities.get(usize::from(target_slot))?;
    (target_slot != 0 && row.active && row.life > 0 && !row.drawing_disabled()).then_some((
        target_slot,
        FireflyStageTarget {
            source_id: row.source_id,
            position: firefly_stage_position(target_slot, row.position, players),
        },
    ))
}

fn nearest_firefly_stage_target(
    arena: &SourceArena,
    parent_slot: u16,
    parent: FireflyStageTarget,
    pass_start_class3_slots: &[u16],
    players: &[GoatPlayerTarget],
) -> Option<(u16, FireflyStageTarget)> {
    let mut nearest = None;
    let mut nearest_distance = u32::MAX;
    for target_slot in pass_start_class3_slots {
        let Some(row) = arena.entities.get(usize::from(*target_slot)) else {
            continue;
        };
        if row.class != 3
            || !matches!(row.model, 0 | 1)
            || row.source_id == parent.source_id
            || *target_slot == parent_slot
        {
            continue;
        }
        let position = firefly_stage_position(*target_slot, row.position, players);
        let distance = crate::legacy_sqrt(squared_wrapping_xy_distance(parent.position, position));
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest = Some((
                *target_slot,
                FireflyStageTarget {
                    source_id: row.source_id,
                    position,
                },
            ));
        }
    }
    nearest
}

const FIREFLY_DESCRIPTOR_CADENCE: u8 = 35;
const FIREFLY_TURN_LIMIT: i16 = 113;
const FIREFLY_GROUND_CLEARANCE: i16 = 51;
const FIREFLY_DESCENT: i16 = -8;
const FIREFLY_MAX_TILE_SLOPE: i32 = 20;
const FIREFLY_STAGE_REACH: i32 = 2_048;
const FIREFLY_GUARD_REACH: i32 = 3_072;
const FIREFLY_SEPARATION: i32 = 85;
const FIREFLY_LINKED_SEPARATION: i32 = 256;
const FIREFLY_RANGE: u16 = 5_120;
const FIREFLY_VIEW_HALF_ANGLE: i16 = 682;
const FIREFLY_ORBIT_RADIUS: i16 = 2_048;
const FIREFLY_ORBIT_REACHED_DISTANCE: u32 = 1_280;
const FIREFLY_CHARGE_DURATION: i32 = 24;
const FIREFLY_CHARGE_CONTACT_RANGE: u32 = 768;
const FIREFLY_FIREBALL_PAYLOAD: u16 = 500;
/// Imported reset roots have source-slot identity, not an authored placement
/// index. Keep that provenance disjoint from the 1,200 authored records.
pub(crate) const IMPORTED_FIREFLY_AUTHORED_INDEX: u16 = u16::MAX;

/// Adopt the already constructed source roots, without rerunning NewEvent or
/// sub_12330. Captured RNG, counters, slots, stage bindings and map links stay
/// authoritative; this only transfers their live ownership to the Firefly.
pub(crate) fn adopt_imported_fireflies(
    arena: &mut crate::campaign::source_arena::SourceArena,
    runtime: &RuntimeSnapshot,
) -> std::result::Result<(), String> {
    let imported = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 5 && row.model == FIREFLY_MODEL)
        .filter(|row| {
            !arena
                .economy
                .authored_class5
                .iter()
                .any(|source| source.source_entity_index == row.index)
        })
        .cloned()
        .collect::<Vec<_>>();
    for row in imported {
        let record = row.record_bytes();
        let word = |offset| u16::from_le_bytes([record[offset], record[offset + 1]]);
        let stage_var1 = record[76];
        let stage_var2 = record[77];
        let stage_target = if stage_var1 == 0 {
            Position::default()
        } else {
            let stage = runtime
                .stage_variables
                .get(usize::from(stage_var1))
                .filter(|stage| matches!(stage.kind, 1 | 2))
                .ok_or_else(|| {
                    format!(
                        "imported Firefly {} has unsupported StageVar {stage_var1}",
                        row.index
                    )
                })?;
            let RuntimeRelation::AxisScalar(point) = stage.relation else {
                return Err(format!(
                    "imported Firefly {} has no StageVar point",
                    row.index
                ));
            };
            Position {
                x: point as u16,
                y: (point >> 16) as u16,
                z: 0,
            }
        };
        let descriptor_index = row
            .parameter_relation
            .ok_or_else(|| format!("imported Firefly {} has no descriptor", row.index))?;
        arena
            .economy
            .authored_class5
            .push(Level2AuthoredClass5Source {
                source_entity_index: row.index,
                entity_rng: row.entity_rng,
                authored_index: IMPORTED_FIREFLY_AUTHORED_INDEX,
                model: row.model,
                action: row.action,
                minimum_speed: row.minimum_speed(),
                maximum_speed: word(138) as i16,
                actual_speed: row.actual_speed,
                max_life: row.max_life,
                life: row.life,
                mana: row.mana,
                max_mana: row.max_mana,
                subspell_index: row.subspell_index,
                dword_0x10: row.source_dword_10(),
                pending_word_0x62: word(102),
                word_0x26: row.word_26(),
                word_0x24: row.word_24(),
                byte_0x3b: row.byte_3b,
                word_0x96_150: row.source_word_96(),
                byte_0x46_70: record[74],
                particle_index: word(94),
                source_byte_3e: row.phase,
                source_byte_39: record[61],
                yaw: row.yaw,
                pitch: row.pitch,
                roll: row.source_roll(),
                fov: row.fov,
                shift_rot_x: row.array_pitch(),
                shift_rot_y: row.array_fov(),
                position: row.position,
                phase: row.phase,
                drawing_enabled: !row.drawing_disabled(),
                active: true,
            });
        arena
            .economy
            .authored_class5_runtime
            .push(Level2AuthoredClass5RuntimeState {
                source_entity_index: row.index,
                byte_0x38: record[60],
                xtype: record[69],
                descriptor_index,
                pending_damage: i32::from_le_bytes(
                    record[98..102]
                        .try_into()
                        .expect("imported Firefly damage dword extent"),
                ),
                linked_head: row.word_34(),
            });
        let parent_generation = arena.slot_generation(word(44));
        arena
            .economy
            .authored_fireflies
            .push(AuthoredFireflyRuntime {
                source_entity_index: row.index,
                authored_index: None,
                linked_entity_index: row.word_32(),
                control_flags: row.control_flags,
                byte_0x38: record[60],
                xtype: record[69],
                descriptor_index,
                stage_var1,
                stage_var2,
                stage_target,
                movement_blocked: row.control_flags & 0x0004_0000 != 0,
                parent_entity_index: word(44),
                parent_generation,
                word_0x2e: word(50),
            });
        arena.economy.source_rng.insert(row.index, row.entity_rng);
        let old = &mut arena.entities[usize::from(row.index)];
        old.active = false;
        old.record[2] = 0;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireflyAttackTarget {
    pub(crate) entity_index: u16,
    pub(crate) position: Position,
    pub(crate) yaw: i16,
    pub(crate) live: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FireflyAttackStep {
    pub(crate) live: bool,
    pub(crate) sound_id: Option<u16>,
    pub(crate) contact: Option<FireflyContact>,
    pub(crate) fireball: Option<FireflyFireballLaunch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireflyContact {
    pub(crate) target_entity_index: u16,
    pub(crate) attacker_entity_index: u16,
    pub(crate) damage: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireflyFireballLaunch {
    pub(crate) owner_entity_index: u16,
    pub(crate) target_entity_index: u16,
    pub(crate) position: Position,
    pub(crate) target_position: Position,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) payload: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FireflyStageRoute {
    pub(crate) stage_var1: u8,
    pub(crate) stage_var2: u8,
    pub(crate) target: Position,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FireflyLifecycleStep {
    pub(crate) handled: bool,
    pub(crate) retained_for_epilogue: bool,
    pub(crate) pre_kill_credit: Option<u16>,
}

impl Level2EconomyState {
    pub(crate) fn allocate_authored_firefly(
        &mut self,
        authored_index: u16,
        position: Position,
        route: FireflyStageRoute,
        max_life: u32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        let max_life = i32::try_from(max_life).ok().filter(|value| *value > 0)?;
        let source_entity_index = self.allocate_slot()?;
        self.initialize_authored_firefly_at_slot(
            source_entity_index,
            Some(authored_index),
            position,
            Some(route),
            max_life,
            terrain,
        )
    }

    /// Construct an ordinary Firefly when sub_12100 finds no active route.
    /// The source constructor leaves its stage fields zero and action 153.
    pub(crate) fn allocate_authored_firefly_unbound(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: u32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        let max_life = i32::try_from(max_life).ok().filter(|value| *value > 0)?;
        let slot = self.allocate_slot()?;
        self.initialize_authored_firefly_at_slot(
            slot,
            Some(authored_index),
            position,
            None,
            max_life,
            terrain,
        )
    }

    /// Run the canonical Firefly constructor after the shared source arena has
    /// already selected this slot. Summon producers use this path so allocation
    /// provenance, the Firefly RNG/counter sequence, and typed row creation all
    /// refer to one source entity.
    pub(crate) fn initialize_authored_firefly_at_slot(
        &mut self,
        source_entity_index: u16,
        authored_index: Option<u16>,
        position: Position,
        route: Option<FireflyStageRoute>,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if max_life <= 0 {
            self.release_slot(source_entity_index, None);
            return None;
        }
        let initial_rng = self.new_event_rng(source_entity_index);
        let entity_rng = level2_next_rng16(initial_rng);
        let source_byte_3e = self.class5_model19_constructor_phase;
        self.class5_model19_constructor_phase = source_byte_3e.wrapping_add(1);
        let heading = i16::try_from(entity_rng & 0x07ff)
            .expect("Firefly heading fits i16")
            .wrapping_sub(1);
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
            self.release_slot(source_entity_index, None);
            return None;
        }
        // `NewEvent_4A050` zeroes the raw player/source-owner relation. A
        // reclaimed slot may still have a prior carrier entry in this typed
        // arena, so clear it before publishing the new Firefly row.
        self.source_owner.insert(source_entity_index, 0);
        let source = Level2AuthoredClass5Source {
            source_entity_index,
            entity_rng,
            authored_index: authored_index.unwrap_or(IMPORTED_FIREFLY_AUTHORED_INDEX),
            model: FIREFLY_MODEL,
            action: if route.is_some() {
                FIREFLY_STAGE_ACTION
            } else {
                FIREFLY_ROAM_ACTION
            },
            minimum_speed: FIREFLY_MINIMUM_SPEED,
            maximum_speed: FIREFLY_MAXIMUM_SPEED,
            actual_speed: FIREFLY_MINIMUM_SPEED,
            max_life,
            life: max_life,
            mana: FIREFLY_MANA,
            max_mana: 0,
            subspell_index: FIREFLY_SUBSPELL,
            dword_0x10: i32::from(source_entity_index % 100),
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: FIREFLY_PARTICLE,
            source_byte_3e,
            source_byte_39: FIREFLY_DESCRIPTOR_CADENCE
                .wrapping_sub(source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE)
                .wrapping_add(4),
            yaw: heading,
            pitch: heading,
            roll: heading,
            fov: 0,
            shift_rot_x: FIREFLY_SHIFT_X,
            shift_rot_y: FIREFLY_SHIFT_Y,
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };
        self.source_rng.insert(source_entity_index, entity_rng);
        self.authored_class5.push(source);
        self.authored_class5_runtime
            .push(Level2AuthoredClass5RuntimeState {
                source_entity_index,
                byte_0x38: 1,
                xtype: 3,
                descriptor_index: FIREFLY_DESCRIPTOR,
                pending_damage: 0,
                linked_head: 0,
            });
        self.authored_fireflies.push(AuthoredFireflyRuntime {
            source_entity_index,
            authored_index,
            linked_entity_index: 0,
            control_flags: 8,
            byte_0x38: 1,
            xtype: 3,
            descriptor_index: FIREFLY_DESCRIPTOR,
            stage_var1: route.map_or(0, |route| route.stage_var1),
            stage_var2: route.map_or(0, |route| route.stage_var2),
            stage_target: route.map_or(Position::default(), |route| route.target),
            movement_blocked: false,
            parent_entity_index: 0,
            parent_generation: 0,
            word_0x2e: 0,
        });
        Some(source)
    }

    /// Submit one `sub_10C80(a2 = 0)` mailbox write to a live Firefly root.
    pub(crate) fn submit_authored_firefly_damage(
        &mut self,
        target_slot: u16,
        damage: i32,
        attacker: u16,
    ) -> bool {
        if damage <= 0 || attacker == 0 || !self.map_chains.is_linked(target_slot) {
            return false;
        }
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.drawing_enabled
                && source.life >= 0
                && source.source_entity_index == target_slot
                && source.source_entity_index != attacker
                && source.model == FIREFLY_MODEL
        }) else {
            return false;
        };
        let Some(runtime) = self
            .authored_class5_runtime
            .iter_mut()
            .find(|state| state.source_entity_index == target_slot)
        else {
            return false;
        };
        let source = &mut self.authored_class5[source_index];
        runtime.pending_damage = if source.pending_word_0x62 == 0 {
            damage
        } else {
            runtime.pending_damage.wrapping_add(damage)
        };
        source.pending_word_0x62 = attacker;
        true
    }

    /// Consume one Firefly damage mailbox or action-156/157 death visit.
    /// The shared scheduler remains the sole byte-0x3E epilogue owner.
    pub(crate) fn step_authored_firefly_lifecycle_slot(
        &mut self,
        source_slot: u16,
        players: &[GoatPlayerTarget],
        terrain: &SessionTerrain,
    ) -> std::result::Result<FireflyLifecycleStep, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.source_entity_index == source_slot && source.model == FIREFLY_MODEL
        }) else {
            return Ok(FireflyLifecycleStep::default());
        };
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Firefly source slot {source_slot} has no class-5 runtime"))?;
        let firefly_index = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Firefly source slot {source_slot} has no runtime sidecar"))?;
        let mut source = self.authored_class5[source_index];
        let mut runtime = self.authored_class5_runtime[runtime_index];
        let mut firefly = self.authored_fireflies[firefly_index];

        if source.pending_word_0x62 != 0 {
            let prior_action = source.action;
            let guard_hit = prior_action == FIREFLY_STAGE_ACTION && firefly.stage_var2 == 2;
            let attacker = source.pending_word_0x62;
            let same_family = guard_hit
                && self.authored_class5.iter().any(|peer| {
                    peer.active
                        && peer.source_entity_index == attacker
                        && peer.model == FIREFLY_MODEL
                });
            let damage = runtime.pending_damage;
            source.life = source.life.wrapping_sub(damage);
            source.word_0x26 = attacker;
            source.pending_word_0x62 = 0;
            runtime.pending_damage = 0;
            let lethal = source.life < 0;
            if lethal {
                source.word_0x24 = attacker;
                firefly.linked_entity_index = 0;
                source.action = FIREFLY_PRE_KILL_ACTION;
            } else if !same_family {
                if prior_action == FIREFLY_LINKED_ACTION
                    && firefly.linked_entity_index != 0
                    && let Some(peer_index) = self.authored_class5.iter().position(|peer| {
                        peer.active
                            && peer.drawing_enabled
                            && peer.life >= 0
                            && peer.model == FIREFLY_MODEL
                            && peer.source_entity_index == firefly.linked_entity_index
                    })
                {
                    let peer_slot = self.authored_class5[peer_index].source_entity_index;
                    self.authored_class5[peer_index].word_0x96_150 = attacker;
                    self.authored_class5[peer_index].byte_0x46_70 = 0;
                    self.authored_class5[peer_index].action = FIREFLY_ATTACK_ACTION;
                    if let Some(peer_state) = self
                        .authored_fireflies
                        .iter_mut()
                        .find(|state| state.source_entity_index == peer_slot)
                    {
                        peer_state.linked_entity_index = 0;
                    }
                }
                source.word_0x96_150 = attacker;
                if guard_hit {
                    firefly.stage_var2 = 10;
                }
                firefly.linked_entity_index = 0;
                source.action = FIREFLY_ATTACK_ACTION;
                source.byte_0x46_70 = if prior_action == FIREFLY_ATTACK_ACTION {
                    7
                } else {
                    0
                };
            } else {
                // sub_1DBF0 still scans after a same-family hit, but its
                // sub_1DDA0/sub_1E1C0 mailbox branch does not move first.
                acquire_firefly_guard_target(&mut source, &mut firefly, players);
            }
            self.events.push(Level2EconomyEvent::DamageApplied {
                target_entity_index: source_slot,
                attacker_entity_index: attacker,
                damage,
                lethal,
            });
            self.authored_class5[source_index] = source;
            self.authored_class5_runtime[runtime_index] = runtime;
            self.authored_fireflies[firefly_index] = firefly;
            return Ok(FireflyLifecycleStep {
                handled: true,
                retained_for_epilogue: true,
                pre_kill_credit: None,
            });
        }

        if source.life < 0
            && !matches!(source.action, FIREFLY_PRE_KILL_ACTION | FIREFLY_KILL_ACTION)
        {
            // Stage action159 also consumes terrain-written negative life.
            // With no new mailbox, sub_1DDA0/sub_1E1C0 clears the attacker.
            source.word_0x26 = 0;
            source.word_0x24 = 0;
            source.action = FIREFLY_PRE_KILL_ACTION;
            self.authored_class5[source_index] = source;
            return Ok(FireflyLifecycleStep {
                handled: true,
                retained_for_epilogue: true,
                pre_kill_credit: None,
            });
        }
        if source.action == FIREFLY_PRE_KILL_ACTION {
            let pre_kill_credit = (source.word_0x24 != 0).then_some(source.word_0x24);
            source.action = FIREFLY_KILL_ACTION;
            self.authored_class5[source_index] = source;
            return Ok(FireflyLifecycleStep {
                handled: true,
                retained_for_epilogue: true,
                pre_kill_credit,
            });
        }
        if source.action != FIREFLY_KILL_ACTION {
            return Ok(FireflyLifecycleStep::default());
        }
        if source.source_byte_3e & 7 == 0 {
            if source.mana > 0 {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                let owner = self.source_owner(source_slot);
                let _ = self.spawn_mana_sphere_from_death(
                    source_slot,
                    source.position,
                    source.yaw,
                    source.entity_rng,
                    source.mana,
                    owner,
                    terrain,
                );
            }
            if firefly.control_flags & 0x0010_0000 == 0 {
                let _ = self.spawn_death_burst_from_death_with_id(
                    source_slot,
                    source_slot,
                    source.position,
                );
            }
            source.drawing_enabled = false;
            source.active = false;
            firefly.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            self.source_rng.insert(source_slot, source.entity_rng);
            self.authored_class5[source_index] = source;
            self.authored_fireflies[firefly_index] = firefly;
        }
        Ok(FireflyLifecycleStep {
            handled: true,
            retained_for_epilogue: true,
            pre_kill_credit: None,
        })
    }

    /// The point-patrol and attack-rebind branches of
    /// `sub_12500 -> sub_12410 -> sub_12330`.
    /// Follow one link per pre-dispatch pass, including the destination's
    /// binding counter and thinning rule, rather than ending every patrol.
    pub(crate) fn advance_authored_firefly_stage_rows(
        &mut self,
        runtime: &mut RuntimeSnapshot,
    ) -> std::result::Result<(), String> {
        let mut slots = self
            .authored_fireflies
            .iter()
            .map(|state| state.source_entity_index)
            .collect::<Vec<_>>();
        // The source class/model list is rebuilt in numeric entity order.
        // This matters when several arrivals bind a thinned destination.
        slots.sort_unstable();
        for source_slot in slots {
            let Some(source_index) = self.authored_class5.iter().position(|source| {
                source.active
                    && source.drawing_enabled
                    && source.life >= 0
                    && source.source_entity_index == source_slot
                    && source.model == FIREFLY_MODEL
                    && !matches!(source.action & 7, 4 | 5)
            }) else {
                continue;
            };
            let state_index = self
                .authored_fireflies
                .iter()
                .position(|state| state.source_entity_index == source_slot)
                .expect("Firefly source slot came from its runtime row");
            let state = self.authored_fireflies[state_index];
            if matches!(state.stage_var2, 13 | 16 | 17) {
                if !matches!(self.authored_class5[source_index].action & 7, 2 | 6) {
                    self.authored_class5[source_index].action = FIREFLY_STAGE_ACTION;
                }
                continue;
            }
            let rebind = state.stage_var2 == 10;
            if state.stage_var1 == 0
                || !matches!(state.stage_var2, 1 | 10)
                || (rebind && matches!(self.authored_class5[source_index].action & 7, 2 | 6))
            {
                continue;
            }
            let stage = runtime
                .stage_variables
                .get(usize::from(state.stage_var1))
                .copied()
                .ok_or_else(|| format!("Firefly StageVar {} is absent", state.stage_var1))?;
            let RuntimeRelation::AxisScalar(point) = stage.relation else {
                return Err(format!(
                    "Firefly StageVar {} has no patrol point",
                    state.stage_var1
                ));
            };
            if !matches!(stage.kind, 1 | 2) || (!rebind && stage.kind != 1) {
                return Err(format!(
                    "Firefly patrol StageVar {} has kind {}",
                    state.stage_var1, stage.kind
                ));
            }
            let position = self.authored_class5[source_index].position;
            let dx = i32::from((point as u16).wrapping_sub(position.x).cast_signed()).abs();
            let dy = i32::from(
                ((point >> 16) as u16)
                    .wrapping_sub(position.y)
                    .cast_signed(),
            )
            .abs();
            if !rebind && (dx > FIREFLY_STAGE_REACH || dy > FIREFLY_STAGE_REACH) {
                continue;
            }

            let next = if rebind {
                state.stage_var1
            } else {
                stage.word_2 as u8
            };
            let mut route = None;
            if next != 0 {
                let destination = runtime
                    .stage_variables
                    .get(usize::from(next))
                    .copied()
                    .ok_or_else(|| format!("Firefly next StageVar {next} is absent"))?;
                let phase = (destination.word_2 >> 8) as u8 & 3;
                if !runtime.advance_level2_stage_variable_counter(next) {
                    return Err(format!(
                        "Firefly StageVar {next} has no lossless binding counter carrier"
                    ));
                }
                let rejected = match destination.flags as u8 & 0x60 {
                    0x20 => phase == 3,
                    0x40 => phase & 1 != 0,
                    0x60 => phase != 0,
                    _ => false,
                };
                if !rejected {
                    let RuntimeRelation::AxisScalar(point) = destination.relation else {
                        return Err(format!("Firefly next StageVar {next} has no patrol point"));
                    };
                    if !matches!(destination.kind, 1 | 2) {
                        return Err(format!(
                            "Firefly next patrol StageVar {next} has kind {}",
                            destination.kind
                        ));
                    }
                    route = Some((
                        destination.kind as u8,
                        Position {
                            x: point as u16,
                            y: (point >> 16) as u16,
                            z: 0,
                        },
                    ));
                }
            }
            let state = &mut self.authored_fireflies[state_index];
            if let Some((kind, target)) = route {
                state.stage_var1 = next;
                state.stage_var2 = kind;
                state.stage_target = target;
                self.authored_class5[source_index].action = FIREFLY_STAGE_ACTION;
            } else {
                state.stage_var1 = 0;
                state.stage_var2 = 0;
                self.authored_class5[source_index].action = FIREFLY_ROAM_ACTION;
            }
        }
        Ok(())
    }

    pub(crate) fn step_authored_firefly_stage_slot(
        &mut self,
        source_slot: u16,
        players: &[GoatPlayerTarget],
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_STAGE_ACTION
        }) else {
            return false;
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        // sub_1DBF0 tests the signed axis words before choosing its movement
        // body. Kind 2 guards this point; it does not finish on arrival.
        let guarding = state.stage_var2 == 2;
        let at_guard_point = guarding
            && (i32::from(state.stage_target.x.cast_signed())
                - i32::from(source.position.x.cast_signed()))
            .abs()
                <= FIREFLY_GUARD_REACH
            && (i32::from(state.stage_target.y.cast_signed())
                - i32::from(source.position.y.cast_signed()))
            .abs()
                <= FIREFLY_GUARD_REACH;
        if step_firefly_motion(&mut source, &mut state, terrain) {
            self.decrement_source_creature_population();
        }

        if at_guard_point {
            // Firefly descriptor88 has source byte32=1: sub_1E1C0 admits
            // motion, then draws once every sixteen visits when unblocked.
            if source.source_byte_3e & 15 == 0 && !state.movement_blocked {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                source.roll = source.roll.wrapping_add(
                    i16::try_from(source.entity_rng % 113 + 142)
                        .expect("Firefly guard turn fits i16"),
                ) & 0x07ff;
            }
        } else if source.source_byte_3e & 7 == 0 {
            if !state.movement_blocked {
                source.roll = i16::try_from(crate::heading_for_positions(
                    source.position,
                    state.stage_target,
                ))
                .expect("Firefly stage heading fits i16");
                if source.source_byte_3e & 0x3f == 0 {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let direction = source.entity_rng;
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let sign = if direction % 157 < 79 { -1 } else { 1 };
                    let delta = i16::try_from((source.entity_rng & 0x00ff) + 85)
                        .expect("Firefly wander delta fits i16");
                    source.roll = source.roll.wrapping_add(sign * delta) & 0x07ff;
                }
            }
            if let Some(peer) = frozen_model19_slots.iter().find_map(|peer_slot| {
                self.authored_class5.iter().find(|peer| {
                    peer.active
                        && peer.life >= 0
                        && peer.model == FIREFLY_MODEL
                        && peer.source_entity_index == *peer_slot
                        && peer.source_entity_index != source_slot
                        && i32::from(
                            peer.position
                                .x
                                .wrapping_sub(source.position.x)
                                .cast_signed(),
                        )
                        .abs()
                            < FIREFLY_SEPARATION
                        && i32::from(
                            peer.position
                                .y
                                .wrapping_sub(source.position.y)
                                .cast_signed(),
                        )
                        .abs()
                            < FIREFLY_SEPARATION
                })
            }) {
                source.roll =
                    i16::try_from(crate::heading_for_positions(peer.position, source.position))
                        .expect("Firefly separation heading fits i16");
            }
        }

        if guarding {
            acquire_firefly_guard_target(&mut source, &mut state, players);
        }

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(crate) fn step_authored_firefly_roam_slot(
        &mut self,
        source_slot: u16,
        players: &[GoatPlayerTarget],
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_ROAM_ACTION
        }) else {
            return false;
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        if step_firefly_motion(&mut source, &mut state, terrain) {
            self.decrement_source_creature_population();
        }

        if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0 {
            apply_firefly_wander(&mut source);
            if source.source_byte_39 != 0 {
                let range_squared = u32::from(FIREFLY_RANGE).pow(2);
                if let Some(target) = players
                    .iter()
                    .filter(|target| target.control_flags & 0x20 == 0)
                    .filter_map(|target| {
                        let distance =
                            squared_wrapping_xy_distance(source.position, target.position);
                        let heading =
                            crate::heading_for_positions(source.position, target.position);
                        (distance <= range_squared
                            && crate::angle_distance(i32::from(source.yaw), heading)
                                < FIREFLY_VIEW_HALF_ANGLE)
                            .then_some((distance, target))
                    })
                    .min_by_key(|(distance, _)| *distance)
                    .map(|(_, target)| target)
                {
                    source.word_0x96_150 = target.entity_index;
                    source.byte_0x46_70 = 0;
                    state.linked_entity_index = 0;
                    source.action = 154;
                } else if let Some(peer_slot) = nearest_unlinked_firefly(
                    source,
                    frozen_model19_slots,
                    &self.authored_class5,
                    &self.authored_fireflies,
                ) {
                    state.linked_entity_index = peer_slot;
                    source.action = FIREFLY_LINKED_ACTION;
                }
            }
        }

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    pub(crate) fn step_authored_firefly_linked_slot(
        &mut self,
        source_slot: u16,
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> bool {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_LINKED_ACTION
        }) else {
            return false;
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return false;
        };
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        if state.linked_entity_index == 0 {
            source.action = FIREFLY_ROAM_ACTION;
            self.authored_class5[source_index] = source;
            return true;
        }
        let peer = self.authored_class5.iter().copied().find(|peer| {
            peer.source_entity_index == state.linked_entity_index
                && peer.active
                && peer.drawing_enabled
                && peer.life >= 0
                && peer.model == FIREFLY_MODEL
        });
        if step_firefly_motion(&mut source, &mut state, terrain) {
            self.decrement_source_creature_population();
        }
        if let Some(peer) = peer {
            if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0 {
                match peer.action.wrapping_sub(152) {
                    0 | 1 => {}
                    2 => {
                        source.word_0x96_150 = peer.word_0x96_150;
                        source.byte_0x46_70 = 0;
                        state.linked_entity_index = 0;
                        source.action = 154;
                    }
                    3 => {
                        state.linked_entity_index = self
                            .authored_fireflies
                            .iter()
                            .find(|state| state.source_entity_index == peer.source_entity_index)
                            .map_or(0, |state| state.linked_entity_index);
                    }
                    _ => leave_firefly_link(&mut source, &mut state),
                }
                if source.action == FIREFLY_LINKED_ACTION {
                    if let Some(linked) = self.authored_class5.iter().find(|candidate| {
                        candidate.source_entity_index == state.linked_entity_index
                    }) {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            source.position,
                            linked.position,
                        ))
                        .expect("Firefly linked heading fits i16");
                        apply_firefly_separation(
                            &mut source,
                            frozen_model19_slots,
                            &self.authored_class5,
                            FIREFLY_LINKED_SEPARATION,
                        );
                        source.actual_speed = peer.actual_speed.wrapping_add(peer.maximum_speed);
                    } else {
                        leave_firefly_link(&mut source, &mut state);
                    }
                }
            }
        } else {
            leave_firefly_link(&mut source, &mut state);
        }

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Undamaged `HitFirebug_25610` orbit and charge branch for a live
    /// class-5/model-19/action-154 row. Substates 4/5 are the separate
    /// Firefly-fireball producer and stay fail-closed until that projectile's
    /// distinct class-9 lifecycle is represented by the shared arena.
    pub(crate) fn step_authored_firefly_charge_slot(
        &mut self,
        source_slot: u16,
        target: Option<FireflyAttackTarget>,
        frozen_model19_slots: &[u16],
        terrain: &SessionTerrain,
    ) -> std::result::Result<FireflyAttackStep, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_ATTACK_ACTION
        }) else {
            return Ok(FireflyAttackStep::default());
        };
        let Some(state_index) = self
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return Err(format!(
                "Firefly source slot {source_slot} has no runtime sidecar"
            ));
        };

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_fireflies[state_index];
        if step_firefly_motion(&mut source, &mut state, terrain) {
            self.decrement_source_creature_population();
        }

        let Some(target) = target.filter(|target| {
            target.live && target.entity_index != 0 && target.entity_index == source.word_0x96_150
        }) else {
            // Source LABEL_92 resets only action and speed. The target word,
            // attack substate, and Firefly peer word are deliberately left
            // untouched here.
            source.action = FIREFLY_ROAM_ACTION;
            source.actual_speed = FIREFLY_MINIMUM_SPEED;
            let live = self.commit_authored_firefly_step(source_index, state_index, source, state);
            return Ok(FireflyAttackStep {
                live,
                ..FireflyAttackStep::default()
            });
        };

        let mut sound_id = None;
        let mut contact = None;
        let mut fireball = None;
        loop {
            match source.byte_0x46_70 {
                0 => {
                    source.actual_speed = FIREFLY_MINIMUM_SPEED;
                    source.byte_0x46_70 = 1;
                }
                1 => {
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let orbit_yaw = target.yaw.wrapping_sub(256).wrapping_add(
                        i16::try_from((u32::from(source.entity_rng % 90) << 11) / 360)
                            .expect("Firefly orbit delta fits i16"),
                    ) & 0x07ff;
                    let mut orbit_point = target.position;
                    crate::move_entity(&mut orbit_point, orbit_yaw, 0, FIREFLY_ORBIT_RADIUS);
                    if firefly_distance_3d(source.position, orbit_point)
                        <= FIREFLY_ORBIT_REACHED_DISTANCE
                    {
                        source.byte_0x46_70 = 2;
                    } else {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            source.position,
                            orbit_point,
                        ))
                        .expect("Firefly orbit heading fits i16");
                        if source.source_byte_3e & 3 == 0 {
                            apply_firefly_separation(
                                &mut source,
                                frozen_model19_slots,
                                &self.authored_class5,
                                FIREFLY_SEPARATION,
                            );
                        }
                    }
                    break;
                }
                2 => {
                    source.actual_speed = FIREFLY_MAXIMUM_SPEED;
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    source.dword_0x10 =
                        i32::from(target.position.z) + i32::from(source.entity_rng & 0x03ff);
                    source.byte_0x46_70 = 3;
                }
                3 => {
                    source.roll = i16::try_from(crate::heading_for_positions(
                        source.position,
                        target.position,
                    ))
                    .expect("Firefly target heading fits i16");
                    if source.source_byte_3e & 3 != 0 {
                        break;
                    }
                    apply_firefly_separation(
                        &mut source,
                        frozen_model19_slots,
                        &self.authored_class5,
                        FIREFLY_SEPARATION,
                    );
                    let mut ahead = target.position;
                    crate::move_entity(&mut ahead, target.yaw, 0, FIREFLY_ORBIT_RADIUS);
                    if firefly_distance_3d(source.position, ahead) > FIREFLY_ORBIT_REACHED_DISTANCE
                    {
                        source.byte_0x46_70 = 0;
                        break;
                    }
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    let maneuver = source.entity_rng % 287;
                    if maneuver & 0x3f == 0 {
                        source.byte_0x46_70 = 6;
                    }
                    if maneuver & 0x1f == 0 {
                        source.byte_0x46_70 = 7;
                    }
                    if maneuver == 0 {
                        source.byte_0x46_70 = 4;
                    }
                    if maneuver & 3 == 0 {
                        source.position.z = source.position.z.wrapping_add(
                            if i32::from(source.position.z) <= source.dword_0x10 {
                                64
                            } else {
                                -64
                            },
                        );
                    }
                    break;
                }
                4 => {
                    source.actual_speed = FIREFLY_MINIMUM_SPEED;
                    source.byte_0x46_70 = 5;
                }
                5 => {
                    if source.source_byte_3e & 3 == 0 {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            source.position,
                            target.position,
                        ))
                        .expect("Firefly firing heading fits i16");
                        apply_firefly_separation(
                            &mut source,
                            frozen_model19_slots,
                            &self.authored_class5,
                            FIREFLY_SEPARATION,
                        );
                    }
                    if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0 {
                        if firefly_distance_3d(source.position, target.position)
                            >= u32::from(FIREFLY_RANGE)
                        {
                            source.byte_0x46_70 = 6;
                        } else {
                            let angle_origin = source.position;
                            let horizontal = crate::legacy_sqrt(squared_wrapping_xy_distance(
                                angle_origin,
                                target.position,
                            ));
                            let yaw = i16::try_from(crate::heading_for_positions(
                                angle_origin,
                                target.position,
                            ))
                            .expect("Firefly fireball heading fits i16");
                            let pitch = crate::pitch_for_delta(
                                i32::try_from(horizontal)
                                    .expect("Firefly target distance fits i32"),
                                i32::from(target.position.z.wrapping_sub(angle_origin.z)),
                            );
                            let mut position = angle_origin;
                            position.z = position.z.wrapping_add(FIREFLY_SHIFT_Y);
                            fireball = Some(FireflyFireballLaunch {
                                owner_entity_index: source.source_entity_index,
                                target_entity_index: target.entity_index,
                                position,
                                target_position: target.position,
                                yaw,
                                pitch,
                                payload: FIREFLY_FIREBALL_PAYLOAD,
                            });
                        }
                    }
                    break;
                }
                6 => {
                    source.action = FIREFLY_ROAM_ACTION;
                    source.actual_speed = FIREFLY_MINIMUM_SPEED;
                    break;
                }
                7 => {
                    source.actual_speed = FIREFLY_MINIMUM_SPEED.saturating_mul(3);
                    source.entity_rng = level2_next_rng16(source.entity_rng);
                    sound_id = Some(43 + (source.entity_rng & 1));
                    source.dword_0x10 = FIREFLY_CHARGE_DURATION;
                    source.byte_0x46_70 = 8;
                }
                8 | 9 => {
                    source.dword_0x10 = source.dword_0x10.wrapping_sub(1);
                    if source.dword_0x10 == 0 {
                        source.byte_0x46_70 = 0;
                        break;
                    }
                    if source.source_byte_3e & 3 == 0 {
                        if source.dword_0x10 > 16 {
                            source.roll = i16::try_from(crate::heading_for_positions(
                                source.position,
                                target.position,
                            ))
                            .expect("Firefly charge heading fits i16");
                        }
                        apply_firefly_separation(
                            &mut source,
                            frozen_model19_slots,
                            &self.authored_class5,
                            FIREFLY_SEPARATION,
                        );
                    }
                    let altitude_delta = i32::from(target.position.z)
                        .wrapping_sub(i32::from(source.position.z))
                        .clamp(-64, 64);
                    source.position.z = source.position.z.wrapping_add(
                        i16::try_from(altitude_delta)
                            .expect("clamped Firefly altitude delta fits i16"),
                    );
                    if source.byte_0x46_70 == 8
                        && firefly_distance_3d(source.position, target.position)
                            < FIREFLY_CHARGE_CONTACT_RANGE
                    {
                        contact = Some(FireflyContact {
                            target_entity_index: target.entity_index,
                            attacker_entity_index: source.source_entity_index,
                            damage: i32::from(source.subspell_index),
                        });
                        source.byte_0x46_70 = 9;
                    }
                    if source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE == 0
                        && firefly_distance_3d(source.position, target.position)
                            >= u32::from(FIREFLY_RANGE)
                    {
                        source.byte_0x46_70 = 6;
                    }
                    break;
                }
                substate => {
                    return Err(format!(
                        "Firefly source slot {source_slot} reached unsupported attack substate {substate}"
                    ));
                }
            }
        }

        let live = self.commit_authored_firefly_step(source_index, state_index, source, state);
        Ok(FireflyAttackStep {
            live,
            sound_id,
            contact,
            fireball,
        })
    }

    fn commit_authored_firefly_step(
        &mut self,
        source_index: usize,
        state_index: usize,
        source: Level2AuthoredClass5Source,
        state: AuthoredFireflyRuntime,
    ) -> bool {
        self.source_rng
            .insert(source.source_entity_index, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_fireflies[state_index] = state;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source.source_entity_index,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok()
    }

    /// Stage only the geometry and cadence prerequisite for the real-package
    /// action-153 -> action-155 production test. Construction, PreDispatch
    /// arrival, numeric dispatch, callback ordering, and epilogues remain on
    /// the ordinary campaign path; this does not claim natural travel time.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_peer_roam_prerequisite_for_test(
        &mut self,
        roam_slot: u16,
        peer_slot: u16,
    ) -> bool {
        let Some(stage_target) = self
            .authored_fireflies
            .iter()
            .find(|state| state.source_entity_index == roam_slot)
            .map(|state| state.stage_target)
        else {
            return false;
        };
        if !self
            .authored_fireflies
            .iter()
            .any(|state| state.source_entity_index == peer_slot)
        {
            return false;
        }

        for state in &mut self.authored_fireflies {
            state.linked_entity_index = 0;
            state.movement_blocked = false;
        }
        let mut remote_ordinal = 0_u16;
        for source in self
            .authored_class5
            .iter_mut()
            .filter(|source| source.model == FIREFLY_MODEL)
        {
            source.actual_speed = FIREFLY_MINIMUM_SPEED;
            source.yaw = 0;
            source.roll = 0;
            match source.source_entity_index {
                slot if slot == roam_slot => {
                    source.action = FIREFLY_STAGE_ACTION;
                    source.source_byte_3e = 70;
                    source.position = stage_target;
                }
                slot if slot == peer_slot => {
                    source.action = FIREFLY_ROAM_ACTION;
                    source.source_byte_3e = 1;
                    source.position = Position {
                        x: stage_target.x.wrapping_add(2_048),
                        ..stage_target
                    };
                }
                _ => {
                    source.action = FIREFLY_STAGE_ACTION;
                    remote_ordinal = remote_ordinal.wrapping_add(1);
                    source.position = Position {
                        y: stage_target
                            .y
                            .wrapping_add(10_000_u16.wrapping_add(remote_ordinal * 1_000)),
                        ..stage_target
                    };
                }
            }
        }
        let seeds = self
            .authored_class5
            .iter()
            .filter(|source| source.model == FIREFLY_MODEL)
            .map(|source| RenderableMapSeed {
                entity_index: source.source_entity_index,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .collect::<Vec<_>>();
        seeds
            .into_iter()
            .all(|seed| self.map_chains.sync_entity(seed).is_ok())
    }

    /// Stage only elapsed source cadence for the action-155 boundary test.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_phase_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        phase: u8,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_LINKED_ACTION
        }) else {
            return false;
        };
        source.source_byte_3e = phase;
        true
    }

    /// Stage only geometry, cadence, and private RNG for the real-package
    /// action-153 player acquisition -> action-154 charge test. The ordinary
    /// scheduler still owns target acquisition, every attack callback, sound,
    /// contact mailbox routing, and phase epilogues.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_charge_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        player_position: Position,
        player_yaw: i16,
    ) -> bool {
        if !self.authored_class5.iter().any(|source| {
            source.source_entity_index == source_slot && source.model == FIREFLY_MODEL
        }) || !self
            .authored_fireflies
            .iter()
            .any(|state| state.source_entity_index == source_slot)
        {
            return false;
        }

        let mut position = player_position;
        crate::move_entity(&mut position, player_yaw, 0, FIREFLY_ORBIT_RADIUS);
        let toward_player = player_yaw.wrapping_add(1_024) & 0x07ff;
        let mut remote_ordinal = 0_u16;
        let mut seeds = Vec::new();
        for source in self
            .authored_class5
            .iter_mut()
            .filter(|source| source.model == FIREFLY_MODEL)
        {
            if source.source_entity_index == source_slot {
                source.action = FIREFLY_ROAM_ACTION;
                source.actual_speed = FIREFLY_MINIMUM_SPEED;
                source.entity_rng = 739;
                source.source_byte_3e = 70;
                source.source_byte_39 = 1;
                source.word_0x96_150 = 0;
                source.byte_0x46_70 = 0;
                source.dword_0x10 = 0;
                source.position = position;
                source.yaw = toward_player;
                source.roll = toward_player;
            } else {
                remote_ordinal = remote_ordinal.wrapping_add(1);
                source.position = Position {
                    x: player_position
                        .x
                        .wrapping_add(12_000_u16.wrapping_add(remote_ordinal * 1_000)),
                    y: player_position.y.wrapping_add(12_000),
                    z: player_position.z,
                };
            }
            seeds.push(RenderableMapSeed {
                entity_index: source.source_entity_index,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            });
        }
        for state in &mut self.authored_fireflies {
            state.linked_entity_index = 0;
            state.movement_blocked = false;
            if state.source_entity_index == source_slot {
                state.stage_var1 = 0;
                state.stage_var2 = 0;
            }
        }
        self.source_rng.insert(source_slot, 739);
        seeds
            .into_iter()
            .all(|seed| self.map_chains.sync_entity(seed).is_ok())
    }

    /// Stage only the real charging Firefly's geometry for the strict
    /// action-154 contact boundary. The scheduler still owns movement, the
    /// altitude clamp, mailbox publication, player consumption, and phase.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_contact_geometry_for_test(
        &mut self,
        source_slot: u16,
        target_position: Position,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_ATTACK_ACTION
                && source.byte_0x46_70 == 8
        }) else {
            return false;
        };
        source.position = target_position;
        let seed = RenderableMapSeed {
            entity_index: source_slot,
            active: source.active,
            x: source.position.x,
            y: source.position.y,
        };
        self.map_chains.sync_entity(seed).is_ok()
    }

    /// Stage only the geometry, state-5 selector, and 35-tick cadence for a
    /// real package Firefly. The ordinary action-154 callback still moves the
    /// creature, aims, allocates the shared class-9 projectile, updates the
    /// target, and takes the common source epilogue.
    #[cfg(test)]
    pub(crate) fn stage_authored_firefly_fireball_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        target_position: Position,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.source_entity_index == source_slot
                && source.model == FIREFLY_MODEL
                && source.action == FIREFLY_ATTACK_ACTION
                && source.word_0x96_150 != 0
        }) else {
            return false;
        };
        source.byte_0x46_70 = 5;
        source.source_byte_3e = 140;
        source.actual_speed = FIREFLY_MINIMUM_SPEED;
        source.position = Position {
            x: target_position.x.wrapping_add(4_096),
            y: target_position.y,
            z: target_position.z,
        };
        source.yaw = i16::try_from(crate::heading_for_positions(
            source.position,
            target_position,
        ))
        .expect("Firefly firing prerequisite heading fits i16");
        source.roll = source.yaw;
        let seed = RenderableMapSeed {
            entity_index: source_slot,
            active: source.active,
            x: source.position.x,
            y: source.position.y,
        };
        self.map_chains.sync_entity(seed).is_ok()
    }
}

fn acquire_firefly_guard_target(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredFireflyRuntime,
    players: &[GoatPlayerTarget],
) {
    if source.action != FIREFLY_STAGE_ACTION
        || state.stage_var2 != 2
        || source.source_byte_39 == 0
        || source.source_byte_3e % FIREFLY_DESCRIPTOR_CADENCE != 0
    {
        return;
    }
    let range_squared = u32::from(FIREFLY_RANGE).pow(2);
    if let Some(target) = players
        .iter()
        .filter(|target| target.control_flags & 0x20 == 0)
        .filter_map(|target| {
            // sub_1DBF0 promotes x before subtraction, but retains the y
            // difference in an i16 local before its squared-distance test.
            let dx = i32::from(target.position.x) - i32::from(source.position.x);
            let dy = i32::from(
                target
                    .position
                    .y
                    .wrapping_sub(source.position.y)
                    .cast_signed(),
            );
            let distance = dx
                .wrapping_mul(dx)
                .wrapping_add(dy.wrapping_mul(dy))
                .cast_unsigned();
            let heading = crate::heading_for_positions(source.position, target.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < FIREFLY_VIEW_HALF_ANGLE)
                .then_some((distance, target))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, target)| target)
    {
        source.word_0x96_150 = target.entity_index;
        state.stage_var2 = 10;
        source.action = FIREFLY_ATTACK_ACTION;
        // AddFirebug05_13_25D50's same-call epilogue.
        source.byte_0x46_70 = 0;
    }
}

fn leave_firefly_link(source: &mut Level2AuthoredClass5Source, state: &mut AuthoredFireflyRuntime) {
    source.action = FIREFLY_ROAM_ACTION;
    state.linked_entity_index = 0;
}

fn step_firefly_motion(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredFireflyRuntime,
    terrain: &SessionTerrain,
) -> bool {
    let transition = crate::ground_creature_motion::step_ground_creature_motion(
        source.position,
        source.yaw,
        source.roll,
        source.actual_speed,
        terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: source.shift_rot_x,
            half_height: source.shift_rot_y,
            ground_clearance: FIREFLY_GROUND_CLEARANCE,
            descent: FIREFLY_DESCENT,
            turn_limit: FIREFLY_TURN_LIMIT,
            terrain_mask: u32::MAX,
            maximum_tile_slope: FIREFLY_MAX_TILE_SLOPE,
        },
    );
    source.position = transition.position;
    source.yaw = transition.yaw;
    state.movement_blocked = transition.movement_blocked;
    if transition.killed {
        source.life = -1;
    }
    // Descriptor88 has byte32 bit0: a fatal four-candidate blockage also
    // decrements source364D2, independently of the later death callback.
    transition.killed
}

fn apply_firefly_wander(source: &mut Level2AuthoredClass5Source) {
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let direction = source.entity_rng;
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let sign = if direction % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((source.entity_rng & 0x00ff) + 85).expect("Firefly wander delta fits i16");
    source.roll = source.roll.wrapping_add(sign * delta) & 0x07ff;
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

fn firefly_distance_3d(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    let dz = i32::from(right.z.wrapping_sub(left.z));
    crate::legacy_sqrt(
        dx.wrapping_mul(dx)
            .cast_unsigned()
            .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
            .wrapping_add(dz.wrapping_mul(dz).cast_unsigned()),
    )
}

fn nearest_unlinked_firefly(
    source: Level2AuthoredClass5Source,
    frozen_model19_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    fireflies: &[AuthoredFireflyRuntime],
) -> Option<u16> {
    let range_squared = u32::from(FIREFLY_RANGE).pow(2);
    frozen_model19_slots
        .iter()
        .filter_map(|slot| {
            let peer = roots.iter().find(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.source_entity_index == *slot
                    && peer.source_entity_index != source.source_entity_index
                    && fireflies
                        .iter()
                        .find(|state| state.source_entity_index == *slot)
                        .is_some_and(|state| state.linked_entity_index == 0)
            })?;
            let distance = squared_wrapping_xy_distance(source.position, peer.position);
            let heading = crate::heading_for_positions(source.position, peer.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < FIREFLY_VIEW_HALF_ANGLE)
                .then_some((distance, *slot))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, slot)| slot)
}

fn apply_firefly_separation(
    source: &mut Level2AuthoredClass5Source,
    frozen_model19_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    separation: i32,
) {
    if let Some(peer) = frozen_model19_slots.iter().find_map(|peer_slot| {
        roots.iter().find(|peer| {
            peer.active
                && peer.life >= 0
                && peer.source_entity_index == *peer_slot
                && peer.source_entity_index != source.source_entity_index
                && i32::from(
                    peer.position
                        .x
                        .wrapping_sub(source.position.x)
                        .cast_signed(),
                )
                .abs()
                    < separation
                && i32::from(
                    peer.position
                        .y
                        .wrapping_sub(source.position.y)
                        .cast_signed(),
                )
                .abs()
                    < separation
        })
    }) {
        source.roll = i16::try_from(crate::heading_for_positions(peer.position, source.position))
            .expect("Firefly separation heading fits i16");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use std::path::PathBuf;

    #[test]
    fn ordinary_unbound_firefly_constructs_and_runs_source_roam() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut economy = session.economy().clone();
        let terrain = session.canonical_terrain();
        let source = economy
            .allocate_authored_firefly_unbound(
                683,
                session.state.position,
                FIREFLY_LIFE as u32,
                terrain,
            )
            .expect("ordinary Firefly constructor has source capacity");
        let slot = source.source_entity_index;
        let state = economy
            .authored_fireflies
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (source.action, state.stage_var1, state.stage_var2),
            (FIREFLY_ROAM_ACTION, 0, 0)
        );
        assert!(economy.step_authored_firefly_roam_slot(slot, &[], &[], terrain));
        assert!(
            economy
                .authored_class5
                .iter()
                .any(|row| row.active && row.source_entity_index == slot)
        );
    }

    #[test]
    fn parent_stage_prepass_resumes_after_attack_without_interrupting_attack() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut economy = session.economy().clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let source = economy
            .allocate_authored_firefly_unbound(
                683,
                session.state.position,
                FIREFLY_LIFE as u32,
                session.canonical_terrain(),
            )
            .unwrap();
        let slot = source.source_entity_index;
        let state = economy
            .authored_fireflies
            .iter_mut()
            .find(|state| state.source_entity_index == slot)
            .unwrap();
        state.stage_var2 = 13;
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        let root = economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(root.action, FIREFLY_STAGE_ACTION);
        root.action = FIREFLY_ATTACK_ACTION;
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        assert_eq!(
            economy
                .authored_class5
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .action,
            FIREFLY_ATTACK_ACTION
        );
    }

    #[test]
    fn mode16_negative_countdown_expires_on_next_visit() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let particles = &session.session.definition.presentation.particles;
        let parent = session.state.entity_index;
        let handle = allocate_summoned(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            session.state.position,
            parent,
            parent,
            parent,
        )
        .unwrap()
        .unwrap();
        let slot = handle.source_slot;
        let state = arena
            .economy
            .authored_fireflies
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        state.stage_var2 = 16;
        state.word_0x2e = 2;
        let source = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        source.source_byte_3e = 1;
        assert!(
            step_summoned_stage_slot(&mut arena, slot, &[], &[], &[], &mut terrain, particles)
                .unwrap()
        );
        let state = arena
            .economy
            .authored_fireflies
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(state.word_0x2e.cast_signed(), -2);
        assert!(
            arena
                .economy
                .authored_class5
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .drawing_enabled
        );
        assert!(
            step_summoned_stage_slot(&mut arena, slot, &[], &[], &[], &mut terrain, particles)
                .unwrap()
        );
        assert!(
            !arena
                .economy
                .authored_class5
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .drawing_enabled
        );
    }

    #[test]
    fn summoned_firefly_expires_when_parent_slot_is_reused() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let particles = &session.session.definition.presentation.particles;
        let parent = session.state.entity_index;
        let child = allocate_summoned(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            session.state.position,
            parent,
            parent,
            parent,
        )
        .unwrap()
        .unwrap();
        let old_parent = arena.handle(parent).unwrap();
        arena.economy.release_slot(parent, None);
        assert_eq!(
            arena.allocate_source_slot_with_terrain(&mut terrain),
            Some(parent)
        );
        let mut replacement =
            crate::native_world::cave_features::new_event(parent, 5, session.state.position);
        replacement.class = 3;
        replacement.model = 0;
        replacement.record[67] = 3;
        replacement.set_life(1000);
        arena.entities[usize::from(parent)] = replacement;
        assert_ne!(arena.handle(parent), Some(old_parent));
        assert!(
            step_summoned_stage_slot(
                &mut arena,
                child.source_slot,
                &[],
                &[],
                &[],
                &mut terrain,
                particles
            )
            .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == child.source_slot)
            .unwrap();
        assert!(source.active && !source.drawing_enabled);
    }

    #[test]
    fn parent_bound_launch_transitions_and_parent_loss_hides_the_real_firefly() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let particles = &session.session.definition.presentation.particles;
        let parent = session.state.entity_index;
        let handle = allocate_summoned(
            &mut arena,
            &mut runtime,
            &mut terrain,
            particles,
            session.state.position,
            parent,
            parent,
            parent,
        )
        .unwrap()
        .expect("real Firefly constructor has a free source slot");
        let slot = handle.source_slot;
        let source = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        source.word_0x96_150 = parent;
        source.actual_speed = 24;
        source.source_byte_3e = 1;
        let state = arena
            .economy
            .authored_fireflies
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        state.stage_var2 = 17;

        assert!(
            step_summoned_stage_slot(&mut arena, slot, &[], &[], &[], &mut terrain, particles)
                .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        let state = arena
            .economy
            .authored_fireflies
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (state.stage_var2, state.word_0x2e, source.actual_speed),
            (16, 250, 76)
        );

        assert!(
            step_summoned_stage_slot(&mut arena, slot, &[], &[], &[], &mut terrain, particles)
                .unwrap()
        );
        let state = arena
            .economy
            .authored_fireflies
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            state.word_0x2e, 250,
            "mode16 does not consume lifetime while pursuing a target"
        );

        arena.entities[usize::from(parent)].disable_drawing();
        assert!(
            step_summoned_stage_slot(&mut arena, slot, &[], &[], &[], &mut terrain, particles)
                .unwrap()
        );
        let source = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert!(
            source.active && !source.drawing_enabled,
            "hidden row remains for its same-pass tail"
        );
        retire_hidden_summoned_stage_at_source_boundary(&mut arena);
        arena
            .rebuild_source_arena_with_terrain(&mut terrain, &[])
            .unwrap();
        assert!(
            arena.resolve(handle).is_none(),
            "following boundary reclaims the original generation"
        );
    }

    fn patrol_point(runtime: &RuntimeSnapshot, index: u8) -> Position {
        let row = runtime.stage_variables[usize::from(index)];
        assert_eq!(row.kind, 1);
        let RuntimeRelation::AxisScalar(point) = row.relation else {
            panic!("owner patrol row has no axis point");
        };
        Position {
            x: point as u16,
            y: (point >> 16) as u16,
            z: 0,
        }
    }

    fn level_one_patrol() -> (Level2EconomyState, RuntimeSnapshot, u16) {
        let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq8-level1-collision-v5-night")
            },
            PathBuf::from,
        );
        let campaign = CampaignSession::open(package, CampaignSelection::LEVEL1)
            .expect("real Level-1 patrol owner data");
        let session = campaign.simulation();
        let mut economy = session.economy().clone();
        let mut runtime = session.runtime().clone();
        let target = patrol_point(&runtime, 8);
        let source = economy
            .allocate_authored_firefly(
                156,
                target,
                FireflyStageRoute {
                    stage_var1: 8,
                    stage_var2: 1,
                    target,
                },
                FIREFLY_LIFE as u32,
                session.canonical_terrain(),
            )
            .expect("production Firefly constructor");
        // Match the constructor's existing sub_12330 initial binding. The
        // regression below concerns later pre-dispatch arrivals only.
        assert!(runtime.advance_level2_stage_variable_counter(8));
        (economy, runtime, source.source_entity_index)
    }

    #[test]
    fn cave_guard_negative_life_runs_death_tail_and_next_pass_reclaim() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let campaign =
            CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap()).unwrap();
        let mut session = campaign.simulation().clone();
        let source = session
            .session
            .state
            .live
            .economy
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| {
                row.active && row.model == FIREFLY_MODEL && row.action == FIREFLY_STAGE_ACTION
            })
            .expect("actual Cave guard begins in action159");
        let slot = source.source_entity_index;
        // Stage the direct life write produced by fatal movement; the normal
        // scheduler must consume it even though no fresh damage mailbox exists.
        source.life = -1;
        source.word_0x26 = 7;
        source.word_0x24 = 7;
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        let source = session
            .economy()
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (source.action, source.word_0x26, source.word_0x24),
            (156, 0, 0)
        );
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert_eq!(
            session
                .economy()
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .action,
            157
        );
        for _ in 0..8 {
            session.step_outer_turn(crate::PlayerCommand::default(), 0);
            if session
                .economy()
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .is_some_and(|row| !row.drawing_enabled)
            {
                break;
            }
        }
        assert!(
            session
                .economy()
                .authored_class5()
                .iter()
                .any(|row| row.source_entity_index == slot && !row.drawing_enabled)
        );
        assert!(
            session
                .economy()
                .death_bursts()
                .iter()
                .any(|row| row.parent_entity_index == slot)
        );
        session.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !session
                .economy()
                .authored_class5()
                .iter()
                .any(|row| row.source_entity_index == slot && row.active)
        );
        assert!(!session.economy().map_chains().is_linked(slot));
    }

    #[test]
    fn cave_imported_kind_two_firefly_adopts_guards_attacks_and_rebinds() {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level4");
        let campaign = CampaignSession::open(package, CampaignSelection::ordinary(4).unwrap())
            .expect("actual Cave campaign package");
        let session = campaign.simulation();
        let live = &session.session.state.live;
        let feature = *live
            .mission
            .authored_features()
            .iter()
            .find(|feature| feature.index == 683)
            .expect("authored group0 Firefly row683");
        let binding = live
            .mission
            .level2_authored_stage_variable_for(
                feature,
                live.core.runtime.active_stage_variable_count(),
            )
            .unwrap();
        assert_eq!(
            (binding.index, binding.kind),
            (1, 2),
            "first subtype match wins"
        );
        assert_eq!(
            campaign.inspection().presentation().entity_parameters[88][32],
            1
        );
        // The owner reset already constructed and stepped these roots. Its
        // source slots are authoritative; authored placement683 is not a
        // runtime origin field and must not be guessed from moved positions.
        let imported = campaign
            .inspection()
            .entities()
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == FIREFLY_MODEL)
            .collect::<Vec<_>>();
        assert_eq!(imported.len(), 34);
        assert_eq!(session.economy().authored_fireflies.len(), imported.len());
        for raw in &imported {
            let source = session
                .economy()
                .authored_class5
                .iter()
                .find(|source| source.active && source.source_entity_index == raw.index)
                .expect("every imported Firefly has a live typed owner at its original slot");
            assert_eq!(source.authored_index, IMPORTED_FIREFLY_AUTHORED_INDEX);
            assert_eq!(
                (source.position, source.entity_rng, source.life, source.mana),
                (raw.position, raw.entity_rng, raw.life, raw.mana)
            );
            assert_eq!(
                (source.action, source.source_byte_3e, source.actual_speed),
                (raw.action, raw.phase, raw.actual_speed)
            );
            let state = session
                .economy()
                .authored_fireflies
                .iter()
                .find(|state| state.source_entity_index == raw.index)
                .unwrap();
            assert_eq!(
                (state.stage_var1, state.stage_var2),
                (raw.record[76], raw.record[77])
            );
            assert_eq!(
                (state.control_flags, state.linked_entity_index),
                (raw.control_flags, raw.word_32())
            );
            let retained = &session.source_arena().entities[usize::from(raw.index)];
            assert!(
                !retained.active,
                "raw carrier cannot dispatch alongside its typed owner"
            );
            let mut expected_record = raw.record_bytes().to_vec();
            expected_record[2] = 0;
            assert_eq!(retained.record_bytes(), expected_record);
            let identity = session
                .source_arena()
                .creature_runtime_identity(
                    raw.index,
                    session.session.definition.initial_creature_bindings(),
                    session.session.definition.authored_creature_bindings(),
                )
                .unwrap();
            assert_eq!(
                identity.kind,
                crate::level_definition::CreatureKind::Firefly
            );
            assert_eq!(
                identity.origin,
                crate::campaign::source_arena::CreatureRootOrigin::InitialPackage
            );
        }
        assert_eq!(
            session.runtime().stage_variables[1],
            campaign.inspection().runtime().stage_variables[1],
            "adoption cannot replay the source binding counter"
        );
        let mut adopted = session.source_arena().clone();
        let before = adopted.clone();
        adopt_imported_fireflies(&mut adopted, session.runtime()).unwrap();
        assert_eq!(
            adopted, before,
            "adoption is idempotent, without new slots or RNG"
        );
        let mut dispatched = campaign.clone();
        dispatched.step(crate::PlayerCommand::default(), 0);
        assert!(
            dispatched
                .events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            dispatched.events()
        );
        assert!(
            dispatched.events().iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::DynamicArena {
                    source_slot: 132,
                    kind: Level2DynamicArenaKind::AuthoredClass5,
                    ..
                }
            )),
            "ordinary scheduler dispatches the imported semantic Firefly owner"
        );
        let mut economy = session.economy().clone();
        let mut runtime = session.runtime().clone();
        let source_index = economy
            .authored_class5
            .iter()
            .position(|source| source.active && source.source_entity_index == 132)
            .expect("ordinary open must adopt actual reset source132");
        let slot = economy.authored_class5[source_index].source_entity_index;
        let state_index = economy
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == slot)
            .unwrap();
        let point = economy.authored_fireflies[state_index].stage_target;
        assert_eq!((point.x, point.y), (25088, 47872));
        assert_eq!(economy.authored_fireflies[state_index].stage_var2, 2);
        let initial_word = runtime.stage_variables[1].word_2;
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        assert_eq!(runtime.stage_variables[1].word_2, initial_word);
        assert_eq!(
            economy.authored_class5[source_index].action,
            FIREFLY_STAGE_ACTION
        );

        // Inside the guard box: sub_1E1C0's one-draw, sixteen-phase wander,
        // with no kind-1 arrival transition. This is a leaf diagnostic, not
        // an earned Cave entry or a mission-playthrough receipt.
        let terrain = session.canonical_terrain();
        let source = &mut economy.authored_class5[source_index];
        source.position = Position {
            z: terrain.ground_height(point),
            ..point
        };
        source.actual_speed = 0;
        source.roll = 77;
        source.yaw = 77;
        source.source_byte_3e = 16;
        source.entity_rng = 1234;
        assert!(economy.step_authored_firefly_stage_slot(slot, &[], &[], terrain));
        assert!(!economy.authored_fireflies[state_index].movement_blocked);
        let rng = level2_next_rng16(1234);
        assert_eq!(economy.authored_class5[source_index].entity_rng, rng);
        assert_eq!(
            economy.authored_class5[source_index].roll,
            (77 + i16::try_from(rng % 113 + 142).unwrap()) & 0x7ff
        );
        assert_eq!(economy.authored_fireflies[state_index].stage_var2, 2);

        // Outside the guard box, retain sub_1DDA0's point steering instead.
        let source = &mut economy.authored_class5[source_index];
        source.position.x = point.x + 4096;
        source.position.z = terrain.ground_height(source.position);
        source.source_byte_3e = 8;
        source.entity_rng = 1234;
        source.roll = 77;
        assert!(economy.step_authored_firefly_stage_slot(slot, &[], &[], terrain));
        let source = economy.authored_class5[source_index];
        assert_eq!(source.entity_rng, 1234);
        if !economy.authored_fireflies[state_index].movement_blocked {
            assert_eq!(
                source.roll,
                i16::try_from(crate::heading_for_positions(source.position, point)).unwrap()
            );
        }

        let target = GoatPlayerTarget {
            entity_index: session.state.entity_index,
            position: Position {
                x: point.x + 1000,
                ..point
            },
            control_flags: 0,
        };
        let source = &mut economy.authored_class5[source_index];
        source.position = Position {
            z: terrain.ground_height(point),
            ..point
        };
        source.source_byte_3e = 35;
        source.source_byte_39 = 1;
        source.byte_0x46_70 = 9;
        source.roll = i16::try_from(crate::heading_for_positions(
            source.position,
            target.position,
        ))
        .unwrap();
        source.yaw = source.roll;
        let hidden = GoatPlayerTarget {
            entity_index: slot,
            control_flags: 0x20,
            ..target
        };
        assert!(economy.step_authored_firefly_stage_slot(slot, &[hidden, target], &[], terrain));
        let source = economy.authored_class5[source_index];
        assert_eq!(
            (source.action, source.word_0x96_150, source.byte_0x46_70),
            (FIREFLY_ATTACK_ACTION, target.entity_index, 0)
        );
        assert_eq!(economy.authored_fireflies[state_index].stage_var2, 10);
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        assert_eq!(
            runtime.stage_variables[1].word_2, initial_word,
            "active attack is not rebound"
        );

        // The ordinary attack's return to roam admits sub_12500's same-stage
        // rebind, even though this guard row has a zero next link.
        economy.authored_class5[source_index].action = FIREFLY_ROAM_ACTION;
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        assert_eq!(
            economy.authored_class5[source_index].action,
            FIREFLY_STAGE_ACTION
        );
        let state = economy.authored_fireflies[state_index];
        assert_eq!(
            (state.stage_var1, state.stage_var2, state.stage_target),
            (1, 2, point)
        );
        assert_eq!(runtime.stage_variables[1].word_2 as u8, 0);
        assert_eq!(
            (runtime.stage_variables[1].word_2 >> 8) as u8,
            ((initial_word >> 8) as u8).wrapping_add(1)
        );
        let offset = 392 + 12 + 2;
        assert_eq!(
            runtime.bytes()[offset..offset + 2],
            runtime.stage_variables[1].word_2.to_le_bytes()
        );

        let peer = economy
            .authored_class5
            .iter()
            .find(|peer| {
                peer.active && peer.model == FIREFLY_MODEL && peer.source_entity_index != slot
            })
            .expect("other source-authored Cave Firefly")
            .source_entity_index;
        let position = economy.authored_class5[source_index].position;
        assert!(economy.submit_authored_firefly_damage(slot, 1, peer));
        assert!(
            economy
                .step_authored_firefly_lifecycle_slot(slot, &[], terrain)
                .unwrap()
                .handled
        );
        assert_eq!(
            economy.authored_class5[source_index].action,
            FIREFLY_STAGE_ACTION
        );
        assert_eq!(economy.authored_fireflies[state_index].stage_var2, 2);
        assert_eq!(
            economy.authored_class5[source_index].position, position,
            "mailbox visit does not move"
        );
        // A same-family hit still admits the cadence-gated source player
        // scan. It selects the player, rather than retaliating at its peer.
        assert!(economy.submit_authored_firefly_damage(slot, 1, peer));
        assert!(
            economy
                .step_authored_firefly_lifecycle_slot(slot, &[target], terrain)
                .unwrap()
                .handled
        );
        assert_eq!(
            economy.authored_class5[source_index].word_0x96_150,
            target.entity_index
        );
        assert_eq!(
            economy.authored_class5[source_index].action,
            FIREFLY_ATTACK_ACTION
        );
        assert_eq!(economy.authored_fireflies[state_index].stage_var2, 10);
        assert_eq!(economy.authored_class5[source_index].position, position);
    }

    #[test]
    fn level_one_firefly_patrol_follows_owner_cycle_and_updates_raw_binding_counters() {
        let (mut economy, mut runtime, slot) = level_one_patrol();
        let source_index = economy
            .authored_class5
            .iter()
            .position(|source| source.source_entity_index == slot)
            .unwrap();
        let state_index = economy
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == slot)
            .unwrap();

        let first = patrol_point(&runtime, 8);
        economy.authored_class5[source_index].position.x = first.x.wrapping_add(2_049);
        let before = runtime.clone();
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        assert_eq!(runtime, before, "outside the inclusive arrival box");
        assert_eq!(economy.authored_fireflies[state_index].stage_var1, 8);

        for (current, next) in [(8, 9), (9, 10), (10, 8), (8, 9)] {
            assert_eq!(
                runtime.stage_variables[usize::from(current)].word_2 as u8,
                next
            );
            let point = patrol_point(&runtime, current);
            economy.authored_class5[source_index].position = Position {
                x: point.x.wrapping_add(2_048),
                y: point.y.wrapping_sub(2_048),
                z: 0,
            };
            let rng = economy.authored_class5[source_index].entity_rng;
            let old_word = runtime.stage_variables[usize::from(next)].word_2;
            let mut expected = runtime.stage_variables.clone();
            expected[usize::from(next)].word_2 =
                (old_word & 0xff) | (u16::from(((old_word >> 8) as u8).wrapping_add(1)) << 8);
            economy
                .advance_authored_firefly_stage_rows(&mut runtime)
                .unwrap();

            let state = economy.authored_fireflies[state_index];
            assert_eq!((state.stage_var1, state.stage_var2), (next, 1));
            assert_eq!(state.stage_target, patrol_point(&runtime, next));
            assert_eq!(
                economy.authored_class5[source_index].action,
                FIREFLY_STAGE_ACTION
            );
            assert_eq!(economy.authored_class5[source_index].entity_rng, rng);
            assert_eq!(runtime.stage_variables, expected);
            let offset = 392 + usize::from(next) * 12 + 2;
            assert_eq!(
                runtime.bytes()[offset..offset + 2],
                expected[usize::from(next)].word_2.to_le_bytes()
            );
        }

        // sub_12500 skips death actions even when their stage bytes survive.
        for action in [FIREFLY_PRE_KILL_ACTION, FIREFLY_KILL_ACTION] {
            economy.authored_class5[source_index].action = action;
            economy.authored_class5[source_index].position = patrol_point(&runtime, 9);
            let state = economy.authored_fireflies[state_index];
            let before = runtime.clone();
            economy
                .advance_authored_firefly_stage_rows(&mut runtime)
                .unwrap();
            assert_eq!(economy.authored_fireflies[state_index], state);
            assert_eq!(runtime, before);
            assert_eq!(economy.authored_class5[source_index].action, action);
        }
    }

    #[test]
    fn firefly_patrol_binding_thins_after_counter_increment_and_zero_link_roams() {
        let (economy, runtime, slot) = level_one_patrol();
        let source_index = economy
            .authored_class5
            .iter()
            .position(|source| source.source_entity_index == slot)
            .unwrap();
        let state_index = economy
            .authored_fireflies
            .iter()
            .position(|state| state.source_entity_index == slot)
            .unwrap();
        for flags in [0, 0x20, 0x40, 0x60] {
            for counter in [0_u8, 1, 2, 3, 255] {
                let mut economy = economy.clone();
                let mut runtime = runtime.clone();
                economy.authored_class5[source_index].position = patrol_point(&runtime, 8);
                // Source sub_12500 also processes a retained patrol binding
                // during ordinary attack actions, before action dispatch.
                economy.authored_class5[source_index].action = FIREFLY_ATTACK_ACTION;
                runtime.stage_variables[9].flags = flags as i8;
                runtime.stage_variables[9].word_2 =
                    (runtime.stage_variables[9].word_2 & 0xff) | (u16::from(counter) << 8);
                let offset = 392 + 9 * 12;
                runtime.bytes[offset + 1] = flags;
                let word = runtime.stage_variables[9].word_2;
                runtime.bytes[offset + 2..offset + 4].copy_from_slice(&word.to_le_bytes());
                economy
                    .advance_authored_firefly_stage_rows(&mut runtime)
                    .unwrap();

                let rejected = match flags {
                    0x20 => counter & 3 == 3,
                    0x40 => counter & 1 != 0,
                    0x60 => counter & 3 != 0,
                    _ => false,
                };
                let state = economy.authored_fireflies[state_index];
                assert_eq!(
                    (
                        state.stage_var1,
                        state.stage_var2,
                        economy.authored_class5[source_index].action
                    ),
                    if rejected {
                        (0, 0, FIREFLY_ROAM_ACTION)
                    } else {
                        (9, 1, FIREFLY_STAGE_ACTION)
                    },
                    "flags={flags:#x}, counter={counter}"
                );
                assert_eq!(
                    (runtime.stage_variables[9].word_2 >> 8) as u8,
                    counter.wrapping_add(1)
                );
                assert_eq!(runtime.bytes[offset + 3], counter.wrapping_add(1));
                assert_eq!(runtime.stage_variables[9].word_2 as u8, 10);
            }
        }

        let mut economy = economy;
        let mut runtime = runtime;
        economy.authored_class5[source_index].position = patrol_point(&runtime, 8);
        runtime.stage_variables[8].word_2 &= 0xff00;
        let word = runtime.stage_variables[8].word_2;
        runtime.bytes[392 + 8 * 12 + 2..392 + 8 * 12 + 4].copy_from_slice(&word.to_le_bytes());
        let before = runtime.clone();
        economy
            .advance_authored_firefly_stage_rows(&mut runtime)
            .unwrap();
        assert_eq!(runtime, before, "zero next link binds no destination");
        let state = economy.authored_fireflies[state_index];
        assert_eq!((state.stage_var1, state.stage_var2), (0, 0));
        assert_eq!(
            economy.authored_class5[source_index].action,
            FIREFLY_ROAM_ACTION
        );
    }
}
