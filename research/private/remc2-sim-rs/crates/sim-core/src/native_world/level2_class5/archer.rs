//! Reachable Level 2 class-5/model-4 Archer family.
//!
//! The pinned source calls this family `AddArchers_4BA10` and dispatches its
//! live root through actions 33, 34, and 35.  Action 34's `sub_1CCE0`
//! callback allocates a class-9/model-13 child.  The enclosing economy owns
//! the shared source arena, hash, reset, and dispatch carrier; this file owns
//! the source-exact action packet and typed constructor helpers used by it.

use crate::campaign::source_arena::SourceArena;
use crate::campaign::source_arena::{
    CreatureRootOrigin, CreatureRuntimeIdentity, CreatureRuntimeRole, SourceEntityHandle,
};
use crate::combat::actors::{DamageMailboxMut, DamageWriter, awards_creature_kill, inherit_killer};
use crate::level_definition::CreatureKind;
use crate::level1_mission::HouseResidentRequest;
pub(crate) use crate::native_world::creatures::archer::*;
use crate::native_world::{
    ENTITY_DRAW_DISABLED_BIT, EntitySnapshot, Level2AuthoredClass5ArcherChild,
    Level2AuthoredClass5ArcherState, Level2AuthoredClass5Source, Level2EconomyState,
    ParticleParameter, RuntimeRelation, RuntimeSnapshot, SessionTerrain, SourceAudioCallTag,
    level2_dynamic_economy, level2_next_rng16, mark_source_audio_call, source_map_search_slots,
    source_search_offsets,
};
use crate::{FlightWorld, Position, renderable_map_chain::RenderableMapSeed};

impl ArcherStageRuntime for RuntimeSnapshot {
    fn descriptor(&self, index: u8) -> Option<ArcherStageDescriptor> {
        let row = self
            .stage_variables
            .get(usize::from(index))
            .filter(|_| index != 0)?;
        let scalar = match row.relation {
            RuntimeRelation::AxisScalar(value) => value,
            _ => 0,
        };
        let actor = match row.relation {
            RuntimeRelation::LiveEntity(slot) | RuntimeRelation::LiveEntityIndex(slot) => slot,
            _ => 0,
        };
        Some(ArcherStageDescriptor {
            kind: row.kind as u8,
            flags: row.flags as u8,
            next: row.word_2 as u8,
            point: Position {
                x: scalar as u16,
                y: (scalar >> 16) as u16,
                z: 0,
            },
            actor,
            model: scalar as u8,
            scalar: scalar as u16,
        })
    }

    fn bind(&mut self, index: u8) -> Option<ArcherStageDescriptor> {
        let descriptor = self.descriptor(index)?;
        let phase = (self.stage_variables[usize::from(index)].word_2 >> 8) as u8 & 3;
        if !self.advance_level2_stage_variable_counter(index) {
            return None;
        }
        let reject = match descriptor.flags & 0x60 {
            0x20 => phase == 3,
            0x40 => phase & 1 != 0,
            0x60 => phase != 0,
            _ => false,
        };
        (!reject).then_some(descriptor)
    }

    fn rearm_model_relations(&mut self) {
        for (index, row) in self.stage_variables.iter_mut().enumerate().skip(1) {
            if matches!(row.kind, 3 | 4 | 5 | 8 | 9) && row.flags as u8 & 6 == 6 {
                row.flags = (row.flags as u8 & !4) as i8;
                self.bytes[392 + index * 12 + 1] = row.flags as u8;
            }
        }
    }
}

/// Project the live typed Archer into the fields read by the Alliance impact.
/// This value is temporary; the typed source and stage remain authoritative.
pub(crate) fn alliance_projection(arena: &SourceArena, slot: u16) -> Option<EntitySnapshot> {
    let handle = arena.handle(slot)?;
    if !matches!(arena.resolve(handle), Some(crate::campaign::source_arena::SourceEntityView::Dynamic(
        crate::native_world::Level2DynamicArenaKind::AuthoredClass5
    ))) {
        return None;
    }
    let source = arena.authored_class5().iter().find(|row| {
        row.active && row.source_entity_index == slot && row.model == ARCHER_MODEL
    })?;
    let state = arena.authored_class5_archer().iter().find(|row| {
        row.source_entity_index == slot && row.slot_generation == handle.generation
    })?;
    let mut row = arena.entities.get(usize::from(slot))?.clone();
    row.active = true;
    row.record[2] = 1;
    row.source_id = state.entity_id;
    row.record[30..32].copy_from_slice(&state.entity_id.to_le_bytes());
    row.class = 5;
    row.model = ARCHER_MODEL;
    row.record[67] = 5;
    row.record[68] = ARCHER_MODEL;
    row.set_source_action(source.action);
    row.set_position(source.position);
    row.set_life(source.life);
    row.set_control_flags((state.control_flags & !0x400) | if source.drawing_enabled { 0 } else { 0x400 });
    row.record[44..46].copy_from_slice(&state.stage.parent.to_le_bytes());
    row.record[50..52].copy_from_slice(&state.stage.remaining.to_le_bytes());
    row.record[52..54].copy_from_slice(&state.stage.duration.to_le_bytes());
    row.record[76] = state.stage.index;
    row.record[77] = state.stage.mode;
    row.record[78..80].copy_from_slice(&state.stage.relation.to_le_bytes());
    row.record[154..156].copy_from_slice(&source.word_0x96_150.to_le_bytes());
    Some(row)
}

/// Commit a successful source impact only to the exact projected incarnation.
pub(crate) fn apply_alliance_takeover(
    arena: &mut SourceArena,
    child: SourceEntityHandle,
    row: &EntitySnapshot,
    parent: Option<SourceEntityHandle>,
) -> Result<bool, String> {
    let slot = child.source_slot;
    if row.index != slot || (row.class, row.model) != (5, ARCHER_MODEL)
        || row.record[76] != 0 || row.record[77] != 14
        || arena.handle(slot) != Some(child)
        || alliance_projection(arena, slot).is_none_or(|current| current.source_id != row.source_id)
    {
        return Ok(false);
    }
    let parent_slot = u16::from_le_bytes([row.record[44], row.record[45]]);
    let bound_parent = parent.filter(|handle| handle.source_slot == parent_slot)
        .or_else(|| (parent_slot != 0).then(|| SourceEntityHandle {
            source_slot: parent_slot, generation: arena.slot_generation(parent_slot),
        }));
    if let Some(parent) = bound_parent {
        if !arena.bind_carried_creature_parent(child, parent) {
            return Err(format!("Alliance Archer {slot} lost its parent binding"));
        }
    }
    let source = arena.economy.authored_class5.iter_mut().find(|source| {
        source.active && source.source_entity_index == slot && source.model == ARCHER_MODEL
    }).ok_or_else(|| format!("Alliance Archer {slot} lost its typed source"))?;
    source.action = row.action;
    source.word_0x96_150 = row.source_word_96();
    let state = arena.economy.authored_class5_archer.iter_mut().find(|state| {
        state.source_entity_index == slot && state.slot_generation == child.generation
    }).ok_or_else(|| format!("Alliance Archer {slot} lost its stage"))?;
    state.stage.parent = parent_slot;
    state.stage.remaining = i16::from_le_bytes([row.record[50], row.record[51]]);
    state.stage.duration = i16::from_le_bytes([row.record[52], row.record[53]]);
    state.stage.index = row.record[76];
    state.stage.mode = row.record[77];
    state.stage.relation = u16::from_le_bytes([row.record[78], row.record[79]]);
    Ok(true)
}

/// Resolve live dynamic occupants before the immutable reset row at the same
/// slot. This is also the target view used by the ordinary Archer callbacks.
pub(crate) fn target_in_arena(arena: &SourceArena, slot: u16) -> Option<ArcherTarget> {
    target_in_arena_with_dynamic_slots(arena, slot, None)
}

fn target_in_arena_with_dynamic_slots(
    arena: &SourceArena,
    slot: u16,
    dynamic_slots: Option<&std::collections::BTreeSet<u16>>,
) -> Option<ArcherTarget> {
    if slot == 0 {
        return None;
    }
    if let Some(source) = arena
        .castle_defenders()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(ArcherTarget {
            entity_index: slot,
            class: 5,
            model: 15,
            action: source.action,
            active: source.active,
            life: source.life,
            control_flags: source.control_flags,
            minimum_speed: source.minimum_speed,
            maximum_speed: source.maximum_speed,
            actual_speed: source.speed,
            dword_0x10: source.source_dword_10,
            word_0x248: 0,
            word_0x32: source.chain_previous_slot,
            word_0x96_150: source.target_sphere_slot,
            position: source.position,
        });
    }
    if let Some(source) = arena
        .authored_class5()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let state = arena
            .authored_class5_archer()
            .iter()
            .find(|row| row.source_entity_index == slot);
        let firefly = arena
            .authored_fireflies()
            .iter()
            .find(|row| row.source_entity_index == slot);
        return Some(ArcherTarget {
            entity_index: slot,
            class: 5,
            model: source.model,
            action: source.action,
            active: source.active,
            life: source.life,
            control_flags: state.map(|row| row.control_flags)
                .or_else(|| firefly.map(|row| row.control_flags))
                .unwrap_or(0)
                | if source.drawing_enabled {
                    0
                } else {
                    ENTITY_DRAW_DISABLED_BIT
                },
            minimum_speed: source.minimum_speed,
            maximum_speed: source.maximum_speed,
            actual_speed: source.actual_speed,
            dword_0x10: source.dword_0x10,
            word_0x248: 0,
            word_0x32: state.map_or(0, |row| row.word_0x32),
            word_0x96_150: source.word_0x96_150,
            position: source.position,
        });
    }
    if let Some(source) = arena
        .authored_model45()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(ArcherTarget {
            entity_index: slot,
            class: source.source_class,
            model: source.model,
            action: source.action,
            active: source.active,
            life: source.life,
            control_flags: source.control_flags,
            minimum_speed: source.minimum_speed,
            maximum_speed: 0,
            actual_speed: source.actual_speed,
            dword_0x10: source.dword_0x10,
            word_0x248: 0,
            word_0x32: 0,
            word_0x96_150: 0,
            position: source.position,
        });
    }
    if let Some(source) = arena
        .bee_creatures()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        return Some(ArcherTarget {
            entity_index: slot,
            class: source.source_class,
            model: source.model,
            action: source.action,
            active: source.active,
            life: i32::from(source.life),
            control_flags: (source.control_flags & !ENTITY_DRAW_DISABLED_BIT)
                | if source.drawing_enabled { 0 } else { ENTITY_DRAW_DISABLED_BIT },
            minimum_speed: source.minimum_speed,
            maximum_speed: source.maximum_speed,
            actual_speed: source.actual_speed,
            dword_0x10: source.dword_0x10,
            word_0x248: 0,
            word_0x32: 0,
            word_0x96_150: source.target_entity_index.unwrap_or(0),
            position: source.position,
        });
    }
    // An imported slot that has been replaced must never shadow its current
    // dynamic occupant, even if that occupant is not a valid Archer target.
    let replaced = dynamic_slots.map_or_else(
        || {
            arena
                .economy
                .live_arena_slots()
                .iter()
                .any(|(live, _)| *live == slot)
        },
        |slots| slots.contains(&slot),
    );
    if replaced {
        return None;
    }
    let entity = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| row.active)?;
    Some(ArcherTarget {
        entity_index: slot,
        class: entity.class,
        model: entity.model,
        action: entity.action,
        active: entity.active,
        life: entity.life,
        control_flags: entity.control_flags,
        minimum_speed: entity.minimum_speed(),
        maximum_speed: i16::from_le_bytes([entity.record_bytes()[138], entity.record_bytes()[139]]),
        actual_speed: entity.actual_speed,
        dword_0x10: entity.source_dword_10(),
        word_0x248: 0,
        word_0x32: entity.word_32(),
        word_0x96_150: entity.source_word_96(),
        position: entity.position,
    })
}

pub(crate) fn relations_in_arena(arena: &SourceArena) -> Vec<ArcherRelation> {
    // Only slot membership is reused within this callback. Every pose,
    // mailbox and relation below is still read from its current live carrier.
    let dynamic_slots = arena
        .economy
        .live_arena_slots()
        .into_iter()
        .map(|(slot, _)| slot)
        .collect::<std::collections::BTreeSet<_>>();
    let mut slots = arena
        .entities
        .iter()
        .filter(|row| row.active)
        .map(|row| row.index)
        .collect::<std::collections::BTreeSet<_>>();
    slots.extend(dynamic_slots.iter().copied());
    slots
        .into_iter()
        .filter_map(|slot| {
            let target = target_in_arena_with_dynamic_slots(arena, slot, Some(&dynamic_slots))?;
            if let Some(source) = arena
                .castle_defenders()
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                return Some(ArcherRelation {
                    slot,
                    target,
                    source_id: source.id_entity_index,
                    parent: source.stage.parent,
                    stage_index: source.stage.index,
                    stage_mode: source.stage.mode,
                    stage_relation: source.stage.relation,
                    attacker: source.damage_attacker_entity_index,
                    yaw: source.yaw as i16,
                    roll: source.roll as i16,
                    half_width: source.shift_rot_x,
                    frozen_class5_member: false,
                });
            }
            // Bee and Firefly actors keep their live Alliance fields in
            // family state, not the retained reset row at this address.
            // sub_1E9C0 consults these fields before attacking a sibling.
            if let Some(source) = arena.bee_creatures().iter()
                .find(|row| row.active && row.source_entity_index == slot)
            {
                return Some(ArcherRelation {
                    slot,
                    target,
                    source_id: source.entity_id,
                    parent: source.parent_entity_index,
                    stage_index: source.stage_var1,
                    stage_mode: source.stage_var2,
                    stage_relation: source.stage_relation,
                    attacker: source.damage_attacker_entity_index,
                    yaw: source.yaw,
                    roll: source.roll,
                    half_width: source.shift_rot_x as i16,
                    frozen_class5_member: false,
                });
            }
            if let Some(source) = arena
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
            {
                if let Some(state) = arena.authored_fireflies().iter()
                    .find(|row| row.source_entity_index == slot)
                {
                    return Some(ArcherRelation {
                        slot,
                        target,
                        source_id: state.entity_id,
                        parent: state.parent_entity_index,
                        stage_index: state.stage_var1,
                        stage_mode: state.stage_var2,
                        stage_relation: state.stage_relation,
                        attacker: source.word_0x26,
                        yaw: source.yaw,
                        roll: source.roll,
                        half_width: source.shift_rot_x,
                        frozen_class5_member: false,
                    });
                }
                let state = arena
                    .authored_class5_archer()
                    .iter()
                    .find(|row| row.source_entity_index == slot);
                let stage = state.map_or(ArcherStageState::default(), |row| row.stage);
                return Some(ArcherRelation {
                    slot,
                    target,
                    source_id: state.map_or(slot, |row| row.entity_id),
                    parent: stage.parent,
                    stage_index: stage.index,
                    stage_mode: stage.mode,
                    stage_relation: stage.relation,
                    attacker: source.word_0x26,
                    yaw: source.yaw,
                    roll: source.roll,
                    half_width: source.shift_rot_x,
                    frozen_class5_member: false,
                });
            }
            let raw = arena.entities.get(usize::from(slot))?;
            let record = raw.record_bytes();
            Some(ArcherRelation {
                slot,
                target,
                source_id: raw.source_id,
                parent: u16::from_le_bytes([record[44], record[45]]),
                stage_index: record[76],
                stage_mode: record[77],
                stage_relation: u16::from_le_bytes([record[78], record[79]]),
                attacker: raw.word_26(),
                yaw: raw.yaw,
                roll: raw.source_roll(),
                half_width: raw.array_pitch(),
                frozen_class5_member: false,
            })
        })
        .collect()
}

/// Transfer reset-imported Archer records into the same family carrier used
/// by authored/House constructors. No allocation, constructor, RNG draw or
/// incarnation change occurs. The imported row becomes retained input only.
fn adopt_imported_archers(arena: &mut SourceArena) {
    let raw = arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 5 && row.model == ARCHER_MODEL)
        .filter(|row| {
            !arena
                .authored_class5()
                .iter()
                .any(|root| root.source_entity_index == row.index)
        })
        .cloned()
        .collect::<Vec<_>>();
    for row in raw {
        let record = row.record_bytes();
        let word = |offset| u16::from_le_bytes([record[offset], record[offset + 1]]);
        let source = Level2AuthoredClass5Source {
            source_entity_index: row.index,
            entity_rng: row.entity_rng,
            authored_index: u16::MAX,
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
            word_0x24: word(40),
            byte_0x3b: row.byte_3b,
            word_0x96_150: row.source_word_96(),
            byte_0x46_70: row.level_gate as u8,
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
        };
        let state = Level2AuthoredClass5ArcherState {
            source_entity_index: row.index,
            entity_id: row.source_id,
            origin: ArcherOrigin::InitialPackage,
            slot_generation: arena.slot_generation(row.index),
            word_0x32: row.word_32(),
            word_0x34: word(56),
            descriptor_index: row.parameter_relation.unwrap_or(ARCHER_DESCRIPTOR_INDEX),
            byte_0x38: record[60],
            xtype: record[69],
            xsubtype: record[70],
            pending_damage: i32::from_le_bytes(
                record[98..102]
                    .try_into()
                    .expect("validated source damage dword"),
            ),
            last_child_root_phase: 0,
            control_flags: row.control_flags,
            proximity_delay: record[62],
            stage: ArcherStageState {
                index: record[76],
                mode: record[77],
                relation: word(78),
                parent: word(44),
                remaining: word(50) as i16,
                duration: word(52) as i16,
            },
        };
        arena.economy.authored_class5.push(source);
        arena.economy.authored_class5_archer.push(state);
        // Keep the map/free-stack untouched. All mutable reads now resolve to
        // the owned typed row before this old package record.
        let old = &mut arena.entities[usize::from(row.index)];
        old.active = false;
        old.record[2] = 0;
    }
}

pub(crate) fn creature_runtime_identity(
    arena: &SourceArena,
    slot: u16,
    initial: &std::collections::BTreeMap<u16, CreatureKind>,
    authored: &std::collections::BTreeMap<u16, CreatureKind>,
    include_retained: bool,
) -> Option<CreatureRuntimeIdentity> {
    let source = arena
        .authored_class5()
        .iter()
        .find(|row| row.source_entity_index == slot && (include_retained || row.active))?;
    let state = arena
        .authored_class5_archer()
        .iter()
        .find(|row| row.source_entity_index == slot)?;
    let origin = match state.origin {
        ArcherOrigin::InitialPackage => {
            if initial.get(&slot) != Some(&CreatureKind::Archer) {
                return None;
            }
            CreatureRootOrigin::InitialPackage
        }
        ArcherOrigin::AuthoredRecord(record) => {
            if authored.get(&record) != Some(&CreatureKind::Archer) {
                return None;
            }
            CreatureRootOrigin::AuthoredRecord {
                authored_record: record,
            }
        }
        ArcherOrigin::HouseSpawn(house_slot) => CreatureRootOrigin::HouseSpawn { house_slot },
    };
    Some(CreatureRuntimeIdentity {
        handle: SourceEntityHandle {
            source_slot: slot,
            generation: state.slot_generation,
        },
        kind: CreatureKind::Archer,
        root_slot: slot,
        role: CreatureRuntimeRole::Root,
        origin,
        raw_model: Some(ARCHER_MODEL),
        living: source.active && source.drawing_enabled && source.life >= 0,
    })
}

pub(crate) fn prepare_archer_source_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    frozen: &[Vec<u16>; 29],
) {
    adopt_imported_archers(arena);
    let player = arena
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .map(|row| row.position);
    let targets = relations_in_arena(arena)
        .into_iter()
        .map(|row| row.target)
        .collect::<Vec<_>>();
    for slot in &frozen[usize::from(ARCHER_MODEL)] {
        let Some(source) = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == *slot)
        else {
            continue;
        };
        let Some(state) = arena
            .economy
            .authored_class5_archer
            .iter_mut()
            .find(|row| row.source_entity_index == *slot)
        else {
            continue;
        };
        prepass_archer_stage(source, &mut state.stage, runtime, &targets);
        if let Some(player) = player {
            advance_archer_proximity(source, state, player);
        }
    }
}

pub(crate) fn bind_authored_archer_route(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    index: u8,
) {
    let Some(source) = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
    else {
        return;
    };
    let Some(state) = arena
        .economy
        .authored_class5_archer
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
    else {
        return;
    };
    bind_archer_stage(source, &mut state.stage, index, runtime);
}

pub(crate) fn apply_target_in_arena(arena: &mut SourceArena, target: ArcherTarget) {
    if let Some(source) = arena
        .economy
        .authored_model45
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == target.entity_index)
    {
        source.dword_0x10 = target.dword_0x10;
    } else if let Some(source) = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == target.entity_index)
    {
        source.dword_0x10 = target.dword_0x10;
    } else if let Some(source) = arena
        .entities
        .get_mut(usize::from(target.entity_index))
        .filter(|row| row.active)
    {
        source.set_source_dword_10(target.dword_0x10);
    }
}

pub(crate) fn player_target_ticks(arena: &SourceArena, slot: u16) -> u16 {
    arena
        .archer_player_feedback
        .get(&slot)
        .map_or(0, |feedback| feedback.target_ticks)
}

/// Alive, unpaused class-3/model-0 action 0 upkeep, after its flight body.
/// Keep this at the player's actual source visit: an earlier Archer's write
/// decays now, while a later Archer's write remains 200 until the next visit.
pub(crate) fn advance_player_target_ticks(arena: &mut SourceArena, player: u16) {
    if let Some(feedback) = arena.archer_player_feedback.get_mut(&player) {
        feedback.target_ticks = feedback.target_ticks.saturating_sub(1);
    }
}

pub(crate) fn apply_player_effects(
    arena: &mut SourceArena,
    player: u16,
    effects: &ArcherActionEffects,
) {
    if effects.player_alert_entity == Some(player) {
        arena
            .archer_player_feedback
            .entry(player)
            .or_default()
            .target_ticks = 200;
    }
}

/// Compose the narrow Archer kernel with its actual arena, shared StageVars,
/// frozen target memberships and source effect constructors. Shared roots
/// supply domains and consume the returned player/experience effects.
#[allow(clippy::too_many_arguments)]
pub(crate) fn step_source_runtime(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    frozen_models: &[Vec<u16>; 29],
    frozen_players: &[u16],
    player_alerts: &[(u16, u16)],
) -> Result<ArcherStepResult, String> {
    let source = arena
        .authored_class5()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
        .copied()
        .ok_or_else(|| format!("Archer source slot {slot} is absent"))?;
    let source_generation = arena.slot_generation(slot);
    let state = arena
        .authored_class5_archer()
        .iter()
        .find(|row| row.source_entity_index == slot)
        .copied()
        .ok_or_else(|| format!("Archer slot {slot} has no owned state"))?;
    let mut relations = relations_in_arena(arena);
    // Stage 14 must resolve the caster observed at takeover, not a later
    // occupant of the same slot. Preserve unbound pre-existing source rows.
    if state.stage.mode == 14
        && let Some(child) = arena.handle(slot)
        && let Some(parent) = arena.creature_parent_binding(child)
        && (parent.source_slot != state.stage.parent || arena.resolve(parent).is_none())
    {
        relations.retain(|row| row.slot != state.stage.parent);
    }
    for row in &mut relations {
        row.frozen_class5_member = row.target.class == 5
            && frozen_models
                .get(usize::from(row.target.model))
                .is_some_and(|slots| slots.contains(&row.slot));
        if let Some((_, alert)) = player_alerts.iter().find(|(player, _)| *player == row.slot) {
            row.target.word_0x248 = *alert;
        }
    }
    let resolve = |slot| {
        relations
            .iter()
            .find(|row| row.slot == slot)
            .map(|row| row.target)
    };
    let players = frozen_players
        .iter()
        .filter_map(|slot| resolve(*slot))
        .collect::<Vec<_>>();
    let skeletons = frozen_models[9]
        .iter()
        .filter_map(|slot| resolve(*slot))
        .collect::<Vec<_>>();
    let peers = frozen_models[4]
        .iter()
        .filter_map(|slot| resolve(*slot))
        .collect::<Vec<_>>();
    let mut linked = Vec::new();
    let mut current = state.word_0x34;
    for _ in 0..=arena.authored_class5_archer_children().len() {
        if current == 0 {
            break;
        }
        let Some(child) = arena
            .authored_class5_archer_children()
            .iter()
            .find(|row| row.active && row.source_entity_index == current)
        else {
            break;
        };
        linked.push(ArcherLinkedState {
            entity_index: current,
            next_entity_index: child.word_0x34,
            life: i32::from(child.life),
            current_attacker: child.word_0x26,
        });
        current = child.word_0x34;
    }
    let target = resolve(source.word_0x96_150);
    let context = ArcherActionContext {
        terrain,
        control_flags: state.control_flags,
        pending_damage: state.pending_damage,
        linked_head: state.word_0x34,
        linked: &linked,
        target,
        entities: &players,
        fallback_entities: &skeletons,
        peers: &peers,
        relations: &relations,
        stage: state.stage,
        stage_runtime: Some(runtime),
    };
    let mut result = arena
        .economy
        .prepare_archer_source_slot(slot, context, terrain)
        .ok_or_else(|| {
            format!(
                "Archer slot {slot} cannot execute source action {}",
                source.action
            )
        })?;
    if let Some(request) = result.effects.child_request
        && construct_archer_arrow(arena, terrain, request).is_none()
    {
        result.effects.target_feedback_entity = None;
    }
    if let Some(death) = result.death {
        // KillEntity disables the current source only after mana and burst
        // constructors; the kernel's final hidden state is published below.
        arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == slot)
            .ok_or_else(|| format!("Archer death lost typed source at slot {slot}"))?
            .drawing_enabled = true;
        if death.mana > 0 {
            crate::world::effects::spawn_death_mana_sphere(
                arena,
                terrain,
                slot,
                death.position,
                death.yaw,
                death.mana,
                death.owner,
            )?;
            // The source clears the address's owner even if NewEvent fails.
            crate::world::effects::clear_replacement_mana_owner(arena, slot)?;
            arena.economy.source_owner.insert(slot, 0);
        }
        // KillEntity reads the current fixed-address parent after its mana
        // allocation, then the shared constructor records the actual birth.
        let current = super::super::source_target::read(
            arena,
            super::super::source_target::Context::default(),
            slot,
        )?
        .ok_or_else(|| format!("Archer death lost source address {slot}"))?;
        let burst = if current.control_flags & 0x10_0000 == 0 {
            crate::world::effects::spawn_death_burst(
                arena,
                terrain,
                particles,
                slot,
                current.position,
                current.source_id,
            )?
        } else {
            None
        };
        // If either allocation replaced the address, KillEntity disables the
        // current raw effect or typed sphere; otherwise the Archer owns hide.
        if !crate::world::effects::hide_birth_replacement(arena, slot, burst)? {
            if arena.slot_generation(slot) != source_generation {
                return Err(format!("Archer death cannot hide replacement at source slot {slot}"));
            }
            arena
                .economy
                .authored_class5
                .iter_mut()
                .find(|row| row.active && row.source_entity_index == slot)
                .ok_or_else(|| format!("Archer death lost typed source at slot {slot}"))?
                .drawing_enabled = false;
        }
    }
    if result.effects.target_dword_0x10_incremented
        && let Some(target) = result.target
    {
        apply_target_in_arena(arena, target);
    }
    if result.effects.expire_effect {
        let position = arena
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == slot)
            .map_or(source.position, |row| row.position);
        if result.effects.summon_expire_effect {
            crate::native_world::creatures::goat::runtime::spawn_expiry_visual(
                arena, terrain, position, particles,
            )?;
        }
        crate::world::effects::allocate(
            arena,
            terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::Impact,
            ),
            position,
            particles,
        )?;
    }
    // NewEvent can overwrite a source address. Only the original incarnation
    // may receive this family's epilogue after its constructors have run.
    if arena.slot_generation(slot) == state.slot_generation
        && let Some(source) =
            arena.economy.authored_class5.iter_mut().find(|row| {
                row.active && row.source_entity_index == slot && row.model == ARCHER_MODEL
            })
    {
        source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
        source.phase = source.phase.wrapping_add(1);
    }
    Ok(result)
}

/// AddArchers through NewEvent's frozen replacement cursor, including the old
/// terrain cell's head publication. The initializer owns the constructor RNG.
pub(crate) fn construct_campaign_archer(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    authored_index: u16,
    position: Position,
    max_life: i32,
) -> Option<Level2AuthoredClass5Source> {
    if max_life <= 0 {
        return None;
    }
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let source = arena.economy.initialize_campaign_archer(
        slot,
        authored_index,
        position,
        max_life,
        terrain,
    )?;
    crate::world::effects::sync_position(arena, terrain, slot, source.position, true, false)
        .expect("canonical Archer constructor retains its valid source slot");
    Some(source)
}

fn construct_archer_arrow(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    request: ArcherChildRequest,
) -> Option<ArcherChildPacket> {
    if !arena.authored_class5().iter().any(|source| {
        source.active
            && source.model == ARCHER_MODEL
            && source.source_entity_index == request.parent_entity_index
    }) {
        return None;
    }
    let slot = arena.allocate_source_slot_with_terrain(terrain)?;
    let packet = arena
        .economy
        .initialize_class9_model13_child(slot, request, false)?;
    crate::world::effects::sync_position(arena, terrain, slot, packet.position, true, false)
        .expect("canonical Archer arrow retains its valid source slot");
    Some(packet)
}

/// Group-20 authored Archer rows, in owner package/source order.
pub(crate) const ARCHER_AUTHORED_ROWS: [u16; 4] = [232, 233, 234, 235];

#[must_use]
pub(crate) fn is_authored_archer_row(authored_index: u16) -> bool {
    ARCHER_AUTHORED_ROWS.contains(&authored_index)
}

/// The complete `AddArchers_4BA10` source packet before projection into the
/// pre-existing class-5 root carrier.  `source_byte_39` is not decorative:
/// descriptor row 75 uses it as the first cadence countdown, so dropping it
/// changes the first target/child callback after construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherConstructorPayload {
    pub(crate) source_entity_index: u16,
    pub(crate) entity_rng: u16,
    pub(crate) authored_index: u16,
    pub(crate) source_class: u8,
    pub(crate) model: u8,
    pub(crate) action: u8,
    pub(crate) minimum_speed: i16,
    pub(crate) maximum_speed: i16,
    pub(crate) actual_speed: i16,
    pub(crate) max_life: i32,
    pub(crate) life: i32,
    pub(crate) mana: i32,
    pub(crate) max_mana: i32,
    pub(crate) subspell_index: u16,
    pub(crate) initial_dword_0x10: i32,
    pub(crate) dword_0x10: i32,
    pub(crate) byte_0x38: u8,
    pub(crate) descriptor_index: u16,
    pub(crate) source_byte_39: u8,
    pub(crate) xtype: u8,
    pub(crate) source_byte_3e: u8,
    pub(crate) particle_index: u16,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) roll: i16,
    pub(crate) fov: i16,
    pub(crate) shift_rot_x: i16,
    pub(crate) shift_rot_y: i16,
    pub(crate) position: Position,
    pub(crate) phase: u8,
    pub(crate) drawing_enabled: bool,
    pub(crate) active: bool,
}

/// Reproduce the ordered writes in `AddArchers_4BA10` after `NewEvent_4A050`.
/// `initial_rng` is the value installed by `NewEvent`; the constructor's one
/// LCG draw supplies the body orientation. `SetEntityIndexAndRot(0)` writes
/// the separate array_0x52 collision dimensions, not these body angles.
#[must_use]
pub(crate) fn archer_constructor_payload(
    source_entity_index: u16,
    initial_rng: u16,
    authored_index: u16,
    source_byte_3e: u8,
    position: Position,
    max_life: i32,
) -> ArcherConstructorPayload {
    let entity_rng = level2_next_rng16(initial_rng);
    let orientation = (entity_rng & 0x7ff) as i16 - 1;
    let source_byte_39 = ARCHER_DESCRIPTOR_CADENCE
        .wrapping_sub(source_byte_3e % ARCHER_DESCRIPTOR_CADENCE)
        .wrapping_add(4);
    ArcherConstructorPayload {
        source_entity_index,
        entity_rng,
        authored_index,
        source_class: ARCHER_SOURCE_CLASS,
        model: ARCHER_MODEL,
        action: ARCHER_ACTION,
        minimum_speed: ARCHER_MINIMUM_SPEED,
        maximum_speed: ARCHER_MAXIMUM_SPEED,
        actual_speed: ARCHER_MINIMUM_SPEED,
        max_life,
        life: max_life,
        mana: max_life >> 1,
        max_mana: ARCHER_MAX_MANA,
        subspell_index: ARCHER_SUBSPELL_INDEX,
        initial_dword_0x10: i32::from(source_entity_index % 100),
        dword_0x10: i32::from(source_entity_index % 100),
        byte_0x38: 1,
        descriptor_index: ARCHER_DESCRIPTOR_INDEX,
        source_byte_39,
        xtype: 3,
        source_byte_3e,
        particle_index: ARCHER_PARTICLE_INDEX,
        yaw: orientation,
        pitch: orientation,
        roll: orientation,
        fov: 0,
        shift_rot_x: ARCHER_SHIFT_ROT_X,
        shift_rot_y: ARCHER_SHIFT_ROT_Y,
        position,
        phase: 0,
        drawing_enabled: true,
        active: true,
    }
}

impl ArcherConstructorPayload {
    /// Project the packet into the existing class-5 root carrier.  The
    /// runtime sidecar still carries byte-0x38, xtype, and descriptor index;
    /// those are installed by `allocate_authored_archer` below.
    #[must_use]
    fn into_class5_source(self) -> Level2AuthoredClass5Source {
        Level2AuthoredClass5Source {
            source_entity_index: self.source_entity_index,
            entity_rng: self.entity_rng,
            authored_index: self.authored_index,
            model: self.model,
            action: self.action,
            minimum_speed: self.minimum_speed,
            maximum_speed: self.maximum_speed,
            actual_speed: self.actual_speed,
            max_life: self.max_life,
            life: self.life,
            mana: self.mana,
            max_mana: self.max_mana,
            subspell_index: self.subspell_index,
            dword_0x10: self.dword_0x10,
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: self.particle_index,
            source_byte_3e: self.source_byte_3e,
            source_byte_39: self.source_byte_39,
            yaw: self.yaw,
            pitch: self.pitch,
            roll: self.roll,
            fov: self.fov,
            shift_rot_x: self.shift_rot_x,
            shift_rot_y: self.shift_rot_y,
            position: self.position,
            phase: self.phase,
            drawing_enabled: self.drawing_enabled,
            active: self.active,
        }
    }
}

impl Level2EconomyState {
    /// A House's ordinary defender/random-resident constructor is the same
    /// Archer family, including its constructor RNG and real action callbacks.
    /// `AddHouse0A_2D_38330` only adds a one-point mailbox for a defender;
    /// neither it nor `GetRandManaSphere_38270` copies the House's owner/id.
    pub(crate) fn allocate_house_archer(
        &mut self,
        parent_house: u16,
        _house_owner: u16,
        request: HouseResidentRequest,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        if request.model != ARCHER_MODEL {
            return None;
        }
        let before = self.clone();
        let slot = self.allocate_slot()?;
        let result = self.initialize_house_archer(slot, parent_house, request, terrain);
        if result.is_none() {
            *self = before;
        }
        result
    }

    /// The House producer has already acquired this address from SourceArena.
    pub(crate) fn initialize_house_archer(
        &mut self,
        slot: u16,
        parent_house: u16,
        request: HouseResidentRequest,
        terrain: &SessionTerrain,
    ) -> Option<u16> {
        if request.model != ARCHER_MODEL {
            return None;
        }
        let source = self.initialize_campaign_archer(
            slot,
            u16::MAX,
            request.position,
            ARCHER_MAX_LIFE,
            terrain,
        )?;
        if let Some(state) = self
            .authored_class5_archer
            .iter_mut()
            .find(|row| row.source_entity_index == source.source_entity_index)
        {
            state.origin = ArcherOrigin::HouseSpawn(parent_house);
        }
        if request.attacker_entity_index != 0 {
            self.submit_authored_archer_damage(
                source.source_entity_index,
                request.attacker_entity_index,
                1,
            );
        }
        Some(source.source_entity_index)
    }

    fn commit_class9_model13_child(
        &mut self,
        request: ArcherChildRequest,
        castle_auxiliary_tail: bool,
    ) -> Option<ArcherChildPacket> {
        let state_before = self.clone();
        let source_entity_index = self.allocate_slot()?;
        let result = self.initialize_class9_model13_child(
            source_entity_index,
            request,
            castle_auxiliary_tail,
        );
        if result.is_none() {
            *self = state_before;
        }
        result
    }

    fn initialize_class9_model13_child(
        &mut self,
        source_entity_index: u16,
        request: ArcherChildRequest,
        castle_auxiliary_tail: bool,
    ) -> Option<ArcherChildPacket> {
        let entity_rng = self.new_event_rng(source_entity_index);
        let mut packet = archer_child_packet(source_entity_index, entity_rng, request);
        if castle_auxiliary_tail {
            // `sub_23E60` keeps AddEvent09_0D's NewEvent subspell (100),
            // then copies the model-15 auxiliary's xtype/xsubtype (3/-1).
            // The Archer-only sub_1CCE0 tail instead installs 250 and the
            // target's class/model, so it cannot be reused unchanged.
            packet.subspell_index = 100;
            packet.xtype = 3;
            packet.xsubtype = u8::MAX;
        }
        let position = Position {
            x: request.parent_position.x,
            y: request.parent_position.y,
            z: request
                .parent_position
                .z
                .wrapping_add(request.parent_fov / 2),
        };
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
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        // NewEvent_4A050 clears the source player/owner relation before the
        // class-9/model-13 constructor writes its own fields. Keep a reused
        // arena slot from leaking the prior occupant's owner into this child.
        self.source_owner.insert(source_entity_index, 0);
        // AddEvent09_0D + sub_1CCE0 retain the NewEvent-cleared word 0x34.
        // An arrow is an independent projectile, not an Archer body part:
        // linking it here makes sub_1FAA0 copy its lifetime into parent life.
        self.authored_class5_archer_children
            .push(Level2AuthoredClass5ArcherChild {
                source_entity_index: packet.source_entity_index,
                entity_id: packet.entity_id,
                entity_rng: packet.entity_rng,
                parent_entity_index: packet.parent_entity_index,
                parent_entity_id: packet.parent_entity_id,
                slot_generation: self.slot_generation(source_entity_index),
                word_0x32: 0,
                word_0x34: 0,
                word_0x24: 0,
                pending_word_0x62: 0,
                word_0x26: 0,
                word_0x96: packet.word_0x96,
                dword_0x10: packet.dword_0x10,
                source_class: packet.source_class,
                model: packet.model,
                action: packet.action,
                minimum_speed: packet.minimum_speed,
                maximum_speed: packet.maximum_speed,
                actual_speed: packet.actual_speed,
                max_life: packet.max_life,
                life: packet.life,
                subspell_index: packet.subspell_index,
                particle_index: packet.particle_index,
                control_flags: packet.control_flags,
                xtype: packet.xtype,
                xsubtype: packet.xsubtype,
                yaw: packet.yaw,
                pitch: packet.pitch,
                roll: packet.roll,
                fov: packet.fov,
                shift_rot_x: if castle_auxiliary_tail {
                    0
                } else {
                    ARCHER_SHIFT_ROT_X
                },
                shift_rot_y: if castle_auxiliary_tail {
                    0
                } else {
                    ARCHER_SHIFT_ROT_Y
                },
                position: packet.position,
                source_byte_3e: packet.source_byte_3e,
                phase: packet.phase,
                drawing_enabled: packet.drawing_enabled,
                active: packet.active,
            });
        Some(packet)
    }

    /// Allocate one authored Archer through the real source free stack,
    /// NewEvent RNG, model-4 constructor counter, terrain/map projection, and
    /// reset-owned class-5 source arena.
    pub(crate) fn allocate_authored_archer(
        &mut self,
        authored_index: u16,
        position: Position,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if !is_authored_archer_row(authored_index) {
            return None;
        }
        self.allocate_campaign_archer(authored_index, position, ARCHER_MAX_LIFE, terrain)
    }

    /// Allocate a package-admitted class-5/model-4 row through the shared
    /// Archer constructor. The campaign adapter has already established the
    /// row's class/model identity, so source record numbers are data here,
    /// not an engine-level Level-2 allowlist.
    pub(crate) fn allocate_campaign_archer(
        &mut self,
        authored_index: u16,
        position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if !(1..=i32::MAX).contains(&max_life) {
            return None;
        }
        let state_before = self.clone();
        let source_entity_index = self.allocate_slot()?;
        let result = self.initialize_campaign_archer(
            source_entity_index,
            authored_index,
            position,
            max_life,
            terrain,
        );
        if result.is_none() {
            *self = state_before;
        }
        result
    }

    fn initialize_campaign_archer(
        &mut self,
        source_entity_index: u16,
        authored_index: u16,
        position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if !(1..=i32::MAX).contains(&max_life) {
            return None;
        }
        let initial_rng = self.new_event_rng(source_entity_index);
        let source_byte_3e = self.class5_model4_constructor_phase;
        self.class5_model4_constructor_phase = source_byte_3e.wrapping_add(1);

        let mut position = position;
        position.z = terrain.ground_height(position);
        let payload = archer_constructor_payload(
            source_entity_index,
            initial_rng,
            authored_index,
            source_byte_3e,
            position,
            max_life,
        );
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
            return None;
        }
        let source = payload.into_class5_source();
        self.source_rng
            .insert(source_entity_index, source.entity_rng);
        // NewEvent_4A050 memsets playerEntityIndex to zero. The Archer
        // constructor does not replace it, so a reused source slot must not
        // retain the previous occupant's owner relation.
        self.source_owner.insert(source_entity_index, 0);
        self.authored_class5.push(source);
        self.authored_class5_archer
            .push(Level2AuthoredClass5ArcherState {
                source_entity_index,
                entity_id: source_entity_index,
                origin: ArcherOrigin::AuthoredRecord(authored_index),
                slot_generation: self.slot_generation(source_entity_index),
                word_0x32: 0,
                word_0x34: 0,
                descriptor_index: payload.descriptor_index,
                byte_0x38: payload.byte_0x38,
                xtype: payload.xtype,
                pending_damage: 0,
                last_child_root_phase: 0,
                // NewEvent starts damageable (8); AddEventToMap adds 4.
                control_flags: 12,
                xsubtype: u8::MAX,
                stage: ArcherStageState::default(),
                proximity_delay: 0,
            });
        Some(source)
    }

    /// Submit one `sub_10C80(a2 = 0)` mailbox write to a live Archer root.
    pub(crate) fn submit_authored_archer_damage(
        &mut self,
        source_slot: u16,
        attacker_slot: u16,
        damage: i32,
    ) -> bool {
        if damage <= 0
            || attacker_slot == 0
            || attacker_slot == source_slot
            || !self.map_chains.is_linked(source_slot)
        {
            return false;
        }
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == ARCHER_MODEL
        }) else {
            return false;
        };
        let Some(runtime) = self
            .authored_class5_archer
            .iter_mut()
            .find(|runtime| runtime.source_entity_index == source_slot)
        else {
            return false;
        };
        DamageMailboxMut::new(&mut runtime.pending_damage, &mut source.pending_word_0x62).submit(
            damage,
            attacker_slot,
            DamageWriter::Ordinary,
        )
    }

    #[cfg(test)]
    pub(crate) fn stage_archer_mailbox_damage_for_test(
        &mut self,
        source_slot: u16,
        attacker_slot: u16,
        damage: i32,
    ) -> bool {
        self.submit_authored_archer_damage(source_slot, attacker_slot, damage)
    }
}

impl Level2EconomyState {
    /// Allocate the child source slot and map relation without inventing a
    /// second child arena.  The caller must retain the returned packet in the
    /// shared reset/hash/reclaim carrier.  Failure before the map link leaves
    /// the free stack, source RNG map, and child state unchanged.
    pub(crate) fn allocate_archer_child_packet(
        &mut self,
        request: ArcherChildRequest,
    ) -> Option<ArcherChildPacket> {
        self.authored_class5_archer.iter().find(|state| {
            state.source_entity_index == request.parent_entity_index
                && self.authored_class5.iter().any(|source| {
                    source.active && source.source_entity_index == state.source_entity_index
                })
        })?;
        self.commit_class9_model13_child(request, false)
    }

    /// Commit the same `IfSubtypeCallCreatingManaSphere(..., 9, 13)` product
    /// with the Castle-specific constructor tail. Castle auxiliary action
    /// 122 sets only id, aim, target, xtype/xsubtype and the raised Z position.
    pub(crate) fn allocate_castle_auxiliary_projectile_packet(
        &mut self,
        request: ArcherChildRequest,
    ) -> Option<ArcherChildPacket> {
        self.commit_class9_model13_child(request, true)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArcherStepResult {
    pub(crate) effects: ArcherActionEffects,
    pub(crate) target: Option<ArcherTarget>,
    pub(crate) map_synced: bool,
    death: Option<ArcherDeathRequest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArcherDeathRequest {
    position: Position,
    yaw: i16,
    mana: i32,
    owner: u16,
}

impl Level2EconomyState {
    /// Dispatch one retained model-4 root from the ordinary dynamic cursor.
    /// The outer reset session supplies imported entity views; all action
    /// state, child allocation, source cadence, map projection, and rollback
    /// remain in this shared economy carrier.
    #[cfg(test)]
    pub(crate) fn step_authored_archer_slot(
        &mut self,
        source_slot: u16,
        context: ArcherActionContext<'_>,
        terrain: &SessionTerrain,
    ) -> Option<ArcherStepResult> {
        self.step_archer_action_packet(source_slot, context, terrain, false)
    }

    fn prepare_archer_source_slot(
        &mut self,
        source_slot: u16,
        context: ArcherActionContext<'_>,
        terrain: &SessionTerrain,
    ) -> Option<ArcherStepResult> {
        self.step_archer_action_packet(source_slot, context, terrain, true)
    }

    fn step_archer_action_packet(
        &mut self,
        source_slot: u16,
        mut context: ArcherActionContext<'_>,
        terrain: &SessionTerrain,
        arena_allocation: bool,
    ) -> Option<ArcherStepResult> {
        let source_index = self.authored_class5.iter().position(|source| {
            // The numeric callback scan tests the current class/action,
            // without rechecking the hide bit. An earlier Skeleton may hide
            // this root before its visit; it still executes once now and is
            // reclaimed by the following pass's prologue.
            source.active && source.source_entity_index == source_slot
        })?;
        let archer_index = self
            .authored_class5_archer
            .iter()
            .position(|state| state.source_entity_index == source_slot)?;
        let state_before = self.clone();
        let mut source = self.authored_class5[source_index];
        let mut archer_state = self.authored_class5_archer[archer_index];
        let mut word_0x32 = archer_state.word_0x32;
        let mut death = None;
        context.linked_head = archer_state.word_0x34;
        context.control_flags = archer_state.control_flags;
        context.stage = archer_state.stage;

        let mut effects = match source.action {
            ARCHER_ACTION_STATIONARY => {
                step_archer_action32(&mut source, &mut word_0x32, &mut context)
            }
            ARCHER_ACTION => step_archer_action33(&mut source, &mut word_0x32, &mut context),
            ARCHER_ACTION_ATTACK => step_archer_action34(&mut source, &mut context),
            ARCHER_ACTION_LINK => step_archer_action35(&mut source, &mut word_0x32, &mut context),
            ARCHER_ACTION_HIT => {
                step_archer_action36(&mut source);
                if source.action == ARCHER_ACTION_KILL {
                    self.prekill_archer_chain(&mut source, archer_state.word_0x34);
                    if let Some(killer) = context
                        .relations
                        .iter()
                        .find(|row| row.slot == source.word_0x24)
                        .map(|row| row.target)
                        .or_else(|| {
                            context
                                .entities
                                .iter()
                                .copied()
                                .find(|target| target.entity_index == source.word_0x24)
                        })
                        && awards_creature_kill(
                            source.model,
                            archer_state.entity_id,
                            context
                                .relations
                                .iter()
                                .find(|row| row.slot == killer.entity_index)
                                .map_or(killer.entity_index, |row| row.source_id),
                            killer.class,
                            killer.model,
                        )
                    {
                        self.record_authored_creature_kill(killer.entity_index);
                    }
                }
                ArcherActionEffects::default()
            }
            ARCHER_ACTION_KILL => {
                // `KillArcher_201040` calls KillEntity only on the phase-zero
                // cadence. Visits at phases 1..7 retain the row and consume no
                // allocation. The common UpdateEntities epilogue below then
                // advances byte 0x3E exactly once.
                if source.source_byte_3e & 7 == 0 {
                    let owner_entity_index = self.source_owner(source.source_entity_index);
                    if source.mana > 0 {
                        // KillEntity_1C930 advances the dying row's RNG once
                        // before TransformEntityToManaSphere_36BA0 consumes
                        // the resulting stream value.
                        source.entity_rng = level2_next_rng16(source.entity_rng);
                    }
                    if arena_allocation {
                        death = Some(ArcherDeathRequest {
                            position: source.position,
                            yaw: source.yaw,
                            mana: source.mana,
                            owner: owner_entity_index,
                        });
                    } else {
                        let _ = self.spawn_mana_sphere_from_death(
                            source.source_entity_index,
                            source.position,
                            source.yaw,
                            source.entity_rng,
                            source.mana,
                            owner_entity_index,
                            terrain,
                        );
                        if context.control_flags & 0x10_0000 == 0 {
                            let _ = self.spawn_death_burst_from_death_with_id(
                                source.source_entity_index,
                                archer_state.entity_id,
                                source.position,
                            );
                        }
                    }
                    source.drawing_enabled = false;
                }
                ArcherActionEffects::default()
            }
            ARCHER_ACTION_IDLE_VISUAL => step_archer_action39(&mut source, &mut context),
            // str50[38] has no source callback. Never manufacture a visual
            // update or advance its callback clock for this unsupported slot.
            _ => return None,
        };

        if !arena_allocation && let Some(request) = effects.child_request {
            if self.allocate_archer_child_packet(request).is_none() {
                effects.target_feedback_entity = None;
            }
            archer_state.word_0x34 = self.authored_class5_archer[archer_index].word_0x34;
        }
        if effects.population_decrement {
            self.decrement_source_creature_population();
        }
        if let Some(peer) = effects.peer_update {
            if let Some(root) = self
                .authored_class5
                .iter_mut()
                .find(|root| root.active && root.source_entity_index == peer.entity_index)
            {
                root.action = peer.action;
                root.word_0x96_150 = peer.word_0x96_150;
            }
            if let Some(state) = self
                .authored_class5_archer
                .iter_mut()
                .find(|state| state.source_entity_index == peer.entity_index)
            {
                state.word_0x32 = peer.word_0x32;
            }
        }
        if let Some(visual) = effects.visual {
            archer_state.xtype = visual.xtype;
            archer_state.xsubtype = visual.xsubtype;
        }
        if archer_state.stage.mode == 14 {
            let mut current = archer_state.word_0x34;
            for _ in 0..=self.authored_class5_archer_children.len() {
                let Some(child) = self
                    .authored_class5_archer_children
                    .iter_mut()
                    .find(|child| child.source_entity_index == current)
                else {
                    break;
                };
                child.control_flags =
                    (child.control_flags & !0x0600_0000) | (context.control_flags & 0x0600_0000);
                current = child.word_0x34;
                if current == 0 {
                    break;
                }
            }
        }
        archer_state.control_flags = context.control_flags;
        archer_state.stage = context.stage;
        if !arena_allocation {
            source.source_byte_3e = source.source_byte_3e.wrapping_add(1);
            source.phase = source.phase.wrapping_add(1);
        }
        archer_state.word_0x32 = word_0x32;
        archer_state.pending_damage = context.pending_damage;
        self.authored_class5[source_index] = source;
        self.authored_class5_archer[archer_index] = archer_state;
        self.source_rng.insert(source_slot, source.entity_rng);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .is_err()
        {
            *self = state_before;
            return None;
        }
        Some(ArcherStepResult {
            effects,
            target: context.target,
            map_synced: true,
            death,
        })
    }

    fn prekill_archer_chain(&mut self, source: &mut Level2AuthoredClass5Source, chain_head: u16) {
        let mut current = chain_head;
        let mut visits = 0_usize;
        while current != 0 && visits <= self.authored_class5_archer_children.len() {
            let Some(child_index) = self
                .authored_class5_archer_children
                .iter()
                .position(|child| child.active && child.source_entity_index == current)
            else {
                break;
            };
            let child = &mut self.authored_class5_archer_children[child_index];
            child.action = ARCHER_ACTION_KILL;
            inherit_killer(&mut source.word_0x24, child.word_0x24);
            current = child.word_0x34;
            visits = visits.wrapping_add(1);
        }
    }

    /// Class/model, protection flags and mana at the current source visit.
    /// Resolve typed live carriers before the imported reset rows.
    fn archer_arrow_target_properties(
        &self,
        slot: u16,
        imported: &[EntitySnapshot],
    ) -> Option<(u8, u8, u32, i32)> {
        if let Some(actor) = self
            .castle_defenders
            .iter()
            .find(|row| row.active && row.source_entity_index == slot)
        {
            return Some((5, 15, actor.control_flags, actor.mana));
        }
        if let Some(part) = self
            .authored_dragon_parts
            .iter()
            .find(|row| row.source_entity_index == slot)
        {
            return Some((5, part.model, 8, part.mana));
        }
        if let Some(root) = self
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == slot)
        {
            let flags = self
                .authored_class5_archer
                .iter()
                .find(|row| row.source_entity_index == slot)
                .map(|row| row.control_flags)
                .or_else(|| {
                    self.authored_dragons
                        .iter()
                        .find(|row| row.source_entity_index == slot)
                        .map(|row| row.control_flags)
                })
                .unwrap_or(8);
            return Some((5, root.model, flags, root.mana));
        }
        if let Some(bee) = self
            .bee_creatures
            .iter()
            .find(|row| row.source_entity_index == slot)
        {
            return Some((bee.source_class, bee.model, bee.control_flags, bee.mana));
        }
        if let Some(castle) = self.castle_entry(slot) {
            return Some((3, 2, castle.control_flags, castle.mana));
        }
        imported
            .get(usize::from(slot))
            .filter(|row| row.active)
            .map(|row| (row.class, row.model, row.control_flags, row.mana))
    }

    /// AddArcherArrow_672E0 on the shared source cursor. A hidden arrow is
    /// reclaimed only at the following source boundary. The third return
    /// value requests the reflector's source family-8 experience award.
    pub(in crate::native_world) fn step_authored_class5_archer_child_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        imported_entities: &[EntitySnapshot],
    ) -> (
        bool,
        Option<level2_dynamic_economy::Level2Class10Model0Step>,
        Option<u16>,
    ) {
        let Some(child_index) = self
            .authored_class5_archer_children
            .iter()
            .position(|child| child.active && child.source_entity_index == source_slot)
        else {
            return (false, None, None);
        };
        let state_before = self.clone();
        let mut child = self.authored_class5_archer_children[child_index];
        let mut damage = None;
        let mut reflection = None;
        if child.action == ARCHER_CHILD_ACTION {
            // AddArcherArrow_672E0 draws from the global stream exactly once.
            if child.control_flags & 2 == 0 {
                self.advance_global_rng();
                let rng = self.global_rng;
                mark_source_audio_call(SourceAudioCallTag::ArcherArrowLaunch);
                self.prepare_sound(source_slot, source_slot, -1, ((rng & 1) + 33) as u16);
                child.control_flags |= 2;
            }
            let mut predicted = child.position;
            crate::move_entity(&mut predicted, child.yaw, child.pitch, child.actual_speed);
            // sub_10780 searches at the current position, not the prediction.
            let targets = self.fireball_collision_targets(imported_entities);
            let hit = source_map_search_slots(
                &self.map_chains,
                child.position,
                &source_search_offsets(0),
            )
            .into_iter()
            .find_map(|slot| {
                targets.iter().find(|target| {
                    target.source_slot == slot
                        && target.source_id != child.entity_id
                        && self
                            .archer_arrow_target_properties(slot, imported_entities)
                            .is_some_and(|(class, model, _, _)| {
                                child.xtype == u8::MAX
                                    || (child.xtype == class
                                        && (child.xsubtype == u8::MAX || child.xsubtype == model))
                            })
                        && crate::level1_mission::source_entities_overlap(
                            child.position,
                            0,
                            30,
                            60,
                            target.position,
                            target.half_width,
                            target.vertical_offset,
                            target.half_height,
                        )
                })
            })
            .copied();
            // The post-decrement is conditional on terrain admission in source.
            let mut advances = false;
            if terrain.ground_height(predicted) <= predicted.z {
                let life = child.life;
                child.life = child.life.wrapping_sub(1);
                advances = life != 0 && hit.is_none();
            }
            if advances {
                child.position = predicted;
            } else if let Some(target) = hit.filter(|target| {
                self.archer_arrow_target_properties(target.source_slot, imported_entities)
                    .is_some_and(|(_, _, flags, mana)| flags & 0x8010 != 0 && mana >= 0)
            }) {
                // NewEvent/AddEvent09_0D leave mana zero: reflection costs
                // zero, but still awards experience and prepares sound 28.
                let (_, _, flags, _) = self
                    .archer_arrow_target_properties(target.source_slot, imported_entities)
                    .unwrap();
                self.prepare_sound(source_slot, target.source_slot, -1, 28);
                reflection = Some(target.source_slot);
                child.roll = child.yaw.wrapping_add(1024) & 2047;
                child.pitch = child.pitch.wrapping_neg() & 2047;
                child.fov = child.pitch;
                if flags & 0x10 != 0 {
                    child.yaw = child.roll;
                    child.subspell_index = child.subspell_index.wrapping_mul(2);
                } else {
                    child.entity_rng = level2_next_rng16(child.entity_rng);
                    child.yaw = child
                        .roll
                        .wrapping_add((child.entity_rng % 45) as i16)
                        .wrapping_sub(22);
                }
                child.word_0x96 = child.entity_id;
                if let Some((class, model, _, _)) =
                    self.archer_arrow_target_properties(child.entity_id, imported_entities)
                {
                    child.xtype = class;
                    child.xsubtype = model;
                }
                child.entity_id = target.source_id;
                child.life = child.max_life;
                child.position = target.position;
                child.position.z = child.position.z.wrapping_add(target.half_height);
            } else {
                if let Some(target) = hit {
                    child.position = target.position;
                    // sub_65580 temporarily shifts the hit to its collision centre.
                    let model = self
                        .archer_arrow_target_properties(target.source_slot, imported_entities)
                        .map(|(_, model, _, _)| model);
                    if model != Some(2) {
                        child.position.z = child.position.z.wrapping_add(target.vertical_offset);
                    }
                }
                damage = Some(level2_dynamic_economy::Level2Class10Model0Step {
                    live: true,
                    first_impact: true,
                    damage_pulse: true,
                    owner_entity_index: child.entity_id,
                    payload: child.subspell_index,
                    position: child.position,
                });
                child.drawing_enabled = false;
                child.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            }
        }
        child.source_byte_3e = child.source_byte_3e.wrapping_add(1);
        child.phase = child.phase.wrapping_add(1);
        self.authored_class5_archer_children[child_index] = child;
        self.source_rng.insert(source_slot, child.entity_rng);
        if self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: child.position.x,
                y: child.position.y,
            })
            .is_err()
        {
            *self = state_before;
            return (false, None, None);
        }
        (true, damage, reflection)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::level_two_fixture_path;
    use super::*;
    use crate::native_world::{
        Level2EconomyEvent, NativeWorld, heading_for_positions, level2_next_rng32,
    };

    fn archer_position() -> Position {
        Position {
            x: 0x2800,
            y: 0x3100,
            z: 0,
        }
    }

    fn empty_context(terrain: &SessionTerrain) -> ArcherActionContext<'_> {
        ArcherActionContext {
            terrain,
            control_flags: 4,
            pending_damage: 0,
            linked_head: 0,
            linked: &[],
            target: None,
            entities: &[],
            fallback_entities: &[],
            peers: &[],
            relations: &[],
            stage: ArcherStageState::default(),
            stage_runtime: None,
        }
    }

    #[test]
    fn scheduled_alliance_controls_authored_archer_until_caster_generation_is_lost() {
        let mut session = crate::native_world::tests::level_two_campaign_session();
        let parent_slot = session.state.entity_index;
        let position = session.state.position;
        let parent = session.source_arena().handle(parent_slot).unwrap();
        let (slot, effect) = {
            let live = &mut session.session.state.live;
            let archer = construct_campaign_archer(
                &mut live.economy, &mut live.terrain, 232, position, ARCHER_MAX_LIFE,
            ).expect("ordinary authored Archer constructor");
            let effect = crate::native_world::alliance_spell::construct_impact(
                &mut live.economy, &mut live.terrain,
            ).expect("Alliance terminal allocation");
            let row = &mut live.economy.entities[usize::from(effect)];
            row.source_id = parent_slot;
            row.record[30..32].copy_from_slice(&parent_slot.to_le_bytes());
            row.record[74] = 32;
            row.subspell_index = 40;
            row.record[154..156].copy_from_slice(&archer.source_entity_index.to_le_bytes());
            (archer.source_entity_index, effect)
        };
        let child = session.source_arena().handle(slot).unwrap();
        let effect_handle = session.source_arena().handle(effect).unwrap();
        let original_id = session.source_arena().authored_class5_archer().iter()
            .find(|state| state.source_entity_index == slot).unwrap().entity_id;
        session.step_level2_source_pass(0);
        assert!(!session.dispatch_events().iter().any(|event|
            matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })));
        let state = session.source_arena().authored_class5_archer().iter()
            .find(|state| state.source_entity_index == slot).unwrap();
        assert_eq!(state.entity_id, original_id);
        assert_eq!((state.stage.mode, state.stage.parent, state.stage.duration), (14, parent_slot, 40));
        assert!((1..=40).contains(&state.stage.remaining));
        assert_eq!(session.source_arena().creature_parent_binding(child), Some(parent));
        assert!(session.source_arena().resolve(effect_handle).is_some());
        assert!(session.source_arena().entities[usize::from(effect)].drawing_disabled());
        session.step_level2_source_pass(1);
        assert!(session.source_arena().resolve(effect_handle).is_none());

        for recycled in [false, true] {
            let mut control = session.clone();
            if recycled {
                let live = &mut control.session.state.live;
                live.economy.economy.release_slot(parent_slot, None);
                assert_eq!(live.economy.allocate_source_slot_with_terrain(&mut live.terrain), Some(parent_slot));
                let mut replacement = crate::native_world::cave_features::new_event(parent_slot, 5, position);
                replacement.class = 3;
                replacement.model = 0;
                replacement.record[67] = 3;
                replacement.set_life(1_000);
                live.economy.entities[usize::from(parent_slot)] = replacement;
                assert_ne!(live.economy.handle(parent_slot), Some(parent));
            }
            control.step_level2_source_pass(0);
            assert!(!control.dispatch_events().iter().any(|event|
                matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })));
            let state = control.source_arena().authored_class5_archer().iter()
                .find(|state| state.source_entity_index == slot).unwrap();
            assert_eq!(state.entity_id, original_id);
            if recycled {
                assert_eq!((state.stage.mode, state.stage.parent, state.stage.index), (10, 0, 0));
            } else {
                assert_eq!((state.stage.mode, state.stage.parent), (14, parent_slot));
            }
        }
    }

    #[test]
    fn allied_archer_reads_typed_creature_current_parent_before_selecting_attack() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let runtime = session.session.state.live.core.runtime.clone();
        let particles = &session.session.definition.presentation.particles;
        let parent = session.state.entity_index;
        let position = session.state.position;
        let archer = construct_campaign_archer(&mut arena, &mut terrain, 232, position, ARCHER_MAX_LIFE)
            .expect("ordinary authored Archer");
        let firefly = arena.economy.allocate_authored_firefly_unbound(0, position, 1_000, &terrain)
            .expect("ordinary unbound Firefly");
        crate::world::effects::sync_position(
            &mut arena, &mut terrain, firefly.source_entity_index, firefly.position, true, false,
        ).unwrap();
        let bee = crate::native_world::creatures::bee::mc2::allocate_authored(
            &mut arena, &mut terrain, 149, position, 1_000,
        ).expect("ordinary authored Bee on Day terrain");
        let tile = ((position.x >> 8) as u8, (position.y >> 8) as u8);
        let order = arena.economy.map_chains.entity_chain(tile.0, tile.1);
        assert!(order.contains(&archer.source_entity_index));
        assert!(order.contains(&firefly.source_entity_index));
        assert!(order.contains(&bee.source_entity_index));
        for target in [archer.source_entity_index, firefly.source_entity_index, bee.source_entity_index] {
            let effect = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain)
                .expect("Alliance terminal");
            let row = &mut arena.entities[usize::from(effect)];
            row.source_id = parent;
            row.record[30..32].copy_from_slice(&parent.to_le_bytes());
            row.record[74] = 1;
            row.subspell_index = 40;
            row.record[154..156].copy_from_slice(&target.to_le_bytes());
            let impact = crate::native_world::alliance_spell::step_impact(&mut arena, effect, &order)
                .expect("source Alliance impact");
            assert!(impact.affected_entities.contains(&target));
        }
        for (target, is_bee, same_parent) in [
            (firefly.source_entity_index, false, true),
            (firefly.source_entity_index, false, false),
            (bee.source_entity_index, true, true),
            (bee.source_entity_index, true, false),
        ] {
            let mut control = arena.clone();
            let mut world = terrain.clone();
            let mut control_runtime = runtime.clone();
            control.entities[usize::from(parent)].record[154..156]
                .copy_from_slice(&target.to_le_bytes());
            if !same_parent {
                if is_bee {
                    control.economy.bee_creatures.iter_mut()
                        .find(|state| state.source_entity_index == target)
                        .unwrap().parent_entity_index = 0;
                } else {
                    control.economy.authored_fireflies.iter_mut()
                        .find(|state| state.source_entity_index == target)
                        .unwrap().parent_entity_index = 0;
                }
            }
            let relation = relations_in_arena(&control).into_iter()
                .find(|row| row.slot == target).unwrap();
            assert_eq!(relation.stage_mode, 14);
            assert_eq!(relation.parent, if same_parent { parent } else { 0 });
            let families = control.rebuilt_class5_model_slots();
            step_source_runtime(&mut control, &mut control_runtime, &mut world, particles,
                archer.source_entity_index, &families, &[parent], &[]).unwrap();
            let root = control.authored_class5().iter()
                .find(|row| row.source_entity_index == archer.source_entity_index).unwrap();
            if same_parent {
                assert_eq!(root.word_0x96_150, 0, "same-parent allied creature is not attacked");
                assert_eq!(root.action, ARCHER_ACTION_IDLE_VISUAL);
            } else {
                assert_eq!(root.word_0x96_150, target);
                assert_eq!(root.action, ARCHER_ACTION_ATTACK,
                    "the near-identical different-parent creature remains a real target");
                for _ in 0..=ARCHER_DESCRIPTOR_CADENCE {
                    let families = control.rebuilt_class5_model_slots();
                    step_source_runtime(&mut control, &mut control_runtime, &mut world, particles,
                        archer.source_entity_index, &families, &[parent], &[]).unwrap();
                    if control.authored_class5_archer_children().iter().any(|child|
                        child.active && child.parent_entity_index == archer.source_entity_index)
                    {
                        break;
                    }
                }
                let arrow = control.authored_class5_archer_children().iter().find(|child|
                    child.active && child.parent_entity_index == archer.source_entity_index)
                    .expect("the ordinary attack callback allocates its real arrow");
                assert_eq!((arrow.source_class, arrow.model, arrow.xtype, arrow.xsubtype),
                    (9, 13, 5, if is_bee { 2 } else { 19 }));
            }
        }
    }

    #[test]
    fn archer_constructors_use_frozen_arena_replacement_and_publish_old_cell() {
        for arrow in [false, true] {
            let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
            let session = inspection.level2_reset_session().unwrap();
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            for row in &mut arena.entities {
                row.set_control_flags(
                    row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                );
            }
            let parent = construct_campaign_archer(
                &mut arena,
                &mut terrain,
                232,
                archer_position(),
                ARCHER_MAX_LIFE,
            )
            .unwrap();
            let old_tile = (0..=u16::MAX)
                .find(|tile| arena.economy.map_chains.tile_head(*tile) == 0)
                .unwrap();
            let old_position = Position {
                x: ((old_tile & 255) << 8) + 128,
                y: (old_tile & 0xff00) + 128,
                z: 128,
            };
            let replaced = crate::world::effects::allocate(
                &mut arena,
                &mut terrain,
                sim_definition::adapter::mc2_effect_definition(
                    crate::level_definition::EffectKind::Impact,
                ),
                old_position,
                &inspection.presentation().particles,
            )
            .unwrap()
            .unwrap();
            let generation = arena.slot_generation(replaced);
            arena.economy.exhaust_new_event_capacity_for_test();
            arena.rebuild_source_free_arena();
            // Retain exactly the eligible raw effect captured by 49F90.
            // This is an allocator regression, not a live gameplay capture.
            let request = ArcherChildRequest {
                parent_entity_index: parent.source_entity_index,
                parent_entity_id: parent.source_entity_index,
                parent_word_0x96: session.state.entity_index,
                parent_roll: parent.roll,
                parent_position: parent.position,
                parent_fov: parent.shift_rot_y,
                target_entity_index: session.state.entity_index,
                target_class: 3,
                target_model: 0,
                target_position: session.state.position,
            };
            let slot = if arrow {
                construct_archer_arrow(&mut arena, &mut terrain, request)
                    .unwrap()
                    .source_entity_index
            } else {
                construct_campaign_archer(&mut arena, &mut terrain, 233, archer_position(), 1234)
                    .unwrap()
                    .source_entity_index
            };
            assert_eq!(slot, replaced);
            assert_eq!(arena.slot_generation(slot), generation.wrapping_add(1));
            assert_eq!(arena.economy.map_chains.tile_head(old_tile), 0);
            assert_eq!(terrain.planes().entity_indices[usize::from(old_tile)], 0);
            let new_tile = arena.economy.map_chains.tile_of(slot);
            assert_eq!(
                terrain.planes().entity_indices[usize::from(new_tile)] as u16,
                arena.economy.map_chains.tile_head(new_tile)
            );
            let before = (
                arena.global_rng,
                arena.class5_model4_constructor_phase,
                arena.authored_class5_archer_children().len(),
                arena.authored_class5().len(),
            );
            assert!(construct_archer_arrow(&mut arena, &mut terrain, request).is_none());
            assert!(
                construct_campaign_archer(&mut arena, &mut terrain, 234, archer_position(), 1000)
                    .is_none()
            );
            assert_eq!(
                (
                    arena.global_rng,
                    arena.class5_model4_constructor_phase,
                    arena.authored_class5_archer_children().len(),
                    arena.authored_class5().len()
                ),
                before
            );
        }
        for capacity in 0..=2 {
            let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
            let session = inspection.level2_reset_session().unwrap();
            let mut arena = session.source_arena().clone();
            let mut terrain = session.canonical_terrain().clone();
            let mut runtime = session.runtime.clone();
            for row in &mut arena.entities {
                row.set_control_flags(
                    row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                );
            }
            let old_tile = (0..=u16::MAX)
                .find(|tile| arena.economy.map_chains.tile_head(*tile) == 0)
                .unwrap();
            let old_position = Position {
                x: ((old_tile & 255) << 8) + 128,
                y: (old_tile & 0xff00) + 128,
                z: 128,
            };
            // Capture lower addresses before the root exists. Their death
            // products must wait for the following ascending source scan.
            let mut replaced = Vec::new();
            for _ in 0..capacity {
                let victim = crate::world::effects::allocate(
                    &mut arena,
                    &mut terrain,
                    sim_definition::adapter::mc2_effect_definition(
                        crate::level_definition::EffectKind::Impact,
                    ),
                    old_position,
                    &inspection.presentation().particles,
                )
                .unwrap()
                .unwrap();
                replaced.push((victim, arena.slot_generation(victim)));
            }
            let parent = construct_campaign_archer(
                &mut arena,
                &mut terrain,
                232,
                archer_position(),
                ARCHER_MAX_LIFE,
            )
            .unwrap();
            let slot = parent.source_entity_index;
            let old = arena.handle(slot).unwrap();
            let root = arena
                .economy
                .authored_class5
                .iter_mut()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            root.action = ARCHER_ACTION_KILL;
            root.source_byte_3e = 0;
            root.phase = 0;
            root.mana = 321;
            let initial_rng = root.entity_rng;
            arena
                .economy
                .source_owner
                .insert(slot, session.state.entity_index);
            arena.economy.exhaust_new_event_capacity_for_test();
            arena.rebuild_source_free_arena();
            assert_eq!(arena.economy.free_slot_count(), 0);
            let families = arena.rebuilt_class5_model_slots();
            step_source_runtime(
                &mut arena,
                &mut runtime,
                &mut terrain,
                &inspection.presentation().particles,
                slot,
                &families,
                &[],
                &[],
            )
            .unwrap();
            assert!(arena.resolve(old).is_some());
            let root = arena
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap();
            assert!(!root.drawing_enabled);
            assert_eq!(root.source_byte_3e, 1);
            assert_eq!(root.entity_rng, level2_next_rng16(initial_rng));
            assert_eq!(arena.economy.source_owner(slot), 0);
            if capacity > 0 {
                let (child, generation) = replaced[0];
                assert!(child < slot);
                let sphere = arena
                    .spheres()
                    .iter()
                    .find(|row| row.source_entity_index == child)
                    .unwrap();
                assert_eq!(sphere.position, parent.position);
                assert_eq!(sphere.mana, 321);
                assert_eq!(sphere.player_entity_index, session.state.entity_index);
                assert!(sphere.active && !sphere.pending_delete);
                assert_eq!(sphere.phase, child as u8);
                // 500C0 colors the default neutral 512 packet before 36BA0
                // copies this Archer's mana and owner. Those copies do not
                // execute 36920 or change the retained collision axes.
                assert_eq!(
                    (
                        sphere.particle_index,
                        sphere.collision_yaw,
                        sphere.collision_pitch,
                        sphere.collision_roll,
                        sphere.collision_fov,
                    ),
                    (53, 28, 28, 28, 28)
                );
                assert_eq!(arena.slot_generation(child), generation.wrapping_add(1));
            }
            if capacity == 2 {
                let (child, generation) = replaced[1];
                let burst = &arena.entities[usize::from(child)];
                assert_eq!((burst.class, burst.model, burst.action), (10, 1, 1));
                assert_eq!(burst.position, parent.position);
                assert_eq!(burst.source_id, slot);
                assert!(!burst.drawing_disabled());
                assert_eq!(arena.slot_generation(child), generation.wrapping_add(1));
            }
            assert_eq!(arena.economy.map_chains.tile_head(old_tile), 0);
            assert_eq!(terrain.planes().entity_indices[usize::from(old_tile)], 0);
            assert!(
                arena
                    .allocate_source_slot_with_terrain(&mut terrain)
                    .is_none()
            );
            arena.reclaim_hidden_archer_rows_at_source_boundary();
            arena
                .rebuild_source_arena_with_terrain(&mut terrain, &[])
                .unwrap();
            assert!(arena.resolve(old).is_none());
            if capacity > 0 {
                let child = replaced[0].0;
                // The palette resolver is an explicit callback input here;
                // actual player-color lookup belongs to the root adapter.
                let (economy, entities) = (&mut arena.economy, &mut arena.entities);
                assert!(
                    economy.step_mana_sphere_slot(child, &terrain, entities, |owner| {
                        assert_eq!(owner, session.state.entity_index);
                        105
                    },)
                );
                let sphere = arena
                    .spheres()
                    .iter()
                    .find(|row| row.source_entity_index == child)
                    .unwrap();
                assert_eq!(
                    (
                        sphere.particle_index,
                        sphere.collision_yaw,
                        sphere.collision_pitch,
                        sphere.collision_roll,
                        sphere.collision_fov,
                    ),
                    (106, 28, 28, 28, 28)
                );
                assert_eq!(sphere.phase, (child as u8).wrapping_add(1));
            }
        }
    }

    #[test]
    fn archer_free_stack_reuse_retires_raw_packet_through_hidden_reclaim() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let session = inspection.level2_reset_session().unwrap();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.canonical_terrain().clone();
        let mut runtime = session.runtime.clone();
        let position = archer_position();
        let slot = crate::world::effects::allocate(
            &mut arena,
            &mut terrain,
            sim_definition::adapter::mc2_effect_definition(
                crate::level_definition::EffectKind::DarkSmoke,
            ),
            position,
            &inspection.presentation().particles,
        )
        .unwrap()
        .unwrap();
        let raw_handle = arena.handle(slot).unwrap();
        let raw_rng = arena.entities[usize::from(slot)].entity_rng;
        // Reproduce the actual raw/pool boundary seen in A1's 3aa trace.
        // The pool owns the address; an old raw packet still has class10/14.
        crate::world::effects::sync_position(
            &mut arena,
            &mut terrain,
            slot,
            position,
            false,
            false,
        )
        .unwrap();
        arena.economy.release_slot(slot, Some(raw_rng));
        assert!(arena.entities[usize::from(slot)].active);
        let parent =
            construct_campaign_archer(&mut arena, &mut terrain, 232, position, ARCHER_MAX_LIFE)
                .unwrap();
        assert_eq!(parent.source_entity_index, slot);
        assert!(!arena.entities[usize::from(slot)].active);
        assert_eq!(arena.entities[usize::from(slot)].class, 0);
        assert!(arena.resolve(raw_handle).is_none());
        let root_handle = arena.handle(slot).unwrap();
        assert_eq!(
            root_handle.generation,
            raw_handle.generation.wrapping_add(1)
        );
        for row in &mut arena.entities {
            row.set_control_flags(
                row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
        }
        arena.economy.exhaust_new_event_capacity_for_test();
        arena.rebuild_source_free_arena();
        let root = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        root.action = ARCHER_ACTION_KILL;
        root.source_byte_3e = 0;
        root.phase = 0;
        root.mana = 0;
        let families = arena.rebuilt_class5_model_slots();
        step_source_runtime(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &inspection.presentation().particles,
            slot,
            &families,
            &[],
            &[],
        )
        .unwrap();
        assert!(arena.resolve(root_handle).is_some());
        assert!(
            !arena
                .authored_class5()
                .iter()
                .find(|row| row.source_entity_index == slot)
                .unwrap()
                .drawing_enabled
        );
        assert_eq!(arena.economy.free_slot_count(), 0);
        // The real following-pass owner sequence unlinks, then releases.
        crate::world::effects::sync_position(
            &mut arena,
            &mut terrain,
            slot,
            position,
            false,
            false,
        )
        .unwrap();
        arena.reclaim_hidden_archer_rows_at_source_boundary();
        assert!(arena.handle(slot).is_none());
        assert!(arena.resolve(root_handle).is_none());
        assert_eq!(arena.economy.free_slot_count(), 1);
        arena.reclaim_hidden_archer_rows_at_source_boundary();
        assert_eq!(arena.economy.free_slot_count(), 1);
        let replacement =
            construct_campaign_archer(&mut arena, &mut terrain, 233, position, ARCHER_MAX_LIFE)
                .unwrap();
        assert_eq!(replacement.source_entity_index, slot);
        assert_eq!(
            arena.handle(slot).unwrap().generation,
            root_handle.generation.wrapping_add(1)
        );
    }

    #[test]
    fn archer_follow_helpers_compare_source_identity_and_live_collision_width() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let mut session = inspection.level2_reset_session().unwrap();
        let terrain = session.session.state.live.terrain.clone();
        let mut root = session
            .economy_mut()
            .allocate_authored_archer(232, archer_position())
            .unwrap();
        root.actual_speed = 0;
        // Common cadence of sub_1C310 (4/30), sub_1C560 (30), and 1D8C0 (8).
        root.source_byte_3e = 120;
        root.shift_rot_x = 256;
        let mut target = target_in_arena(
            &session.session.state.live.economy,
            root.source_entity_index,
        )
        .unwrap();
        target.entity_index = 777;
        target.position.x = root.position.x.wrapping_add(1000);
        let mut peer = target;
        peer.entity_index = 778;
        // Outside the old hard-coded 128, inside the retained collision width.
        peer.position.x = root.position.x.wrapping_add(192);
        // An earlier callback can kill/hide a member of the frozen peer list.
        // The attack helper still uses that member's geometry this pass.
        peer.life = -1;
        peer.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        let relation = |target: ArcherTarget, source_id| ArcherRelation {
            slot: target.entity_index,
            target,
            source_id,
            parent: 0,
            stage_index: 0,
            stage_mode: 0,
            stage_relation: 0,
            attacker: 0,
            yaw: 0,
            roll: 0,
            half_width: 0,
            frozen_class5_member: true,
        };
        let self_target = ArcherTarget {
            entity_index: root.source_entity_index,
            position: root.position,
            ..target
        };
        for same_identity in [true, false] {
            let relations = [
                relation(self_target, 42),
                relation(target, 43),
                relation(peer, if same_identity { 42 } else { 44 }),
            ];
            let expected_roll = if same_identity {
                heading_for_positions(root.position, target.position)
            } else {
                heading_for_positions(peer.position, root.position)
            } as i16;
            for action in [
                ARCHER_ACTION_ATTACK,
                ARCHER_ACTION_LINK,
                ARCHER_ACTION_IDLE_VISUAL,
            ] {
                let mut source = root;
                source.action = action;
                source.word_0x96_150 = target.entity_index;
                let mut word_0x32 = target.entity_index;
                let mut runtime = session.runtime.clone();
                runtime.stage_variables[1].kind = 5;
                runtime.stage_variables[1].flags = 0;
                runtime.stage_variables[1].relation =
                    RuntimeRelation::LiveEntity(target.entity_index);
                let mut context = empty_context(&terrain);
                context.target = Some(target);
                context.peers = std::slice::from_ref(&peer);
                context.relations = &relations;
                context.stage = ArcherStageState {
                    index: 1,
                    mode: 5,
                    ..ArcherStageState::default()
                };
                context.stage_runtime = Some(&mut runtime);
                match action {
                    ARCHER_ACTION_ATTACK => {
                        step_archer_action34(&mut source, &mut context);
                    }
                    ARCHER_ACTION_LINK => {
                        step_archer_action35(&mut source, &mut word_0x32, &mut context);
                    }
                    _ => {
                        step_archer_action39(&mut source, &mut context);
                    }
                }
                assert_eq!(
                    source.roll, expected_roll,
                    "action {action}, shared id {same_identity}"
                );
            }
        }
    }

    #[test]
    fn archer_guard_scan_preserves_source_axis_promotion_at_world_boundary() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let mut session = inspection.level2_reset_session().unwrap();
        let terrain = session.session.state.live.terrain.clone();
        let root = session
            .economy_mut()
            .allocate_authored_archer(232, archer_position())
            .unwrap();
        for seam_on_x in [true, false] {
            let mut source = root;
            source.position.x = 16;
            source.position.y = 16;
            source.position.z = terrain.ground_height(source.position);
            source.actual_speed = 0;
            source.action = ARCHER_ACTION_IDLE_VISUAL;
            source.source_byte_3e = ARCHER_DESCRIPTOR_CADENCE;
            source.source_byte_39 = 1;
            let mut target = target_in_arena(
                &session.session.state.live.economy,
                session.state.entity_index,
            )
            .unwrap();
            target.control_flags &= !0x20;
            target.position = source.position;
            if seam_on_x {
                target.position.x = u16::MAX - 15;
            } else {
                target.position.y = u16::MAX - 15;
            }
            source.yaw = heading_for_positions(source.position, target.position) as i16;
            let mut runtime = session.runtime.clone();
            runtime.stage_variables[1].relation = RuntimeRelation::AxisScalar(
                u32::from(source.position.x) | (u32::from(source.position.y) << 16),
            );
            let mut context = empty_context(&terrain);
            context.entities = std::slice::from_ref(&target);
            context.stage = ArcherStageState {
                index: 1,
                mode: 2,
                ..ArcherStageState::default()
            };
            context.stage_runtime = Some(&mut runtime);
            step_archer_action39(&mut source, &mut context);
            assert_eq!(
                source.action,
                if seam_on_x {
                    ARCHER_ACTION_IDLE_VISUAL
                } else {
                    ARCHER_ACTION_ATTACK
                }
            );
            assert_eq!(context.stage.mode, if seam_on_x { 2 } else { 10 });
        }
    }

    #[test]
    fn archer_death_and_arrow_reflection_read_the_retained_family_identity() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let mut session = inspection.level2_reset_session().unwrap();
        let terrain = session.session.state.live.terrain.clone();
        let root = session
            .economy_mut()
            .allocate_authored_archer(232, archer_position())
            .unwrap();
        let mut economy = session.economy().clone();
        let state = economy
            .authored_class5_archer
            .iter_mut()
            .find(|state| state.source_entity_index == root.source_entity_index)
            .unwrap();
        state.entity_id = 42;
        state.control_flags |= 0x10;
        assert_eq!(
            economy.archer_arrow_target_properties(root.source_entity_index, &[]),
            Some((5, 4, 0x14, ARCHER_MANA))
        );
        let parent = economy
            .allocate_authored_archer(
                233,
                Position {
                    x: root.position.x.wrapping_add(1000),
                    ..root.position
                },
                &terrain,
            )
            .unwrap();
        let arrow = economy
            .allocate_archer_child_packet(ArcherChildRequest {
                parent_entity_index: parent.source_entity_index,
                parent_entity_id: parent.source_entity_index,
                parent_word_0x96: root.source_entity_index,
                parent_roll: parent.roll,
                parent_position: parent.position,
                parent_fov: parent.shift_rot_y,
                target_entity_index: root.source_entity_index,
                target_class: 5,
                target_model: ARCHER_MODEL,
                target_position: root.position,
            })
            .unwrap();
        // The next projectile callback begins at the target's collision centre.
        economy
            .authored_class5_archer_children
            .last_mut()
            .unwrap()
            .position = Position {
            z: root
                .position
                .z
                .wrapping_add(ARCHER_COLLISION_VERTICAL_OFFSET),
            ..root.position
        };
        let (_, damage, reflection) = economy.step_authored_class5_archer_child_slot(
            arrow.source_entity_index,
            &terrain,
            &[],
        );
        assert!(damage.is_none());
        assert_eq!(reflection, Some(root.source_entity_index));
        let reflected = economy.authored_class5_archer_children.last().unwrap();
        assert_eq!((reflected.entity_id, reflected.subspell_index), (42, 500));
        assert!(reflected.drawing_enabled);
        let source = economy
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root.source_entity_index)
            .unwrap();
        source.action = ARCHER_ACTION_KILL;
        source.source_byte_3e = 0;
        economy
            .step_authored_archer_slot(root.source_entity_index, empty_context(&terrain), &terrain)
            .unwrap();
        assert_eq!(economy.death_bursts.last().unwrap().id_entity_index, 42);
        assert!(
            !economy
                .authored_class5
                .iter()
                .find(|source| source.source_entity_index == root.source_entity_index)
                .unwrap()
                .drawing_enabled
        );
    }

    #[test]
    fn archer_stationary_damage_and_disabled_callback_use_one_authority() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let mut session = inspection.level2_reset_session().unwrap();
        let attacker = session.state.entity_index;
        let terrain = session.session.state.live.terrain.clone();
        let root = session
            .economy_mut()
            .allocate_authored_archer(232, archer_position())
            .unwrap();
        let mut arena = session.economy().clone();
        assert!(
            !arena
                .authored_class5_runtime
                .iter()
                .any(|row| row.source_entity_index == root.source_entity_index)
        );
        let index = arena
            .authored_class5
            .iter()
            .position(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        arena.authored_class5[index].action = ARCHER_ACTION_STATIONARY;
        assert!(arena.submit_authored_archer_damage(root.source_entity_index, attacker, 20));
        assert!(arena.submit_authored_archer_damage(root.source_entity_index, attacker, 30));
        let mut context = empty_context(&terrain);
        context.pending_damage = arena.authored_class5_archer[0].pending_damage;
        let result = arena
            .step_authored_archer_slot(root.source_entity_index, context, &terrain)
            .unwrap();
        assert!(result.effects.consumed_pending_damage);
        let hit = arena.authored_class5[index];
        assert_eq!(
            (
                hit.life,
                hit.action,
                hit.word_0x96_150,
                hit.pending_word_0x62
            ),
            (950, 34, attacker, 0)
        );
        assert_eq!(
            arena.authored_class5_archer[0].pending_damage, 50,
            "source retains the consumed dword payload"
        );
        assert_eq!(hit.entity_rng, level2_next_rng16(root.entity_rng));
        assert_eq!(hit.source_byte_3e, root.source_byte_3e.wrapping_add(1));

        arena.authored_class5[index].action = ARCHER_ACTION_ATTACK_VISUAL;
        let before = arena.clone();
        assert!(
            arena
                .step_authored_archer_slot(
                    root.source_entity_index,
                    empty_context(&terrain),
                    &terrain
                )
                .is_none()
        );
        assert_eq!(
            arena, before,
            "str50[38] has no callback, RNG or phase epilogue"
        );

        arena.authored_class5[index].action = ARCHER_ACTION_IDLE_VISUAL;
        arena.authored_class5[index].dword_0x10 = 91;
        let before = arena.authored_class5[index];
        arena
            .step_authored_archer_slot(root.source_entity_index, empty_context(&terrain), &terrain)
            .unwrap();
        let after = arena.authored_class5[index];
        assert_eq!(
            (after.position, after.entity_rng, after.action),
            (before.position, before.entity_rng, 39)
        );
        assert_eq!(
            after.dword_0x10, 0,
            "20140 resets dword10 even for StageVar's no-op mode0"
        );
        assert_eq!(after.source_byte_3e, before.source_byte_3e.wrapping_add(1));
    }

    #[test]
    fn archer_stage_target_acquisition_keeps_cadence_and_post_hit_wrapper_order() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let mut session = inspection.level2_reset_session().unwrap();
        let mut root = session
            .economy_mut()
            .allocate_authored_archer(232, archer_position())
            .unwrap();
        let terrain = session.session.state.live.terrain.clone();
        let mut runtime = session.runtime.clone();
        runtime.stage_variables[1].kind = 3;
        runtime.stage_variables[1].flags = 2;
        runtime.stage_variables[1].relation = RuntimeRelation::AxisScalar(9);
        let mut target = target_in_arena(
            &session.session.state.live.economy,
            session.state.entity_index,
        )
        .unwrap();
        target.entity_index = 777;
        target.class = 5;
        target.model = 9;
        target.position = root.position;
        let relations = [ArcherRelation {
            slot: 777,
            target,
            source_id: 777,
            parent: 0,
            stage_index: 0,
            stage_mode: 0,
            stage_relation: 0,
            attacker: 0,
            yaw: 0,
            roll: 0,
            half_width: 128,
            frozen_class5_member: true,
        }];
        root.action = ARCHER_ACTION_IDLE_VISUAL;
        root.actual_speed = 0;
        root.source_byte_3e = 1;
        let mut context = empty_context(&terrain);
        context.stage = ArcherStageState {
            index: 1,
            mode: 3,
            ..ArcherStageState::default()
        };
        context.relations = &relations;
        context.stage_runtime = Some(&mut runtime);
        step_archer_action39(&mut root, &mut context);
        assert_eq!(
            context.stage.relation, 0,
            "1D8C0 does not acquire outside its eight-visit gate"
        );
        root.source_byte_3e = 8;
        step_archer_action39(&mut root, &mut context);
        assert_eq!(
            (context.stage.relation, root.action, root.word_0x96_150),
            (777, 34, 777)
        );

        root.action = ARCHER_ACTION_IDLE_VISUAL;
        root.life = 1;
        root.pending_word_0x62 = session.state.entity_index;
        context.pending_damage = 2;
        context.stage.mode = 3;
        step_archer_action39(&mut root, &mut context);
        assert_eq!(root.life, -1);
        assert_eq!(
            (root.action, root.word_0x96_150),
            (34, 777),
            "1D7C0 performs its target check after the base lethal transition"
        );
        assert!(root.pending_word_0x62 == 0);
    }

    #[test]
    fn archer_hidden_before_its_visit_executes_then_reclaims_next_pass() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let session = inspection.level2_reset_session().unwrap();
        let terrain = session.session.state.live.terrain.clone();
        let mut arena = session.economy().clone();
        let root = arena
            .allocate_authored_archer(232, archer_position(), &terrain)
            .unwrap();
        let row = arena
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        row.action = ARCHER_ACTION_IDLE_VISUAL;
        row.drawing_enabled = false;
        let phase = row.source_byte_3e;
        arena
            .step_authored_archer_slot(root.source_entity_index, empty_context(&terrain), &terrain)
            .expect("hide does not remove an enabled same-pass callback");
        let retained = arena
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == root.source_entity_index)
            .unwrap();
        assert!(!retained.drawing_enabled);
        assert_eq!(retained.source_byte_3e, phase.wrapping_add(1));
        assert!(arena.map_chains.is_linked(root.source_entity_index));
        arena.reclaim_hidden_archer_rows_at_source_boundary();
        assert!(
            !arena
                .authored_class5
                .iter()
                .any(|row| row.source_entity_index == root.source_entity_index)
        );
        assert!(
            !arena
                .authored_class5_archer
                .iter()
                .any(|row| row.source_entity_index == root.source_entity_index)
        );
        assert!(!arena.map_chains.is_linked(root.source_entity_index));
    }

    #[test]
    fn archer_house_defender_enters_real_family_and_consumes_constructor_mailbox() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let session = inspection.level2_reset_session().unwrap();
        let terrain = session.session.state.live.terrain.clone();
        let mut arena = session.economy().clone();
        let attacker = session.state.entity_index;
        let slot = arena
            .allocate_house_archer(
                1,
                attacker,
                HouseResidentRequest {
                    position: archer_position(),
                    model: 4,
                    action: 33,
                    attacker_entity_index: attacker,
                },
                &terrain,
            )
            .unwrap();
        assert!(
            !arena
                .house_residents
                .iter()
                .any(|row| row.source_entity_index == slot)
        );
        assert_eq!(
            arena.source_owner(slot),
            0,
            "the House does not copy its owner into AddArchers"
        );
        let root = arena
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!((root.action, root.pending_word_0x62), (33, attacker));
        let mut context = empty_context(&terrain);
        context.pending_damage = 1;
        arena
            .step_authored_archer_slot(slot, context, &terrain)
            .unwrap();
        let root = arena
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(
            (root.life, root.action, root.word_0x96_150),
            (999, 34, attacker)
        );
    }

    #[test]
    fn archer_arrow_flies_expires_then_reclaims_at_the_following_source_boundary() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let session = inspection.level2_reset_session().unwrap();
        let terrain = session.canonical_terrain();
        let mut economy = session.economy().clone();
        let position = Position {
            z: 20_000,
            ..archer_position()
        };
        let parent = economy
            .allocate_authored_archer(232, position, terrain)
            .unwrap();
        let packet = economy
            .allocate_archer_child_packet(ArcherChildRequest {
                parent_entity_index: parent.source_entity_index,
                parent_entity_id: parent.source_entity_index,
                parent_word_0x96: 1,
                parent_roll: 0,
                parent_position: position,
                parent_fov: 0,
                target_entity_index: 1,
                target_class: 3,
                target_model: 0,
                target_position: Position {
                    y: position.y.wrapping_sub(5000),
                    ..position
                },
            })
            .unwrap();
        let slot = packet.source_entity_index;
        let rng = economy.global_rng;
        let mut expected = packet.position;
        for remaining in (0..13).rev() {
            crate::move_entity(&mut expected, packet.yaw, packet.pitch, packet.actual_speed);
            let (live, damage, reflection) =
                economy.step_authored_class5_archer_child_slot(slot, terrain, &[]);
            assert!(reflection.is_none());
            assert!(live && damage.is_none());
            let child = economy.authored_class5_archer_children[0];
            assert_eq!(
                child.position, expected,
                "arrow keeps its ballistic altitude"
            );
            assert_eq!(child.life, remaining);
            assert!(child.drawing_enabled);
            assert_eq!(
                economy.global_rng,
                level2_next_rng32(rng),
                "launch sound draws once"
            );
        }
        let (live, damage, reflection) =
            economy.step_authored_class5_archer_child_slot(slot, terrain, &[]);
        assert!(reflection.is_none());
        let damage = damage.expect("zero lifetime emits sub_10C80 even without a direct target");
        assert!(live);
        assert_eq!(
            (damage.position, damage.payload, damage.owner_entity_index),
            (expected, 250, parent.source_entity_index)
        );
        let child = economy.authored_class5_archer_children[0];
        assert_eq!(child.life, -1);
        assert!(child.active && !child.drawing_enabled);
        assert!(
            economy.map_chains.is_linked(slot),
            "same-pass hide retains allocation"
        );
        assert_eq!(economy.events.iter().filter(|event| matches!(event, Level2EconomyEvent::PreparedSound { source_entity_index, sound_id: 33 | 34, .. } if *source_entity_index == slot)).count(), 1);
        economy.reclaim_hidden_archer_rows_at_source_boundary();
        assert!(economy.authored_class5_archer_children.is_empty());
        assert!(!economy.map_chains.is_linked(slot));
        assert!(economy.free_slots.contains(&slot));
        assert_eq!(economy.authored_class5[0].life, 1000);
    }

    #[test]
    fn archer_arrow_current_position_hit_type_filter_and_reflection() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path()).unwrap();
        let session = inspection.level2_reset_session().unwrap();
        let terrain = session.canonical_terrain();
        let mut economy = session.economy().clone();
        let mut imported = session.entities().to_vec();
        let target_slot = session.state.entity_index;
        for target in &mut imported {
            target.active = target.index == target_slot;
        }
        let target = &mut imported[usize::from(target_slot)];
        target.position = Position {
            z: 20_000,
            ..archer_position()
        };
        target.control_flags = 8;
        target.mana = 1000;
        let target_position = target.position;
        let target_height = target.array_fov();
        let position = Position {
            z: target.position.z.wrapping_add(target.array_yaw()),
            ..target.position
        };
        economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: target_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .unwrap();
        let parent = economy
            .allocate_authored_archer(232, position, terrain)
            .unwrap();
        let packet = economy
            .allocate_archer_child_packet(ArcherChildRequest {
                parent_entity_index: parent.source_entity_index,
                parent_entity_id: parent.source_entity_index,
                parent_word_0x96: target_slot,
                parent_roll: 0,
                parent_position: position,
                parent_fov: 0,
                target_entity_index: target_slot,
                target_class: 3,
                target_model: 0,
                target_position: Position {
                    y: position.y.wrapping_sub(5000),
                    ..position
                },
            })
            .unwrap();
        let slot = packet.source_entity_index;
        let baseline = economy.clone();
        let (live, damage, reflection) =
            economy.step_authored_class5_archer_child_slot(slot, terrain, &imported);
        assert!(live && reflection.is_none());
        let damage = damage.expect("current-position overlap is hit before movement");
        assert_eq!((damage.position, damage.payload), (position, 250));
        assert!(!economy.authored_class5_archer_children[0].drawing_enabled);

        let mut wrong_type = baseline.clone();
        wrong_type.authored_class5_archer_children[0].xtype = 5;
        let (_, damage, reflection) =
            wrong_type.step_authored_class5_archer_child_slot(slot, terrain, &imported);
        assert!(
            damage.is_none() && reflection.is_none(),
            "sub_10780 must reject the wrong class even at the same position"
        );
        assert!(wrong_type.authored_class5_archer_children[0].drawing_enabled);

        imported[usize::from(target_slot)].control_flags |= 0x10;
        let mut reflected = baseline.clone();
        let (_, damage, reflection) =
            reflected.step_authored_class5_archer_child_slot(slot, terrain, &imported);
        assert!(damage.is_none());
        assert_eq!(reflection, Some(target_slot));
        let arrow = reflected.authored_class5_archer_children[0];
        assert_eq!(
            (
                arrow.entity_id,
                arrow.word_0x96,
                arrow.xtype,
                arrow.xsubtype
            ),
            (target_slot, parent.source_entity_index, 5, ARCHER_MODEL)
        );
        assert_eq!(
            (arrow.life, arrow.subspell_index, arrow.yaw),
            (13, 500, packet.yaw.wrapping_add(1024) & 2047)
        );
        assert_eq!(
            arrow.entity_rng, packet.entity_rng,
            "strong reflection does not draw entity RNG"
        );
        assert_eq!(
            arrow.position.z,
            target_position.z.wrapping_add(target_height)
        );
        assert!(arrow.drawing_enabled);

        imported[usize::from(target_slot)].control_flags = 8 | 0x8000;
        let mut deflected = baseline.clone();
        let (_, damage, reflection) =
            deflected.step_authored_class5_archer_child_slot(slot, terrain, &imported);
        assert!(damage.is_none() && reflection.is_some());
        let arrow = deflected.authored_class5_archer_children[0];
        assert_eq!(arrow.entity_rng, level2_next_rng16(packet.entity_rng));
        assert_eq!(arrow.subspell_index, 250);
        assert_eq!(
            arrow.yaw,
            arrow
                .roll
                .wrapping_add((arrow.entity_rng % 45) as i16)
                .wrapping_sub(22)
        );

        let mut buried = baseline;
        let child = &mut buried.authored_class5_archer_children[0];
        child.position.z = -1000;
        child.pitch = 0;
        let (_, damage, reflection) =
            buried.step_authored_class5_archer_child_slot(slot, terrain, &[]);
        assert!(damage.is_some() && reflection.is_none());
        assert_eq!(
            buried.authored_class5_archer_children[0].life, 13,
            "terrain collision does not post-decrement lifetime"
        );
    }

    #[test]
    fn archer_rows_are_the_complete_group20_source_order() {
        assert_eq!(ARCHER_AUTHORED_ROWS, [232, 233, 234, 235]);
        assert!(
            ARCHER_AUTHORED_ROWS
                .windows(2)
                .all(|rows| rows[1] == rows[0].wrapping_add(1))
        );
        assert!(
            ARCHER_AUTHORED_ROWS
                .iter()
                .copied()
                .all(is_authored_archer_row)
        );
        assert!(!is_authored_archer_row(231));
        assert!(!is_authored_archer_row(236));
    }

    #[test]
    fn archer_constructor_payload_preserves_pose_rng_and_descriptor_cadence() {
        let slot = 147_u16;
        let initial_rng = 0x1357_u16;
        let entity_rng = initial_rng.wrapping_mul(9_377).wrapping_add(9_439);
        let payload = archer_constructor_payload(
            slot,
            initial_rng,
            232,
            3,
            archer_position(),
            ARCHER_MAX_LIFE,
        );
        assert_eq!(payload.entity_rng, entity_rng);
        assert_eq!(payload.initial_dword_0x10, 47);
        assert_eq!(payload.dword_0x10, 47);
        assert_eq!(payload.source_class, ARCHER_SOURCE_CLASS);
        assert_eq!(payload.model, ARCHER_MODEL);
        assert_eq!(payload.action, ARCHER_ACTION);
        assert_eq!(
            (
                payload.minimum_speed,
                payload.maximum_speed,
                payload.actual_speed,
                payload.max_life,
                payload.life,
                payload.mana,
                payload.max_mana,
                payload.subspell_index,
            ),
            (30, 0, 30, 1_000, 1_000, 500, 0, 500)
        );
        assert_eq!(payload.byte_0x38, 1);
        assert_eq!(payload.descriptor_index, 75);
        assert_eq!(payload.source_byte_39, 31);
        assert_eq!(payload.xtype, 3);
        assert_eq!(payload.source_byte_3e, 3);
        assert_eq!(payload.particle_index, 0);
        assert_eq!(
            (payload.yaw, payload.pitch, payload.roll, payload.fov),
            (661, 661, 661, 0)
        );
        let other = archer_constructor_payload(slot, 0, 232, 3, archer_position(), ARCHER_MAX_LIFE);
        assert_eq!(
            (other.yaw, other.pitch, other.roll, other.fov),
            (1246, 1246, 1246, 0)
        );
        assert_ne!(
            payload.yaw, other.yaw,
            "particle dimensions must not erase seeded facing"
        );
        assert_eq!((other.shift_rot_x, other.shift_rot_y), (128, 256));
        assert_eq!((payload.shift_rot_x, payload.shift_rot_y), (128, 256));
        assert!(payload.active && payload.drawing_enabled);
    }

    #[test]
    fn archer_action_packets_and_allocator_branches() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Archer model-local session");
        let baseline_hash = session.campaign_state_hash();

        let mut rows = Vec::new();
        for authored_index in ARCHER_AUTHORED_ROWS {
            rows.push(
                session
                    .economy_mut()
                    .allocate_authored_archer(authored_index, archer_position())
                    .expect("every authored Archer row allocates"),
            );
        }
        assert_eq!(
            rows.iter()
                .map(|source| source.authored_index)
                .collect::<Vec<_>>(),
            ARCHER_AUTHORED_ROWS
        );
        assert!(rows.iter().all(|source| {
            source.model == ARCHER_MODEL
                && source.action == ARCHER_ACTION
                && source.minimum_speed == ARCHER_MINIMUM_SPEED
                && source.maximum_speed == ARCHER_MAXIMUM_SPEED
                && source.actual_speed == ARCHER_MINIMUM_SPEED
                && source.max_life == ARCHER_MAX_LIFE
                && source.life == ARCHER_MAX_LIFE
                && source.mana == ARCHER_MANA
                && source.max_mana == ARCHER_MAX_MANA
                && source.subspell_index == ARCHER_SUBSPELL_INDEX
                && source.particle_index == ARCHER_PARTICLE_INDEX
                && source.yaw == (source.entity_rng & 0x7ff) as i16 - 1
                && source.pitch == source.yaw
                && source.roll == source.yaw
                && source.fov == 0
                && source.shift_rot_x == ARCHER_SHIFT_ROT_X
                && source.shift_rot_y == ARCHER_SHIFT_ROT_Y
                && source.active
                && source.drawing_enabled
        }));
        assert_eq!(session.economy().authored_class5_archer().len(), 4);
        assert!(
            session
                .economy()
                .authored_class5_archer()
                .iter()
                .all(|state| state.control_flags == 12)
        );
        let collision_targets = session.economy().fireball_collision_targets(&[]);
        let collision = collision_targets
            .iter()
            .find(|target| target.source_slot == rows[0].source_entity_index)
            .unwrap();
        assert_eq!(
            (
                collision.half_width,
                collision.vertical_offset,
                collision.half_height
            ),
            (128, 125, 256)
        );
        let upper_hit = Position {
            z: collision.position.z + 380,
            ..collision.position
        };
        assert!(crate::level1_mission::source_entities_overlap(
            upper_hit,
            1,
            0,
            1,
            collision.position,
            collision.half_width,
            collision.vertical_offset,
            collision.half_height,
        ));
        assert!(!crate::level1_mission::source_entities_overlap(
            Position {
                z: upper_hit.z + 2,
                ..upper_hit
            },
            1,
            0,
            1,
            collision.position,
            collision.half_width,
            collision.vertical_offset,
            collision.half_height,
        ));
        assert!(
            session
                .economy()
                .authored_class5_archer()
                .iter()
                .all(|state| {
                    state.byte_0x38 == 1
                        && state.xtype == 3
                        && state.descriptor_index == ARCHER_DESCRIPTOR_INDEX
                        && state.pending_damage == 0
                        && state.word_0x34 == 0
                })
        );

        let no_slot_phase = session.economy().class5_model4_constructor_phase;
        let no_slot_rng = session.economy().source_rng.clone();
        session.economy_mut().free_slots.clear();
        assert!(
            session
                .economy_mut()
                .allocate_authored_archer(235, archer_position())
                .is_none()
        );
        assert_eq!(
            session.economy().class5_model4_constructor_phase,
            no_slot_phase
        );
        assert_eq!(session.economy().source_rng, no_slot_rng);

        let mut source = rows[0];
        let source_before = source;
        source.source_byte_3e = 1;
        let mut word_0x32 = 0;
        let mut context = empty_context(&session.session.state.live.terrain);
        let effects = step_archer_action33(&mut source, &mut word_0x32, &mut context);
        assert!(effects.moved);
        assert_eq!(source.action, ARCHER_ACTION);
        assert_eq!(source.entity_rng, source_before.entity_rng);
        assert_eq!(source.roll, source_before.roll);
        assert_eq!(word_0x32, 0);

        let target = ArcherTarget {
            entity_index: 777,
            class: 3,
            model: 0,
            action: 0,
            active: true,
            life: 1_000,
            control_flags: 0,
            minimum_speed: 0,
            maximum_speed: 0,
            actual_speed: 0,
            dword_0x10: 0,
            word_0x248: 0,
            word_0x32: 0,
            word_0x96_150: 0,
            position: source.position,
        };
        let mut attack_source = source_before;
        attack_source.action = ARCHER_ACTION_ATTACK;
        attack_source.source_byte_3e = ARCHER_DESCRIPTOR_CADENCE;
        attack_source.word_0x96_150 = target.entity_index;
        let mut attack_context = ArcherActionContext {
            terrain: &session.session.state.live.terrain,
            control_flags: 4,
            pending_damage: 0,
            linked_head: 0,
            linked: &[],
            target: Some(target),
            entities: &[],
            fallback_entities: &[],
            peers: &[],
            relations: &[],
            stage: ArcherStageState::default(),
            stage_runtime: None,
        };
        let attack_effects = step_archer_action34(&mut attack_source, &mut attack_context);
        let request = attack_effects
            .child_request
            .expect("action-34 cadence emits Archer child request");
        assert_eq!(request.target_entity_index, target.entity_index);
        assert_eq!(request.target_class, target.class);
        assert_eq!(request.target_model, target.model);
        assert_eq!(
            attack_effects.target_feedback_entity,
            Some(target.entity_index)
        );

        let child = archer_child_packet(900, 0x2468, request);
        assert_eq!(child.entity_id, attack_source.source_entity_index);
        assert_eq!(child.parent_entity_index, attack_source.source_entity_index);
        assert_eq!(child.entity_rng, 0x2468);
        assert_eq!(
            (
                child.source_class,
                child.model,
                child.action,
                child.minimum_speed,
                child.actual_speed,
                child.max_life,
                child.life,
                child.subspell_index,
                child.particle_index,
            ),
            (9, 13, 13, 384, 384, 13, 13, 250, 195)
        );
        assert_eq!(child.word_0x96, request.parent_word_0x96);
        assert_eq!((child.xtype, child.xsubtype), (target.class, target.model));
        assert_eq!(child.fov, ARCHER_CHILD_PARTICLE_FOV);
        assert_eq!(child.position.z, attack_source.position.z + 128);
        assert!(child.active && child.drawing_enabled);

        // A full source arena must refuse the child before touching source
        // RNG/map state. The shared carrier remains the integration owner.
        let child_no_slot_rng = session.economy().source_rng.clone();
        assert!(
            session
                .economy_mut()
                .allocate_archer_child_packet(request)
                .is_none()
        );
        assert_eq!(session.economy().source_rng, child_no_slot_rng);

        session.reset();
        assert!(session.economy().authored_class5().is_empty());
        assert_eq!(session.campaign_state_hash(), baseline_hash);
    }
}
