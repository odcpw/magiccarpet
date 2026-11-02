//! Authored class-5/model-0 Dragon construction, body following, damage
//! propagation, roaming, and player attack.
//!
//! Dragons use the shared class-5 root and NewEvent arena. This module owns
//! only the model-0 constructor sidecar, its sixteen action-232 body rows,
//! and the source-shaped undamaged action-1 callback reached by Level 3.

use super::super::*;
use crate::campaign::source_arena::{CreatureRootOrigin, SourceArena, SourceEntityHandle};
use crate::goat::GoatPlayerTarget;

pub(crate) const DRAGON_MODEL: u8 = 0;
/// NewEvent's source flags, copied from the root into every body row.
const DRAGON_CONSTRUCTOR_FLAGS: u32 = 8;
pub(crate) const DRAGON_ROAM_ACTION: u8 = 1;
pub(crate) const DRAGON_ATTACK_ACTION: u8 = 2;
pub(crate) const DRAGON_LINK_ACTION: u8 = 3;
pub(crate) const DRAGON_STAGE_ACTION: u8 = 7;
pub(crate) const DRAGON_PRE_KILL_ACTION: u8 = 4;
pub(crate) const DRAGON_KILL_ACTION: u8 = 5;
pub(crate) const DRAGON_PART_ACTION: u8 = 232;
pub(crate) const DRAGON_PART_COUNT: usize = 16;
pub(crate) const DRAGON_DESCRIPTOR: u16 = 71;
pub(crate) const DRAGON_ROOT_PARTICLE: u16 = 40;
pub(crate) const DRAGON_MINIMUM_SPEED: i16 = 80;
pub(crate) const DRAGON_MAXIMUM_SPEED: i16 = 16;
pub(crate) const DRAGON_ACTUAL_SPEED: i16 = 30;
pub(crate) const DRAGON_LIFE: i32 = 4_000;
pub(crate) const DRAGON_MAX_MANA: i32 = 4_500;

/// Current class9 carrier fields read by source sub_1F0C0. The caller
/// projects raw and typed projectiles before the Dragon's actual visit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DragonProjectileThreat {
    pub source_slot: u16,
    pub target: u16,
    pub life: i32,
    pub flags: u32,
    pub yaw: i16,
}

/// Read the current target carrier for sub_1C310. The target can be any
/// source actor, not only a player; zero-life actors remain valid here.
pub(crate) fn current_attack_target(
    arena: &SourceArena,
    context: crate::native_world::source_target::Context<'_>,
    target_slot: u16,
) -> std::result::Result<Option<GoatPlayerTarget>, String> {
    if target_slot == 0 {
        return Ok(None);
    }
    Ok(crate::native_world::source_target::read(arena, context, target_slot)?
        .filter(|target| target.life >= 0 && !target.drawing_disabled())
        .map(|target| GoatPlayerTarget {
            entity_index: target_slot,
            position: target.position,
            control_flags: target.control_flags,
        }))
}

/// `sub_1C310` consumes damage and moves before it reads word96's current
/// fixed-address actor. Keep that order even when the target is the Dragon.
pub(crate) fn step_attack_in_arena(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
    frozen_model0_slots: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<DragonAttackStep, String> {
    let source_index = arena.economy.authored_class5.iter().position(|source| {
        source.active && source.source_entity_index == source_slot
            && source.model == DRAGON_MODEL && source.action == DRAGON_ATTACK_ACTION
    }).ok_or_else(|| format!("Dragon {source_slot} lost its attack source"))?;
    let state_index = arena.economy.authored_dragons.iter()
        .position(|state| state.source_entity_index == source_slot)
        .ok_or_else(|| format!("Dragon {source_slot} lost its sidecar"))?;
    let runtime_index = arena.economy.authored_class5_runtime.iter()
        .position(|state| state.source_entity_index == source_slot)
        .ok_or_else(|| format!("Dragon {source_slot} lost its class-5 runtime"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_dragons[state_index];
    let mut runtime = arena.economy.authored_class5_runtime[runtime_index];
    let damaged = consume_dragon_damage(&mut source, &mut runtime,
        &arena.economy.authored_dragon_parts, terrain);
    if !damaged && step_dragon_motion(&mut source, &mut state, terrain) {
        arena.economy.decrement_source_creature_population();
    }
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_dragons[state_index] = state;
    arena.economy.authored_class5_runtime[runtime_index] = runtime;
    crate::world::effects::sync_position(arena, terrain, source_slot, source.position, true, false)?;
    if damaged {
        return Ok(DragonAttackStep { live: true, fireball: None });
    }
    let allowed = alliance_attack_target_allowed(arena, source_slot, source.word_0x96_150, context)?;
    let target = if allowed {
        current_attack_target(arena, context, source.word_0x96_150)?
    } else { None };
    arena.economy.step_authored_dragon_attack_slot(
        source_slot, target, frozen_model0_slots, terrain, true,
    )
}
pub(crate) const DRAGON_INITIAL_MANA: i32 = 2_250;

/// Live Dragon fields read and written by the Alliance terminal. The raw
/// arena packet is only a projection; movement and stage remain typed here.
pub(crate) fn alliance_projection(arena: &SourceArena, slot: u16) -> Option<EntitySnapshot> {
    let handle = arena.handle(slot)?;
    if !matches!(arena.resolve(handle), Some(crate::campaign::source_arena::SourceEntityView::Dynamic(
        Level2DynamicArenaKind::AuthoredClass5
    ))) { return None; }
    let source = arena.economy.authored_class5.iter().find(|row|
        row.active && row.source_entity_index == slot && row.model == DRAGON_MODEL)?;
    let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot)?;
    let mut row = arena.entities.get(usize::from(slot))?.clone();
    row.active = true;
    row.record[2] = 1;
    row.source_id = state.entity_id;
    row.record[30..32].copy_from_slice(&state.entity_id.to_le_bytes());
    row.class = 5;
    row.model = DRAGON_MODEL;
    row.record[67] = 5;
    row.record[68] = DRAGON_MODEL;
    row.set_source_action(source.action);
    row.set_position(source.position);
    row.set_life(source.life);
    row.set_control_flags((state.control_flags & !0x400) | if source.drawing_enabled { 0 } else { 0x400 });
    row.record[44..46].copy_from_slice(&state.parent_entity_index.to_le_bytes());
    row.record[50..52].copy_from_slice(&state.stage_remaining.to_le_bytes());
    row.record[52..54].copy_from_slice(&state.alliance_duration.to_le_bytes());
    row.record[76] = state.stage_var1;
    row.record[77] = state.stage_var2;
    row.record[78..80].copy_from_slice(&state.stage_relation.to_le_bytes());
    row.record[154..156].copy_from_slice(&source.word_0x96_150.to_le_bytes());
    Some(row)
}

pub(crate) fn apply_alliance_takeover(
    arena: &mut SourceArena,
    child: SourceEntityHandle,
    row: &EntitySnapshot,
    parent: Option<SourceEntityHandle>,
) -> std::result::Result<bool, String> {
    let slot = child.source_slot;
    if row.index != slot || (row.class, row.model) != (5, DRAGON_MODEL)
        || row.record[76] != 0 || row.record[77] != 14
        || arena.handle(slot) != Some(child)
        || alliance_projection(arena, slot).is_none_or(|current| current.source_id != row.source_id)
    { return Ok(false); }
    let parent_slot = u16::from_le_bytes([row.record[44], row.record[45]]);
    let generation = parent.filter(|handle| handle.source_slot == parent_slot)
        .map_or_else(|| arena.slot_generation(parent_slot), |handle| handle.generation);
    let source = arena.economy.authored_class5.iter_mut().find(|source|
        source.active && source.source_entity_index == slot && source.model == DRAGON_MODEL)
        .ok_or_else(|| format!("Alliance Dragon {slot} lost its typed source"))?;
    source.action = row.action;
    source.word_0x96_150 = row.source_word_96();
    let state = arena.economy.authored_dragons.iter_mut().find(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("Alliance Dragon {slot} lost its sidecar"))?;
    state.parent_entity_index = parent_slot;
    state.parent_generation = generation;
    state.stage_remaining = i16::from_le_bytes([row.record[50], row.record[51]]);
    state.alliance_duration = u16::from_le_bytes([row.record[52], row.record[53]]);
    state.stage_var1 = row.record[76];
    state.stage_var2 = row.record[77];
    state.stage_relation = u16::from_le_bytes([row.record[78], row.record[79]]);
    if let Some(parent) = parent.filter(|handle| handle.source_slot == parent_slot) {
        if !arena.bind_creature_parent(child, parent) {
            return Err(format!("Alliance Dragon {slot} lost its parent binding"));
        }
    }
    Ok(true)
}

/// sub_1ED30 blinks and consumes Alliance time during the ordinary attack
/// callback, then rejects the parent and targets other than its selection.
fn alliance_attack_target_allowed(
    arena: &mut SourceArena,
    slot: u16,
    target_slot: u16,
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<bool, String> {
    let index = arena.economy.authored_dragons.iter().position(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("Dragon {slot} lost its attack sidecar"))?;
    let mut state = arena.economy.authored_dragons[index];
    if state.stage_var2 != 14 { return Ok(true); }
    let phase = arena.economy.authored_class5.iter().find(|row|
        row.active && row.source_entity_index == slot && row.model == DRAGON_MODEL)
        .ok_or_else(|| format!("Alliance Dragon {slot} lost its attack source"))?.source_byte_3e;
    let flags = super::firefly::alliance_blink_bits(
        state.stage_remaining.cast_unsigned(), state.alliance_duration, phase, false,
    );
    super::firefly::alliance_blink_children(arena, slot, flags, context)?;
    state.control_flags = (state.control_flags & !0x0600_0000) | flags;
    state.stage_remaining = state.stage_remaining.wrapping_sub(1);
    arena.economy.authored_dragons[index] = state;
    if state.stage_remaining <= 0 { return Ok(false); }
    if state.parent_entity_index == 0 { return Ok(true); }
    let Some(parent) = crate::native_world::source_target::read(arena, context, state.parent_entity_index)?
        else { return Ok(true); };
    if target_slot == state.parent_entity_index { return Ok(false); }
    let selected = parent.relations.target.ok_or_else(|| format!("Alliance Dragon parent {} has no target", state.parent_entity_index))?;
    let selected = if selected != 0 { selected } else {
        parent.relations.attacker.ok_or_else(|| format!("Alliance Dragon parent {} has no attacker", state.parent_entity_index))?
    };
    Ok(selected == 0 || selected == target_slot)
}

pub(crate) const DRAGON_SEGMENT_MANA: i32 = DRAGON_MAX_MANA >> 5;
pub(crate) const DRAGON_SUBSPELL_INDEX: u16 = 100;
const DRAGON_DESCRIPTOR_CADENCE: u8 = 30;
const DRAGON_TURN_LIMIT: i16 = 34;
const DRAGON_GROUND_CLEARANCE: i16 = 0;
const DRAGON_DESCENT: i16 = -4;
const DRAGON_MAX_TILE_SLOPE: i32 = 30;
const DRAGON_RANGE: u16 = 5_120;
const DRAGON_VIEW_HALF_ANGLE: i16 = 170;
const DRAGON_RENDER_SPEED: i16 = 342;
const DRAGON_RENDER_ROTATION: i16 = 280;
const DRAGON_FIREBALL_PAYLOAD: u16 = 500;
const DRAGON_PART_SPEEDS: [i16; DRAGON_PART_COUNT] = [
    196, 215, 241, 261, 274, 274, 261, 248, 222, 209, 196, 170, 143, 117, 91, 78,
];
const DRAGON_PART_ROTATIONS: [i16; DRAGON_PART_COUNT] = [
    150, 165, 185, 200, 210, 210, 200, 190, 170, 160, 150, 130, 110, 90, 70, 60,
];

/// Construct the boss's real Dragon and its sixteen body segments, then apply
/// the post-constructor launch fields from `sub_21AB0`.
pub(crate) fn allocate_boss_summoned(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    position: Position,
    parent: SourceEntityHandle,
    owner_id: u16,
    target_slot: u16,
    yaw: i16,
) -> std::result::Result<Option<SourceEntityHandle>, String> {
    if arena.resolve(parent).is_none() {
        return Err("boss Dragon parent generation is no longer live".to_owned());
    }
    let Some(source) =
        arena
            .economy
            .allocate_authored_dragon(u16::MAX, position, DRAGON_LIFE, terrain)
    else {
        return Ok(None);
    };
    let slot = source.source_entity_index;
    // The boss constructs at its elevated launch point, not at terrain
    // height. Apply it to the root and copied body rows before publication.
    if let Some(root) = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
    {
        root.position = position;
    }
    for part in arena
        .economy
        .authored_dragon_parts
        .iter_mut()
        .filter(|part| part.active && part.entity_id == slot)
    {
        part.position = position;
    }
    // The economy constructor owns the real slots and map chains. Publish
    // those same occupants into the shared terrain map and clear any stale
    // raw packet left at a reused address.
    let mut occupied = arena
        .economy
        .authored_dragon_parts
        .iter()
        .filter(|part| part.active && part.entity_id == slot)
        .map(|part| (part.source_entity_index, part.position))
        .collect::<Vec<_>>();
    occupied.push((slot, position));
    for (occupied_slot, occupied_position) in occupied {
        arena.entities[usize::from(occupied_slot)].reclaim_source_row();
        crate::world::effects::sync_position(
            arena,
            terrain,
            occupied_slot,
            occupied_position,
            true,
            false,
        )?;
    }
    let handle = arena
        .handle(slot)
        .ok_or_else(|| format!("boss Dragon constructor lost source slot {slot}"))?;
    let source = arena
        .economy
        .authored_class5
        .iter_mut()
        .find(|row| row.active && row.source_entity_index == slot && row.model == DRAGON_MODEL)
        .ok_or_else(|| format!("boss Dragon constructor lost typed root {slot}"))?;
    source.action = DRAGON_STAGE_ACTION;
    source.word_0x96_150 = target_slot;
    source.actual_speed = 320;
    source.yaw = yaw;
    source.roll = yaw;
    let state = arena
        .economy
        .authored_dragons
        .iter_mut()
        .find(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("boss Dragon constructor lost runtime sidecar {slot}"))?;
    state.stage_var2 = 17;
    state.parent_entity_index = parent.source_slot;
    state.parent_generation = parent.generation;
    state.stage_remaining = 250;
    state.entity_id = owner_id;
    if !arena.bind_dynamic_creature_origin(
        handle,
        crate::level_definition::CreatureKind::Dragon,
        CreatureRootOrigin::CreatureConversion {
            producer_slot: parent.source_slot,
        },
    ) {
        return Err(format!(
            "boss Dragon {slot} lost its generation-bound origin"
        ));
    }
    if !arena.bind_carried_creature_parent(handle, parent) {
        return Err(format!("boss Dragon {slot} lost its parent generation binding"));
    }
    Ok(Some(handle))
}

#[derive(Default)]
pub(crate) struct DragonBossStageStep {
    pub(crate) retained_for_epilogue: bool,
    pub(crate) spell_feedback: Option<(u16, u8)>,
}

#[derive(Default)]
pub(crate) struct DragonAllianceStep {
    pub(crate) retained_for_epilogue: bool,
    pub(crate) spell_feedback: Vec<(u16, u8)>,
}

/// Source sub_1D5D0 -> sub_1E9C0 followed by sub_1F300's
/// projectile-dodge and vertical tails, against the same typed Dragon root.
pub(crate) fn step_alliance_stage_slot(
    arena: &mut SourceArena,
    slot: u16,
    frozen_model0_slots: &[u16],
    projectiles: &[DragonProjectileThreat],
    terrain: &mut SessionTerrain,
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<DragonAllianceStep, String> {
    let source_index = arena.economy.authored_class5.iter().position(|row|
        row.active && row.source_entity_index == slot && row.model == DRAGON_MODEL
            && row.action == DRAGON_STAGE_ACTION)
        .ok_or_else(|| format!("Alliance Dragon {slot} lost its stage source"))?;
    let state_index = arena.economy.authored_dragons.iter().position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("Alliance Dragon {slot} lost its sidecar"))?;
    let runtime_index = arena.economy.authored_class5_runtime.iter().position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("Alliance Dragon {slot} lost its class5 runtime"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_dragons[state_index];
    let mut runtime = arena.economy.authored_class5_runtime[runtime_index];
    if state.stage_var2 != 14 { return Err(format!("Dragon {slot} is not in Alliance stage14")); }
    state.stage_remaining = state.stage_remaining.wrapping_sub(1);
    if state.stage_remaining > 0 {
        let flags = super::firefly::alliance_blink_bits(
            state.stage_remaining.cast_unsigned(), state.alliance_duration, source.source_byte_3e, false,
        );
        state.control_flags = (state.control_flags & !0x0600_0000) | flags;
        super::firefly::alliance_blink_children(arena, slot, flags, context)?;
    }
    let parent = if state.stage_remaining > 0 && state.parent_entity_index != 0 {
        crate::native_world::source_target::read(arena, context, state.parent_entity_index)?
            .filter(|row| row.life > 0)
    } else { None };
    let mut feedback = Vec::new();
    if let Some(parent) = parent {
        let saved_target = source.word_0x96_150;
        source.word_0x96_150 = state.parent_entity_index;
        let mut damaged = false;
        if source.pending_word_0x62 != 0 {
            source.life = source.life.wrapping_sub(runtime.pending_damage);
            source.word_0x26 = source.pending_word_0x62;
            source.pending_word_0x62 = 0;
            runtime.pending_damage = 0;
            damaged = true;
        } else { source.word_0x26 = 0; }
        let mut child = runtime.linked_head;
        let mut remaining = arena.entities.len();
        while child != 0 {
            if remaining == 0 { return Err(format!("Alliance Dragon {slot} has a cyclic child chain")); }
            remaining -= 1;
            let Some(view) = crate::native_world::source_target::read(arena, context, child)? else { break; };
            if view.life < source.life {
                source.life = view.life;
                source.word_0x26 = view.relations.attacker.ok_or_else(|| format!("Alliance Dragon child {child} has no attacker"))?;
                damaged = true;
                break;
            }
            child = view.relations.next.ok_or_else(|| format!("Alliance Dragon child {child} has no next link"))?;
        }
        if source.life < 0 {
            source.word_0x24 = source.word_0x26;
            state.stage_remaining = 1;
        } else {
            if step_dragon_motion(&mut source, &mut state, terrain) {
                arena.economy.decrement_source_creature_population();
            }
            if damaged {
                let attacker = source.word_0x26;
                let same_family = crate::native_world::source_target::read(arena, context, attacker)?
                    .is_some_and(|row| (row.class, row.model) == (5, DRAGON_MODEL));
                if attacker != state.parent_entity_index && !same_family {
                    source.word_0x96_150 = attacker;
                    source.action = DRAGON_ATTACK_ACTION;
                    feedback.push((state.parent_entity_index, 19));
                }
            } else if source.source_byte_3e & 7 == 0 {
                if !state.movement_blocked {
                    source.roll = i16::try_from(crate::heading_for_positions(source.position, parent.position))
                        .expect("Alliance Dragon parent heading fits i16");
                    if source.source_byte_3e & 0x3f == 0 { apply_dragon_wander(&mut source); }
                }
                if let Some(peer) = frozen_model0_slots.iter().find_map(|peer_slot| {
                    arena.economy.authored_class5.iter().find(|peer| {
                        peer.active && peer.model == DRAGON_MODEL && peer.source_entity_index == *peer_slot
                            && arena.economy.authored_dragons.iter().any(|other|
                                other.source_entity_index == *peer_slot && other.entity_id != state.entity_id)
                            && i32::from(peer.position.x.wrapping_sub(source.position.x).cast_signed()).abs() < i32::from(source.shift_rot_x)
                            && i32::from(peer.position.y.wrapping_sub(source.position.y).cast_signed()).abs() < i32::from(source.shift_rot_x)
                    })
                }) {
                    source.roll = i16::try_from(crate::heading_for_positions(peer.position, source.position))
                        .expect("Alliance Dragon separation heading fits i16");
                }
            }
        }
        let parent_target = parent.relations.target.ok_or_else(|| format!("Alliance Dragon parent {} has no target", state.parent_entity_index))?;
        let inherited = if parent_target != 0 { parent_target } else {
            parent.relations.attacker.ok_or_else(|| format!("Alliance Dragon parent {} has no attacker", state.parent_entity_index))?
        };
        source.word_0x96_150 = if inherited != 0 { inherited } else { saved_target };
        if source.word_0x96_150 != 0 {
            if let Some(target) = crate::native_world::source_target::read(arena, context, source.word_0x96_150)? {
                let allied = target.relations.parent == Some(state.parent_entity_index)
                    && target.relations.stage_mode == Some(14);
                if allied || target.life <= 0 || target.drawing_disabled() {
                    source.word_0x96_150 = 0;
                } else if distance_3d_wrapped(source.position, target.position) < u32::from(DRAGON_RANGE) {
                    source.action = DRAGON_ATTACK_ACTION;
                    feedback.push((state.parent_entity_index, 24));
                }
            }
        }
    } else {
        super::firefly::alliance_blink_children(arena, slot, 0, context)?;
        state.stage_var2 = 10;
        state.stage_var1 = state.stage_relation as u8;
        state.parent_entity_index = 0;
        state.parent_generation = 0;
        state.control_flags &= !0x0600_0000;
        source.word_0x96_150 = 0;
    }
    step_dragon_tails(&mut source, &mut state, terrain, &arena.economy.map_chains, projectiles);
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_dragons[state_index] = state;
    arena.economy.authored_class5_runtime[runtime_index] = runtime;
    crate::world::effects::sync_position(arena, terrain, slot, source.position, source.active, false)?;
    Ok(DragonAllianceStep { retained_for_epilogue: true, spell_feedback: feedback })
}

/// Action 7 for the boss-launched Dragon. The parent binding is generation
/// checked before mode 16 can follow it, even when its numeric slot is reused.
pub(crate) fn step_boss_stage_slot(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    players: &[GoatPlayerTarget],
    class3_slots: &[u16],
    frozen_model0_slots: &[u16],
    projectiles: &[DragonProjectileThreat],
    target_context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<DragonBossStageStep, String> {
    let Some(source_index) = arena.economy.authored_class5.iter().position(|row| {
        row.active
            && row.drawing_enabled
            && row.source_entity_index == slot
            && row.model == DRAGON_MODEL
            && row.action == DRAGON_STAGE_ACTION
    }) else {
        return Ok(DragonBossStageStep::default());
    };
    let state_index = arena
        .economy
        .authored_dragons
        .iter()
        .position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("boss Dragon {slot} has no sidecar"))?;
    let runtime_index = arena
        .economy
        .authored_class5_runtime
        .iter()
        .position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("boss Dragon {slot} has no class-5 runtime"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_dragons[state_index];
    let mut runtime = arena.economy.authored_class5_runtime[runtime_index];
    let entered_handle = arena
        .handle(slot)
        .ok_or_else(|| format!("boss Dragon {slot} lost its source generation"))?;
    if !matches!(state.stage_var2, 16 | 17) {
        return Err(format!(
            "boss Dragon {slot} has unsupported stage {}",
            state.stage_var2
        ));
    }
    let target_position = |slot: u16, arena: &SourceArena, allow_zero: bool| -> std::result::Result<Option<Position>, String> {
        let Some(row) = crate::native_world::source_target::read(arena, target_context, slot)? else {
            return Ok(None);
        };
        Ok((slot != 0
            && row.active
            && (row.life > 0 || allow_zero && row.life == 0)
            && !row.drawing_disabled())
        .then(|| {
            players
                .iter()
                .find(|player| player.entity_index == slot)
                .map_or(row.position, |player| player.position)
        }))
    };
    let mut spell_feedback = None;
    if state.stage_var2 == 17 {
        if step_dragon_motion(&mut source, &mut state, terrain) {
            arena.economy.decrement_source_creature_population();
        }
        if source.life < 0 {
            source.drawing_enabled = false;
        } else if let Some(target) = target_position(source.word_0x96_150, arena, true)? {
            source.roll = i16::try_from(crate::heading_for_positions(source.position, target))
                .expect("Dragon launch heading fits i16");
            source.actual_speed = source.actual_speed.wrapping_sub(8);
            if source.actual_speed <= 16 {
                source.actual_speed = DRAGON_ACTUAL_SPEED;
                state.stage_var2 = 16;
            }
        } else {
            state.stage_var2 = 16;
        }
    } else {
        let parent = SourceEntityHandle {
            source_slot: state.parent_entity_index,
            generation: state.parent_generation,
        };
        let parent_position = if arena.resolve(parent).is_some() {
            target_position(parent.source_slot, arena, true)?
        } else {
            None
        };
        if parent_position.is_none() {
            state.stage_remaining = 0;
        }
        if state.stage_remaining <= 0 {
            let _ = crate::world::effects::allocate(
                arena,
                terrain,
                sim_definition::adapter::mc2_effect_definition(
                    crate::level_definition::EffectKind::Impact,
                ),
                source.position,
                particles,
            )?;
            if arena.resolve(entered_handle).is_none() {
                return Ok(DragonBossStageStep::default());
            }
            source.drawing_enabled = false;
        } else {
            let parent_position =
                parent_position.expect("positive remaining lifetime has a live parent");
            let mut target = target_position(source.word_0x96_150, arena, false)?;
            if target.is_none() {
                source.word_0x96_150 = 0;
                if source.source_byte_3e & 7 == 0 {
                    let parent_owner = crate::native_world::source_target::read(arena, target_context, parent.source_slot)?
                        .ok_or_else(|| format!("boss Dragon {slot} lost its parent during target scan"))?.source_id;
                    let mut best = u32::MAX;
                    for candidate in class3_slots.iter().copied() {
                        let Some(row) = crate::native_world::source_target::read(arena, target_context, candidate)? else {
                            continue;
                        };
                        if row.class != 3
                            || !matches!(row.model, 0 | 1)
                            || row.source_id == parent_owner
                        {
                            continue;
                        }
                        let position = players.iter().find(|player| player.entity_index == candidate)
                            .map_or(row.position, |player| player.position);
                        let distance = crate::legacy_sqrt(squared_wrapping_xy_distance(
                            parent_position,
                            position,
                        ));
                        if distance < best {
                            best = distance;
                            source.word_0x96_150 = candidate;
                            target = Some(position);
                        }
                    }
                }
            }
            let mut damaged = false;
            if source.pending_word_0x62 != 0 {
                source.life = source.life.wrapping_sub(runtime.pending_damage);
                source.word_0x26 = source.pending_word_0x62;
                source.pending_word_0x62 = 0;
                runtime.pending_damage = 0;
                damaged = true;
            } else {
                source.word_0x26 = 0;
            }
            let mut child_slot = runtime.linked_head;
            while child_slot != 0 {
                let Some(child) = arena
                    .economy
                    .authored_dragon_parts
                    .iter()
                    .find(|row| row.source_entity_index == child_slot)
                else {
                    break;
                };
                if child.life < source.life {
                    source.life = child.life;
                    source.word_0x26 = child.word_0x26;
                    damaged = true;
                    break;
                }
                child_slot = child.next_entity_index;
            }
            if source.life < 0 {
                // sub_1E700 leaves the StageVar callback active and makes
                // the remaining lifetime one instead of entering pre-kill.
                source.word_0x24 = source.word_0x26;
                source.action = DRAGON_STAGE_ACTION;
                state.stage_remaining = 1;
            } else if damaged {
                if step_dragon_motion(&mut source, &mut state, terrain) {
                    arena.economy.decrement_source_creature_population();
                }
                let attacker = source.word_0x26;
                let same_family = arena.economy.authored_class5.iter().any(|row| {
                    row.active && row.source_entity_index == attacker && row.model == DRAGON_MODEL
                }) || arena.economy.authored_dragon_parts.iter().any(|part| {
                    part.active
                        && part.source_entity_index == attacker
                        && part.model == DRAGON_MODEL
                });
                if attacker == parent.source_slot || same_family {
                    source.action = DRAGON_STAGE_ACTION;
                } else {
                    source.word_0x96_150 = attacker;
                    source.action = DRAGON_ATTACK_ACTION;
                    spell_feedback = Some((parent.source_slot, 19));
                }
            } else {
                if step_dragon_motion(&mut source, &mut state, terrain) {
                    arena.economy.decrement_source_creature_population();
                }
                if source.source_byte_3e & 7 == 0 {
                    if !state.movement_blocked {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            source.position,
                            target.unwrap_or(parent_position),
                        ))
                        .expect("Dragon stage heading fits i16");
                        if source.source_byte_3e & 0x3f == 0 {
                            apply_dragon_wander(&mut source);
                        }
                    }
                    let entity_id = state.entity_id;
                    if let Some(peer) = frozen_model0_slots.iter().find_map(|peer_slot| {
                        arena.economy.authored_class5.iter().find(|peer| {
                            peer.active
                                && peer.model == DRAGON_MODEL
                                && peer.source_entity_index == *peer_slot
                                && arena.economy.authored_dragons.iter().any(|other| {
                                    other.source_entity_index == *peer_slot && other.entity_id != entity_id
                                })
                                && i32::from(
                                    peer.position
                                        .x
                                        .wrapping_sub(source.position.x)
                                        .cast_signed(),
                                )
                                .abs()
                                    < i32::from(source.shift_rot_x)
                                && i32::from(
                                    peer.position
                                        .y
                                        .wrapping_sub(source.position.y)
                                        .cast_signed(),
                                )
                                .abs()
                                    < i32::from(source.shift_rot_x)
                        })
                    }) {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            peer.position,
                            source.position,
                        ))
                        .expect("Dragon separation heading fits i16");
                    }
                }
            }
            if let Some(target) = target {
                if source.source_byte_3e & 7 == 0
                    && distance_3d_wrapped(source.position, target) < u32::from(DRAGON_RANGE)
                {
                    source.action = DRAGON_ATTACK_ACTION;
                }
            } else {
                source.word_0x96_150 = 0;
                state.stage_remaining = state.stage_remaining.wrapping_sub(4);
            }
        }
    }
    if state.stage_var2 == 16 {
        step_dragon_font_counter(
            &mut source,
            &mut state,
            &arena.economy.map_chains,
            projectiles,
        );
    }
    step_dragon_vertical(&mut source, terrain);
    // An expiry effect can evict a different typed slot, shifting the
    // backing vectors even when this Dragon generation survives.
    let source_index = arena
        .economy
        .authored_class5
        .iter()
        .position(|row| row.active && row.source_entity_index == slot && row.model == DRAGON_MODEL)
        .ok_or_else(|| format!("boss Dragon {slot} lost its root during callback"))?;
    let state_index = arena
        .economy
        .authored_dragons
        .iter()
        .position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("boss Dragon {slot} lost its sidecar during callback"))?;
    let runtime_index = arena
        .economy
        .authored_class5_runtime
        .iter()
        .position(|row| row.source_entity_index == slot)
        .ok_or_else(|| format!("boss Dragon {slot} lost its runtime during callback"))?;
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_dragons[state_index] = state;
    arena.economy.authored_class5_runtime[runtime_index] = runtime;
    crate::world::effects::sync_position(
        arena,
        terrain,
        slot,
        source.position,
        source.active,
        false,
    )?;
    Ok(DragonBossStageStep { retained_for_epilogue: true, spell_feedback })
}

/// Publish the final fixed-address bytes before the source boundary releases
/// hidden typed roots and bodies. Later body callbacks may still read a
/// reclaimed predecessor's position and yaw even though its class is zero.
pub(crate) fn prepare_retiring_dragon_rows(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
) -> std::result::Result<(), String> {
    let roots = arena
        .economy
        .authored_class5
        .iter()
        .filter(|source| {
            source.active
                && source.model == DRAGON_MODEL
                && !source.drawing_enabled
                && (source.action == DRAGON_KILL_ACTION || arena.economy.authored_dragons.iter().any(|state| {
                    state.source_entity_index == source.source_entity_index
                        && matches!(state.stage_var2, 16 | 17)
                }))
        })
        .copied()
        .collect::<Vec<_>>();
    let parts = arena
        .economy
        .authored_dragon_parts
        .iter()
        .filter(|part| part.active && !part.drawing_enabled)
        .copied()
        .collect::<Vec<_>>();
    for source in roots {
        let slot = source.source_entity_index;
        let flags = arena
            .economy
            .authored_dragons
            .iter()
            .find(|state| state.source_entity_index == slot)
            .map_or(0, |state| state.control_flags)
            | 0x400;
        let entity_id = arena.economy.authored_dragons.iter()
            .find(|state| state.source_entity_index == slot)
            .map_or(slot, |state| state.entity_id);
        let owner = arena.economy.source_owner(slot);
        crate::world::effects::sync_position(arena, terrain, slot, source.position, false, false)?;
        let row = arena
            .entities
            .get_mut(usize::from(slot))
            .ok_or_else(|| format!("retiring Dragon root {slot} is outside the source arena"))?;
        row.reclaim_source_row();
        row.model = source.model;
        row.record[68] = source.model;
        row.source_id = entity_id;
        row.record[30..32].copy_from_slice(&entity_id.to_le_bytes());
        row.set_source_player_entity_index(owner);
        row.set_source_action(source.action);
        row.set_position(source.position);
        row.set_yaw(source.yaw);
        row.pitch = source.pitch;
        row.record[34..36].copy_from_slice(&source.pitch.to_le_bytes());
        row.set_source_roll(source.roll);
        row.set_fov(source.fov);
        row.set_life(source.life);
        row.set_max_life(source.max_life);
        row.set_mana(source.mana);
        row.set_subspell_index(source.subspell_index);
        row.set_entity_rng(source.entity_rng);
        row.set_control_flags(flags);
        row.set_phase(source.source_byte_3e);
    }
    for part in parts {
        let slot = part.source_entity_index;
        crate::world::effects::sync_position(arena, terrain, slot, part.position, false, false)?;
        let row = arena
            .entities
            .get_mut(usize::from(slot))
            .ok_or_else(|| format!("retiring Dragon part {slot} is outside the source arena"))?;
        row.reclaim_source_row();
        row.model = part.model;
        row.record[68] = part.model;
        row.source_id = part.entity_id;
        row.record[30..32].copy_from_slice(&part.entity_id.to_le_bytes());
        row.set_source_action(part.action);
        row.set_position(part.position);
        row.set_yaw(part.yaw);
        row.pitch = part.pitch;
        row.record[34..36].copy_from_slice(&part.pitch.to_le_bytes());
        row.set_source_roll(part.roll);
        row.set_fov(part.fov);
        row.set_life(part.life);
        row.set_max_life(part.max_life);
        row.set_mana(part.mana);
        row.set_subspell_index(part.subspell_index);
        row.set_entity_rng(part.entity_rng);
        row.set_control_flags(part.control_flags);
        row.set_phase(part.source_byte_3e);
        row.record[54..56].copy_from_slice(&part.previous_entity_index.to_le_bytes());
        row.record[56..58].copy_from_slice(&part.next_entity_index.to_le_bytes());
        row.record[58..60].copy_from_slice(&part.word_0x36.to_le_bytes());
        row.record[61] = part.source_byte_39;
    }
    Ok(())
}

pub(crate) fn retire_hidden_boss_dragons_at_source_boundary(arena: &mut SourceArena) {
    let hidden = arena
        .economy
        .authored_dragons
        .iter()
        .filter(|row| matches!(row.stage_var2, 16 | 17))
        .map(|row| row.source_entity_index)
        .collect::<std::collections::BTreeSet<_>>();
    for source in arena.economy.authored_class5.iter_mut().filter(|row| {
        row.active
            && row.model == DRAGON_MODEL
            && !row.drawing_enabled
            && (row.action == DRAGON_KILL_ACTION || hidden.contains(&row.source_entity_index))
    }) {
        source.active = false;
    }
    for part in arena
        .economy
        .authored_dragon_parts
        .iter_mut()
        .filter(|part| part.active && !part.drawing_enabled)
    {
        part.active = false;
    }
}

/// `sub_12500` restores the boss child's StageVar callback after an attack
/// returns to roaming; active attack/flee and death callbacks are untouched.
pub(crate) fn prepare_boss_stage_pass(arena: &mut SourceArena, frozen_model0_slots: &[u16]) {
    for slot in frozen_model0_slots {
        let Some(state) = arena
            .economy
            .authored_dragons
            .iter()
            .find(|state| state.source_entity_index == *slot)
        else {
            continue;
        };
        if !matches!(state.stage_var2, 14 | 16 | 17) {
            continue;
        }
        if let Some(source) = arena.economy.authored_class5.iter_mut().find(|source| {
            source.active
                && source.drawing_enabled
                && source.life >= 0
                && source.source_entity_index == *slot
                && source.model == DRAGON_MODEL
                && !matches!(source.action & 7, 2 | 4 | 5 | 6)
        }) {
            source.action = DRAGON_STAGE_ACTION;
        }
    }
}

/// After Alliance expires, source sub_12500 rebinds the saved StageVar on
/// the next source pass. Keep the shared route counter and thinning rules.
pub(crate) fn prepare_alliance_stage_pass(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    frozen_model0_slots: &[u16],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<(), String> {
    for &slot in frozen_model0_slots {
        let Some(source_index) = arena.economy.authored_class5.iter().position(|source|
            source.active && source.source_entity_index == slot && source.model == DRAGON_MODEL)
            else { continue; };
        let Some(state_index) = arena.economy.authored_dragons.iter().position(|state|
            state.source_entity_index == slot && matches!(state.stage_var2, 1 | 2 | 6 | 10))
            else { continue; };
        let mut source = arena.economy.authored_class5[source_index];
        let mut state = arena.economy.authored_dragons[state_index];
        let mut index = state.stage_var1 as i8;
        let mut mode = state.stage_var2 as i8;
        let mut error = None;
        crate::native_world::creatures::stage_route::Route {
            model: DRAGON_MODEL, position: source.position, action: &mut source.action,
            index: &mut index, mode: &mut mode, relation: &mut state.stage_relation,
        }.prepare(runtime, |target| match crate::native_world::source_target::read(arena, context, target) {
            Ok(Some(view)) => view.life < 0 || view.drawing_disabled(),
            Ok(None) => false,
            Err(e) => { error = Some(e); false }
        });
        if let Some(error) = error { return Err(error); }
        state.stage_var1 = index as u8;
        state.stage_var2 = mode as u8;
        arena.economy.authored_class5[source_index] = source;
        arena.economy.authored_dragons[state_index] = state;
    }
    Ok(())
}

/// sub_12100 binds a route only after the Dragon constructor succeeds.
pub(crate) fn bind_authored_stage_in_arena(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    slot: u16,
    stage_index: u8,
) -> std::result::Result<(), String> {
    let source_index = arena.economy.authored_class5.iter().position(|source|
        source.active && source.source_entity_index == slot && source.model == DRAGON_MODEL)
        .ok_or_else(|| format!("Dragon {slot} lost its stage source"))?;
    let state_index = arena.economy.authored_dragons.iter().position(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("Dragon {slot} lost its stage sidecar"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_dragons[state_index];
    let mut index = state.stage_var1 as i8;
    let mut mode = state.stage_var2 as i8;
    crate::native_world::creatures::stage_route::Route {
        model: DRAGON_MODEL, position: source.position, action: &mut source.action,
        index: &mut index, mode: &mut mode, relation: &mut state.stage_relation,
    }.bind(runtime, stage_index);
    state.stage_var1 = index as u8;
    state.stage_var2 = mode as u8;
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_dragons[state_index] = state;
    Ok(())
}

/// Dragon's source stage-1 patrol, stage-2 guard, and stage-6 wander.
/// sub_1F300 always executes the Dragon-specific tails after these callbacks.
pub(crate) fn step_authored_stage_in_arena(
    arena: &mut SourceArena,
    runtime: &RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    slot: u16,
    players: &[GoatPlayerTarget],
    frozen_model0_slots: &[u16],
    projectiles: &[DragonProjectileThreat],
    context: crate::native_world::source_target::Context<'_>,
) -> std::result::Result<DragonAllianceStep, String> {
    let source_index = arena.economy.authored_class5.iter().position(|source|
        source.active && source.source_entity_index == slot && source.model == DRAGON_MODEL
            && source.action == DRAGON_STAGE_ACTION)
        .ok_or_else(|| format!("Dragon {slot} lost its authored stage source"))?;
    let state_index = arena.economy.authored_dragons.iter().position(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("Dragon {slot} lost its authored stage sidecar"))?;
    let runtime_index = arena.economy.authored_class5_runtime.iter().position(|state| state.source_entity_index == slot)
        .ok_or_else(|| format!("Dragon {slot} lost its creature runtime"))?;
    let mut source = arena.economy.authored_class5[source_index];
    let mut state = arena.economy.authored_dragons[state_index];
    let mut creature = arena.economy.authored_class5_runtime[runtime_index];
    if !matches!(state.stage_var2, 1 | 2 | 6) {
        return Err(format!("Dragon {slot} has unsupported authored stage {}", state.stage_var2));
    }
    let point = if matches!(state.stage_var2, 1 | 2) {
        let stage = runtime.stage_variables.get(usize::from(state.stage_var1))
            .ok_or_else(|| format!("Dragon {slot} lost StageVar {}", state.stage_var1))?;
        let RuntimeRelation::AxisScalar(point) = stage.relation else {
            return Err(format!("Dragon {slot} stage has no point"));
        };
        Position { x: point as u16, y: (point >> 16) as u16, z: 0 }
    } else { Position::default() };
    let guard = state.stage_var2 == 2
        && (i32::from(point.x.cast_signed()) - i32::from(source.position.x.cast_signed())).abs() <= 3072
        && (i32::from(point.y.cast_signed()) - i32::from(source.position.y.cast_signed())).abs() <= 3072;
    let wander = state.stage_var2 == 6 || guard;
    let saved_target = source.word_0x96_150;
    if consume_dragon_damage(&mut source, &mut creature, &arena.economy.authored_dragon_parts, terrain) {
        if source.life >= 0 {
            let same_family = crate::native_world::source_target::read(arena, context, source.word_0x26)?
                .is_some_and(|row| (row.class, row.model) == (5, DRAGON_MODEL));
            if same_family {
                source.word_0x96_150 = saved_target;
                source.action = DRAGON_STAGE_ACTION;
            } else {
                state.stage_var2 = 10;
            }
        }
    } else {
        if step_dragon_motion(&mut source, &mut state, terrain) {
            arena.economy.decrement_source_creature_population();
        }
        if wander {
            // Owner descriptor71 byte32 has bit2 clear, so sub_1E1C0 moves
            // and draws its guarded random turn every sixteen visits.
            if source.source_byte_3e & 15 == 0 && !state.movement_blocked {
                source.entity_rng = level2_next_rng16(source.entity_rng);
                source.roll = source.roll.wrapping_add(
                    i16::try_from(source.entity_rng % 113 + 142).expect("Dragon wander turn fits i16")
                ) & 0x07ff;
            }
        } else if source.source_byte_3e & 7 == 0 {
            if !state.movement_blocked {
                source.roll = i16::try_from(crate::heading_for_positions(source.position, point))
                    .expect("Dragon stage heading fits i16");
                if source.source_byte_3e & 0x3f == 0 { apply_dragon_wander(&mut source); }
            }
            if let Some(peer) = frozen_model0_slots.iter().find_map(|peer_slot| {
                arena.economy.authored_class5.iter().find(|peer| {
                    peer.active && peer.model == DRAGON_MODEL && peer.source_entity_index == *peer_slot
                        && arena.economy.authored_dragons.iter().any(|other|
                            other.source_entity_index == *peer_slot && other.entity_id != state.entity_id)
                        && i32::from(peer.position.x.cast_signed()).abs_diff(i32::from(source.position.x.cast_signed())) < source.shift_rot_x as u32
                        && i32::from(peer.position.y.cast_signed()).abs_diff(i32::from(source.position.y.cast_signed())) < source.shift_rot_x as u32
                })
            }) {
                source.roll = i16::try_from(crate::heading_for_positions(peer.position, source.position))
                    .expect("Dragon stage separation heading fits i16");
            }
        }
    }
    if state.stage_var2 == 2 && source.action == DRAGON_STAGE_ACTION && source.source_byte_39 != 0
        && source.source_byte_3e % DRAGON_DESCRIPTOR_CADENCE == 0
    {
        let range_squared = u32::from(DRAGON_RANGE).pow(2);
        if let Some(target) = players.iter().filter(|target| target.control_flags & 0x20 == 0)
            .filter_map(|target| {
                let dx = i32::from(target.position.x) - i32::from(source.position.x);
                let dy = i32::from(target.position.y.wrapping_sub(source.position.y).cast_signed());
                let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)).cast_unsigned();
                let heading = crate::heading_for_positions(source.position, target.position);
                (distance <= range_squared && crate::angle_distance(i32::from(source.yaw), heading) < DRAGON_VIEW_HALF_ANGLE)
                    .then_some((distance, target.entity_index))
            }).min_by_key(|(distance, _)| *distance).map(|(_, slot)| slot)
        {
            source.word_0x96_150 = target;
            state.stage_var2 = 10;
            source.action = DRAGON_ATTACK_ACTION;
        }
    }
    step_dragon_tails(&mut source, &mut state, terrain, &arena.economy.map_chains, projectiles);
    arena.economy.source_rng.insert(slot, source.entity_rng);
    arena.economy.authored_class5[source_index] = source;
    arena.economy.authored_dragons[state_index] = state;
    arena.economy.authored_class5_runtime[runtime_index] = creature;
    crate::world::effects::sync_position(arena, terrain, slot, source.position, source.active, false)?;
    Ok(DragonAllianceStep { retained_for_epilogue: true, spell_feedback: Vec::new() })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DragonAttackStep {
    pub(crate) live: bool,
    pub(crate) fireball: Option<DragonFireballLaunch>,
}

/// Run the post-1CC20 Dragon tails against the current arena occupant, after
/// allocation and target feedback, rather than against a preallocation copy.
pub(crate) fn finish_attack_in_arena(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
    projectiles: &[DragonProjectileThreat],
) -> std::result::Result<bool, String> {
    let handle = arena.handle(source_slot)
        .ok_or_else(|| format!("Dragon attack tail {source_slot} lost its current address"))?;
    if !matches!(arena.resolve(handle), Some(crate::campaign::source_arena::SourceEntityView::Dynamic(
        Level2DynamicArenaKind::AuthoredClass5
    ))) {
        return Err(format!("Dragon attack tail {source_slot} is no longer a typed class5 carrier"));
    }
    let live = arena.economy.finish_authored_dragon_attack_slot(source_slot, terrain, projectiles)?;
    if live {
        let position = arena.economy.authored_class5.iter()
            .find(|source| source.active && source.source_entity_index == source_slot && source.model == DRAGON_MODEL)
            .ok_or_else(|| format!("Dragon attack tail {source_slot} lost its typed source"))?.position;
        crate::world::effects::sync_position(arena, terrain, source_slot, position, true, false)?;
    }
    Ok(live)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DragonLifecycleStep {
    pub(crate) handled: bool,
    pub(crate) retained_for_epilogue: bool,
    pub(crate) pre_kill_credit: Option<u16>,
    death_due: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DragonFireballLaunch {
    pub(crate) owner_entity_index: u16,
    pub(crate) target_entity_index: u16,
    pub(crate) constructor_position: Position,
    pub(crate) position: Position,
    pub(crate) target_position: Position,
    pub(crate) yaw: i16,
    pub(crate) pitch: i16,
    pub(crate) payload: u16,
}

/// Current occupant of the fixed predecessor address read by sub_1B6B0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DragonPredecessor {
    pub(crate) class: u8,
    pub(crate) position: Position,
    pub(crate) yaw: i16,
}

fn kill_current_dragon(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    is_part: bool,
) -> std::result::Result<(), String> {
    let original = arena.handle(slot)
        .ok_or_else(|| format!("Dragon death lost source handle at {slot}"))?;
    let (position, yaw, mana, rng) = if is_part {
        let part = arena.economy.authored_dragon_parts.iter()
            .find(|part| part.active && part.source_entity_index == slot)
            .ok_or_else(|| format!("Dragon death lost part at {slot}"))?;
        (part.position, part.yaw, part.mana, part.entity_rng)
    } else {
        let root = arena.economy.authored_class5.iter()
            .find(|root| root.active && root.source_entity_index == slot && root.model == DRAGON_MODEL)
            .ok_or_else(|| format!("Dragon death lost root at {slot}"))?;
        (root.position, root.yaw, root.mana, root.entity_rng)
    };
    if mana > 0 {
        let rng = level2_next_rng16(rng);
        if is_part {
            arena.economy.authored_dragon_parts.iter_mut()
                .find(|part| part.active && part.source_entity_index == slot)
                .expect("the part was read above").entity_rng = rng;
        } else {
            arena.economy.authored_class5.iter_mut()
                .find(|root| root.active && root.source_entity_index == slot)
                .expect("the root was read above").entity_rng = rng;
        }
        arena.economy.source_rng.insert(slot, rng);
        let owner = arena.economy.source_owner(slot);
        crate::world::effects::spawn_death_mana_sphere(
            arena, terrain, slot, position, yaw, mana, owner,
        )?;
        crate::world::effects::clear_replacement_mana_owner(arena, slot)?;
        arena.economy.source_owner.insert(slot, 0);
    }
    let current = crate::native_world::source_target::read(
        arena, crate::native_world::source_target::Context::default(), slot,
    )?.ok_or_else(|| format!("Dragon death lost current source address {slot}"))?;
    let burst = if current.control_flags & 0x10_0000 == 0 {
        crate::world::effects::spawn_death_burst(
            arena, terrain, particles, slot, current.position, current.source_id,
        )?
    } else { None };
    if !crate::world::effects::hide_birth_replacement(arena, slot, burst)? {
        if arena.resolve(original).is_none() {
            return Err(format!("Dragon death cannot hide replacement at source slot {slot}"));
        }
        if is_part {
            let part = arena.economy.authored_dragon_parts.iter_mut()
                .find(|part| part.active && part.source_entity_index == slot)
                .ok_or_else(|| format!("Dragon death lost part at {slot}"))?;
            part.drawing_enabled = false;
            part.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        } else {
            arena.economy.authored_class5.iter_mut()
                .find(|root| root.active && root.source_entity_index == slot)
                .ok_or_else(|| format!("Dragon death lost root at {slot}"))?
                .drawing_enabled = false;
            arena.economy.authored_dragons.iter_mut()
                .find(|state| state.source_entity_index == slot)
                .ok_or_else(|| format!("Dragon death lost root state at {slot}"))?
                .control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }
    Ok(())
}

pub(crate) fn step_lifecycle_in_arena(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_slot: u16,
) -> std::result::Result<DragonLifecycleStep, String> {
    let result = arena.economy.step_authored_dragon_lifecycle_slot(source_slot, terrain)?;
    if result.death_due { kill_current_dragon(arena, terrain, particles, source_slot, false)?; }
    Ok(result)
}

pub(crate) fn step_part_in_arena(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    source_slot: u16,
    targets: source_target::Context<'_>,
) -> std::result::Result<bool, String> {
    let Some(part) = arena.economy.authored_dragon_parts.iter().find(|part| {
        part.active && part.source_entity_index == source_slot && part.model == DRAGON_MODEL
    }).copied() else {
        return Ok(false);
    };
    // The generic action-5 callback does not follow the previous segment.
    // Earlier segments may already have changed into their death products.
    if part.action == DRAGON_KILL_ACTION {
        if part.source_byte_3e & 7 == 0 {
            kill_current_dragon(arena, terrain, particles, source_slot, true)?;
        }
        return Ok(true);
    }
    if part.action != DRAGON_PART_ACTION {
        return Ok(false);
    }
    let previous = part.previous_entity_index;
    let predecessor = if let Some(view) = source_target::read(arena, targets, previous)? {
        DragonPredecessor {
            class: view.class,
            position: view.position,
            yaw: view.yaw.ok_or_else(|| format!("Dragon predecessor {previous} has no current yaw"))?,
        }
    } else {
        let sentinel = arena.entities.get(usize::from(previous))
            .ok_or_else(|| format!("Dragon predecessor {previous} is outside the source arena"))?;
        DragonPredecessor {
            class: sentinel.class,
            position: sentinel.position,
            yaw: sentinel.yaw,
        }
    };
    Ok(arena.economy.step_authored_dragon_part_slot(source_slot, terrain, predecessor))
}

impl Level2EconomyState {
    /// Run the generic action-4/action-5 creature lifecycle for one Dragon
    /// root. `PreKillEntity_1C890` changes every linked body row to action 5
    /// before changing the root; `KillEntity_1C930` then converts each row's
    /// own mana on its independent byte-0x3E cadence.
    pub(crate) fn step_authored_dragon_lifecycle_slot(
        &mut self,
        source_slot: u16,
        _terrain: &SessionTerrain,
    ) -> std::result::Result<DragonLifecycleStep, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
        }) else {
            return Ok(DragonLifecycleStep::default());
        };
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|runtime| runtime.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no class-5 runtime"))?;
        let _dragon_index = self
            .authored_dragons
            .iter()
            .position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no runtime sidecar"))?;
        let mut source = self.authored_class5[source_index];

        if source.action == DRAGON_PRE_KILL_ACTION {
            let mut part_slot = self.authored_class5_runtime[runtime_index].linked_head;
            while part_slot != 0 {
                let Some(part_index) = self
                    .authored_dragon_parts
                    .iter()
                    .position(|part| part.source_entity_index == part_slot)
                else {
                    break;
                };
                let next_part = self.authored_dragon_parts[part_index].next_entity_index;
                self.authored_dragon_parts[part_index].action = DRAGON_KILL_ACTION;
                crate::combat::actors::inherit_killer(
                    &mut source.word_0x24,
                    self.authored_dragon_parts[part_index].word_0x24,
                );
                part_slot = next_part;
            }
            let pre_kill_credit = (source.word_0x24 != 0).then_some(source.word_0x24);
            source.action = DRAGON_KILL_ACTION;
            self.authored_class5[source_index] = source;
            return Ok(DragonLifecycleStep {
                handled: true,
                retained_for_epilogue: true,
                pre_kill_credit,
                death_due: false,
            });
        }
        if source.action != DRAGON_KILL_ACTION {
            return Ok(DragonLifecycleStep::default());
        }

        Ok(DragonLifecycleStep {
            handled: true,
            retained_for_epilogue: true,
            pre_kill_credit: None,
            death_due: source.source_byte_3e & 7 == 0,
        })
    }

    /// Reproduce `sub_4B240`: one Dragon root followed by sixteen copied body
    /// rows, then map-link the root after the body chain. The adapter keeps
    /// the constructor transactional so arena exhaustion cannot publish a
    /// partial typed chain.
    pub(crate) fn allocate_authored_dragon(
        &mut self,
        authored_index: u16,
        mut position: Position,
        max_life: i32,
        terrain: &SessionTerrain,
    ) -> Option<Level2AuthoredClass5Source> {
        if !(1..=i32::MAX).contains(&max_life) {
            return None;
        }
        let before = self.clone();
        position.z = terrain.ground_height(position);
        let source_entity_index = self.allocate_slot()?;
        let entity_rng = level2_next_rng16(self.new_event_rng(source_entity_index));
        let heading = i16::try_from(entity_rng & 0x07ff)
            .expect("Dragon heading fits i16")
            .wrapping_sub(1);
        let source_byte_3e = self.class5_model0_constructor_phase;
        self.class5_model0_constructor_phase = source_byte_3e.wrapping_add(1);
        let mut source = Level2AuthoredClass5Source {
            source_entity_index,
            entity_rng,
            authored_index,
            model: DRAGON_MODEL,
            action: DRAGON_ROAM_ACTION,
            minimum_speed: DRAGON_MINIMUM_SPEED,
            maximum_speed: DRAGON_MAXIMUM_SPEED,
            actual_speed: DRAGON_ACTUAL_SPEED,
            max_life,
            life: max_life,
            mana: DRAGON_INITIAL_MANA,
            max_mana: DRAGON_MAX_MANA,
            subspell_index: DRAGON_SUBSPELL_INDEX,
            dword_0x10: i32::from(source_entity_index % 100),
            pending_word_0x62: 0,
            word_0x26: 0,
            word_0x24: 0,
            byte_0x3b: 0,
            word_0x96_150: 0,
            byte_0x46_70: 0,
            particle_index: DRAGON_ROOT_PARTICLE,
            source_byte_3e,
            source_byte_39: DRAGON_DESCRIPTOR_CADENCE
                .wrapping_sub(source_byte_3e % DRAGON_DESCRIPTOR_CADENCE)
                .wrapping_add(4),
            yaw: heading,
            pitch: heading,
            roll: heading,
            fov: 0,
            shift_rot_x: DRAGON_RENDER_SPEED / 2,
            shift_rot_y: DRAGON_RENDER_ROTATION / 2,
            position,
            phase: 0,
            drawing_enabled: true,
            active: true,
        };

        let mut previous_entity_index = source_entity_index;
        let mut first_part = 0;
        for part_index in 0..DRAGON_PART_COUNT {
            let Some(part_slot) = self.allocate_slot() else {
                *self = before;
                return None;
            };
            let speed = DRAGON_PART_SPEEDS[part_index];
            let rotation = DRAGON_PART_ROTATIONS[part_index];
            if self
                .map_chains
                .sync_entity(RenderableMapSeed {
                    entity_index: part_slot,
                    active: true,
                    x: position.x,
                    y: position.y,
                })
                .is_err()
            {
                *self = before;
                return None;
            }
            let part = AuthoredDragonPart {
                source_entity_index: part_slot,
                root_authored_index: source.authored_index,
                entity_id: source_entity_index,
                entity_rng,
                control_flags: DRAGON_CONSTRUCTOR_FLAGS,
                // The body row is a raw root copy. `sub_4B240` uses
                // word_0x32/word_0x34 for the chain and leaves the copied
                // parentId at the root's NewEvent default of zero.
                parent_entity_index: 0,
                previous_entity_index,
                next_entity_index: 0,
                word_0x36: speed / 2,
                pending_word_0x62: 0,
                pending_damage: 0,
                word_0x24: source.word_0x24,
                word_0x26: 0,
                model: DRAGON_MODEL,
                action: DRAGON_PART_ACTION,
                source_byte_3e: u8::try_from(part_index).expect("Dragon part index fits u8"),
                source_byte_39: source.source_byte_39,
                max_life,
                life: max_life,
                // `sub_4B240` copies the root before replacing the root's
                // mana with max-mana/32. The first segment therefore keeps
                // the initial half-mana value; later segments copy 140.
                mana: source.mana,
                subspell_index: source.subspell_index,
                particle_index: u16::try_from(part_index)
                    .expect("Dragon part index fits u16")
                    .wrapping_add(19),
                yaw: heading,
                pitch: heading,
                roll: heading,
                fov: 0,
                render_yaw: rotation / 2,
                render_pitch: speed / 2,
                render_roll: speed / 2,
                render_fov: rotation / 2,
                position,
                drawing_enabled: true,
                active: true,
            };
            if first_part == 0 {
                first_part = part_slot;
            }
            if previous_entity_index != source_entity_index
                && let Some(previous) = self
                    .authored_dragon_parts
                    .iter_mut()
                    .find(|previous| previous.source_entity_index == previous_entity_index)
            {
                previous.next_entity_index = part_slot;
            }
            self.source_rng.insert(part_slot, entity_rng);
            self.source_owner.insert(part_slot, 0);
            self.authored_dragon_parts.push(part);
            source.mana = DRAGON_SEGMENT_MANA;
            previous_entity_index = part_slot;
        }
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
            *self = before;
            return None;
        }
        self.source_rng.insert(source_entity_index, entity_rng);
        self.source_owner.insert(source_entity_index, 0);
        self.authored_class5.push(source);
        self.authored_class5_runtime
            .push(Level2AuthoredClass5RuntimeState {
                source_entity_index,
                byte_0x38: 1,
                xtype: 3,
                descriptor_index: DRAGON_DESCRIPTOR,
                pending_damage: 0,
                linked_head: first_part,
            });
        self.authored_dragons.push(AuthoredDragonRuntime {
            source_entity_index,
            entity_id: source_entity_index,
            stage_var1: 0,
            stage_var2: 0,
            stage_relation: 0,
            parent_entity_index: 0,
            parent_generation: 0,
            stage_remaining: 0,
            alliance_duration: 0,
            linked_entity_index: 0,
            word_0x36: 96,
            control_flags: DRAGON_CONSTRUCTOR_FLAGS,
            font_type_index: 0,
            dodge_remaining: 0,
            render_yaw: DRAGON_RENDER_ROTATION / 2,
            render_pitch: DRAGON_RENDER_SPEED / 2,
            render_roll: DRAGON_RENDER_SPEED / 2,
            render_fov: DRAGON_RENDER_ROTATION / 2,
            movement_blocked: false,
        });
        Some(source)
    }

    /// Run the undamaged `sub_1EF40` action-1 callback: ordinary creature
    /// motion/roam, the initially inert font tail, then Dragon vertical
    /// motion. Damage and successor actions remain visibly unsupported until
    /// their own package-reached callbacks are implemented.
    pub(crate) fn step_authored_dragon_roam_slot(
        &mut self,
        source_slot: u16,
        players: &[GoatPlayerTarget],
        frozen_model0_slots: &[u16],
        terrain: &SessionTerrain,
        projectiles: &[DragonProjectileThreat],
    ) -> std::result::Result<bool, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
                && source.action == DRAGON_ROAM_ACTION
        }) else {
            return Ok(false);
        };
        let Some(state_index) = self
            .authored_dragons
            .iter()
            .position(|state| state.source_entity_index == source_slot)
        else {
            return Err(format!(
                "Dragon source slot {source_slot} has no runtime sidecar"
            ));
        };
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|runtime| runtime.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no class-5 runtime"))?;

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_dragons[state_index];
        let mut runtime = self.authored_class5_runtime[runtime_index];
        if !consume_dragon_damage(
            &mut source,
            &mut runtime,
            &self.authored_dragon_parts,
            terrain,
        ) {
            if step_dragon_motion(&mut source, &mut state, terrain) {
                self.decrement_source_creature_population();
            }

            if source.source_byte_3e % DRAGON_DESCRIPTOR_CADENCE == 0 {
                apply_dragon_wander(&mut source);
                if source.source_byte_39 != 0 {
                    if let Some(target) = nearest_dragon_player(source, players) {
                        source.word_0x96_150 = target;
                        source.action = 2;
                    } else if let Some(peer) = nearest_unlinked_dragon(
                        source,
                        frozen_model0_slots,
                        &self.authored_class5,
                        &self.authored_dragons,
                    ) {
                        state.linked_entity_index = peer;
                        source.action = 3;
                    }
                }
            }
        }

        step_dragon_tails(
            &mut source,
            &mut state,
            terrain,
            &self.map_chains,
            projectiles,
        );

        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_class5_runtime[runtime_index] = runtime;
        self.authored_dragons[state_index] = state;
        Ok(self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok())
    }

    /// Run undamaged `sub_1EF70 -> sub_1C310(..., sub_1CC20)`. The callback
    /// owns movement, target validation, aiming, range/cadence, and the launch
    /// intent. The campaign adapter owns the shared NewEvent allocation and
    /// publishes sound 8 only when that allocation succeeds.
    pub(crate) fn step_authored_dragon_attack_slot(
        &mut self,
        source_slot: u16,
        target: Option<GoatPlayerTarget>,
        frozen_model0_slots: &[u16],
        terrain: &SessionTerrain,
        prelude_done: bool,
    ) -> std::result::Result<DragonAttackStep, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
                && source.action == DRAGON_ATTACK_ACTION
        }) else {
            return Ok(DragonAttackStep::default());
        };
        let state_index = self
            .authored_dragons
            .iter()
            .position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no runtime sidecar"))?;
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|runtime| runtime.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no class-5 runtime"))?;

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_dragons[state_index];
        let mut fireball = None;
        let mut runtime = self.authored_class5_runtime[runtime_index];
        if prelude_done || !consume_dragon_damage(
            &mut source,
            &mut runtime,
            &self.authored_dragon_parts,
            terrain,
        ) {
            if !prelude_done && step_dragon_motion(&mut source, &mut state, terrain) {
                self.decrement_source_creature_population();
            }

            let target = target.filter(|target| {
                target.entity_index == source.word_0x96_150
                    && target.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            });
            if let Some(target) = target {
                if source.source_byte_3e & 3 == 0 {
                    source.roll = i16::try_from(crate::heading_for_positions(
                        source.position,
                        target.position,
                    ))
                    .expect("Dragon target heading fits i16");
                    if let Some(peer) = first_close_dragon(
                        source,
                        frozen_model0_slots,
                        &self.authored_class5,
                        DRAGON_RENDER_SPEED / 2,
                    ) {
                        source.roll = i16::try_from(crate::heading_for_positions(
                            peer.position,
                            source.position,
                        ))
                        .expect("Dragon separation heading fits i16");
                    }
                }
                if source.source_byte_3e % DRAGON_DESCRIPTOR_CADENCE == 0 {
                    if distance_3d_wrapped(source.position, target.position)
                        >= u32::from(DRAGON_RANGE)
                    {
                        source.action = DRAGON_ROAM_ACTION;
                    } else {
                        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
                            source.position,
                            target.position,
                        );
                        let mut launch_position = source.position;
                        launch_position.z =
                            launch_position.z.wrapping_add(DRAGON_RENDER_ROTATION / 2);
                        fireball = Some(DragonFireballLaunch {
                            owner_entity_index: source.source_entity_index,
                            target_entity_index: target.entity_index,
                            constructor_position: source.position,
                            position: launch_position,
                            target_position: target.position,
                            yaw,
                            pitch,
                            payload: DRAGON_FIREBALL_PAYLOAD,
                        });
                    }
                }
            } else {
                source.action = DRAGON_ROAM_ACTION;
            }
        }

        // sub_1EF70 invokes the 1CC20 constructor before its font and
        // vertical tails. Publish this pre-tail source so NewEvent and its
        // fixed-address reads observe the correct position and identity.
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_class5_runtime[runtime_index] = runtime;
        self.authored_dragons[state_index] = state;
        let live = self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok();
        Ok(DragonAttackStep {
            live,
            fireball: if live { fireball } else { None },
        })
    }

    /// Finish sub_1EF70 only after its projectile constructor and successful
    /// feedback have run. The source cursor owns the later common epilogue.
    pub(crate) fn finish_authored_dragon_attack_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        projectiles: &[DragonProjectileThreat],
    ) -> std::result::Result<bool, String> {
        let source_index = self.authored_class5.iter().position(|source| {
            source.active && source.source_entity_index == source_slot && source.model == DRAGON_MODEL
        }).ok_or_else(|| format!("Dragon attack tail {source_slot} lost its current source"))?;
        let state_index = self.authored_dragons.iter().position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon attack tail {source_slot} lost its current sidecar"))?;
        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_dragons[state_index];
        step_dragon_tails(&mut source, &mut state, terrain, &self.map_chains, projectiles);
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_dragons[state_index] = state;
        Ok(self.map_chains.sync_entity(RenderableMapSeed {
            entity_index: source_slot, active: source.active,
            x: source.position.x, y: source.position.y,
        }).is_ok())
    }

    /// Run `sub_1EFD0 -> sub_1C560(..., 0)`: follow the Dragon root in the
    /// source `word_0x32` relation, inherit its target/action at descriptor
    /// cadence, and preserve the common motion and vertical tails.
    pub(crate) fn step_authored_dragon_link_slot(
        &mut self,
        source_slot: u16,
        frozen_model0_slots: &[u16],
        terrain: &SessionTerrain,
        projectiles: &[DragonProjectileThreat],
    ) -> std::result::Result<bool, String> {
        let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
                && source.action == DRAGON_LINK_ACTION
        }) else {
            return Ok(false);
        };
        let state_index = self
            .authored_dragons
            .iter()
            .position(|state| state.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no runtime sidecar"))?;
        let runtime_index = self
            .authored_class5_runtime
            .iter()
            .position(|runtime| runtime.source_entity_index == source_slot)
            .ok_or_else(|| format!("Dragon source slot {source_slot} has no class-5 runtime"))?;

        let mut source = self.authored_class5[source_index];
        let mut state = self.authored_dragons[state_index];
        let mut runtime = self.authored_class5_runtime[runtime_index];
        let peer = self.authored_class5.iter().copied().find(|peer| {
            peer.source_entity_index == state.linked_entity_index
                && peer.source_entity_index != source_slot
                && peer.active
                && peer.drawing_enabled
                && peer.life >= 0
                && peer.model == DRAGON_MODEL
        });

        if consume_dragon_damage(
            &mut source,
            &mut runtime,
            &self.authored_dragon_parts,
            terrain,
        ) {
            if let Some(peer) = peer
                && let Some(peer_index) = self
                    .authored_class5
                    .iter()
                    .position(|candidate| candidate.source_entity_index == peer.source_entity_index)
            {
                self.authored_class5[peer_index].word_0x96_150 = source.word_0x26;
                self.authored_class5[peer_index].action = DRAGON_ATTACK_ACTION;
                if let Some(peer_state) = self
                    .authored_dragons
                    .iter_mut()
                    .find(|candidate| candidate.source_entity_index == peer.source_entity_index)
                {
                    peer_state.linked_entity_index = 0;
                }
            }
            state.linked_entity_index = 0;
        } else {
            if step_dragon_motion(&mut source, &mut state, terrain) {
                self.decrement_source_creature_population();
            }
            if let Some(peer) = peer {
                if source.source_byte_3e % DRAGON_DESCRIPTOR_CADENCE == 0 {
                    match peer.action {
                        0 | DRAGON_ROAM_ACTION => {}
                        DRAGON_ATTACK_ACTION => {
                            source.word_0x96_150 = peer.word_0x96_150;
                            state.linked_entity_index = 0;
                            source.action = DRAGON_ATTACK_ACTION;
                        }
                        DRAGON_LINK_ACTION => {
                            state.linked_entity_index = self
                                .authored_dragons
                                .iter()
                                .find(|candidate| {
                                    candidate.source_entity_index == peer.source_entity_index
                                })
                                .map_or(0, |candidate| candidate.linked_entity_index);
                        }
                        _ => {
                            state.linked_entity_index = 0;
                            source.action = DRAGON_ROAM_ACTION;
                        }
                    }
                    if source.action == DRAGON_LINK_ACTION {
                        let heading_target = self
                            .authored_class5
                            .iter()
                            .find(|candidate| {
                                candidate.active
                                    && candidate.source_entity_index == state.linked_entity_index
                            })
                            .copied();
                        if let Some(heading_target) = heading_target {
                            source.roll = i16::try_from(crate::heading_for_positions(
                                source.position,
                                heading_target.position,
                            ))
                            .expect("Dragon peer heading fits i16");
                            if let Some(close_peer) = first_close_dragon(
                                source,
                                frozen_model0_slots,
                                &self.authored_class5,
                                256,
                            ) {
                                source.roll = i16::try_from(crate::heading_for_positions(
                                    close_peer.position,
                                    source.position,
                                ))
                                .expect("Dragon separation heading fits i16");
                            }
                            source.actual_speed =
                                peer.maximum_speed.wrapping_add(peer.actual_speed);
                        }
                    }
                }
            } else {
                state.linked_entity_index = 0;
                source.action = DRAGON_ROAM_ACTION;
            }
        }

        step_dragon_tails(
            &mut source,
            &mut state,
            terrain,
            &self.map_chains,
            projectiles,
        );
        self.source_rng.insert(source_slot, source.entity_rng);
        self.authored_class5[source_index] = source;
        self.authored_class5_runtime[runtime_index] = runtime;
        self.authored_dragons[state_index] = state;
        Ok(self
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: source.active,
                x: source.position.x,
                y: source.position.y,
            })
            .is_ok())
    }

    #[cfg(test)]
    pub(crate) fn stage_authored_dragon_attack_prerequisite_for_test(
        &mut self,
        source_slot: u16,
        position: Position,
        phase: u8,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
        }) else {
            return false;
        };
        source.position = position;
        source.source_byte_3e = phase;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_ok()
    }

    /// Submit one `sub_10C80(a2 = 0)` mailbox write to a live Dragon root or
    /// body segment. The target consumes this mailbox only on its own numeric
    /// source visit.
    pub(crate) fn submit_authored_dragon_damage(
        &mut self,
        target_slot: u16,
        damage: i32,
        attacker: u16,
    ) -> bool {
        if damage <= 0 || attacker == 0 || !self.map_chains.is_linked(target_slot) {
            return false;
        }
        if let Some(source_index) = self.authored_class5.iter().position(|source| {
            source.active
                && source.drawing_enabled
                && source.life >= 0
                && source.source_entity_index == target_slot
                && source.source_entity_index != attacker
                && source.model == DRAGON_MODEL
        }) {
            let Some(runtime) = self
                .authored_class5_runtime
                .iter_mut()
                .find(|runtime| runtime.source_entity_index == target_slot)
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
            return true;
        }
        let Some(part) = self.authored_dragon_parts.iter_mut().find(|part| {
            part.active
                && part.drawing_enabled
                && part.life >= 0
                && part.source_entity_index == target_slot
                && part.source_entity_index != attacker
                && part.model == DRAGON_MODEL
        }) else {
            return false;
        };
        part.pending_damage = if part.pending_word_0x62 == 0 {
            damage
        } else {
            part.pending_damage.wrapping_add(damage)
        };
        part.pending_word_0x62 = attacker;
        true
    }

    /// Run the action-232 Dragon-body callback. The segment follows its
    /// `word_0x32` predecessor first, then consumes its private damage
    /// mailbox. The scheduler remains the sole byte-0x3E epilogue owner.
    pub(crate) fn step_authored_dragon_part_slot(
        &mut self,
        source_slot: u16,
        terrain: &SessionTerrain,
        predecessor: DragonPredecessor,
    ) -> bool {
        let Some(part_index) = self.authored_dragon_parts.iter().position(|part| {
            part.active && part.source_entity_index == source_slot && part.model == DRAGON_MODEL
        }) else {
            return false;
        };
        let mut part = self.authored_dragon_parts[part_index];
        if part.action == DRAGON_KILL_ACTION {
            // The source-arena wrapper owns phase-zero constructors and hide.
            return true;
        }
        if part.action != DRAGON_PART_ACTION {
            return false;
        }
        let part = &mut self.authored_dragon_parts[part_index];
        if predecessor.class != 5 {
            part.drawing_enabled = false;
            part.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
        if part.source_byte_39 != 0 {
            let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(
                part.position,
                predecessor.position,
            );
            part.yaw = yaw;
            part.pitch = pitch;
            let mut position = predecessor.position;
            crate::move_entity(&mut position, yaw, pitch, part.word_0x36.wrapping_neg());
            part.position = position;
            if part.pending_word_0x62 != 0 {
                part.life = part.life.wrapping_sub(part.pending_damage);
                part.word_0x26 = part.pending_word_0x62;
                part.pending_word_0x62 = 0;
                part.pending_damage = 0;
            } else {
                part.word_0x26 = 0;
            }
        } else if part.source_byte_3e & 3 == 0 {
            part.position = predecessor.position;
            part.yaw = predecessor.yaw;
        }
        self.source_rng.insert(source_slot, part.entity_rng);
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: part.active,
                x: part.position.x,
                y: part.position.y,
            })
            .is_ok()
    }

    #[cfg(test)]
    pub(crate) fn stage_authored_dragon_part_position_for_test(
        &mut self,
        source_slot: u16,
        position: Position,
    ) -> bool {
        let Some(part) = self
            .authored_dragon_parts
            .iter_mut()
            .find(|part| part.active && part.source_entity_index == source_slot)
        else {
            return false;
        };
        part.position = position;
        self.map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: source_slot,
                active: true,
                x: position.x,
                y: position.y,
            })
            .is_ok()
    }

    /// Stage only the already-proven terminal hide of one real Dragon root.
    /// The next ordinary source boundary still owns body propagation, map
    /// unlinking, slot/RNG release, and the frozen model-0 census used by the
    /// semantic extinction objective.
    #[cfg(test)]
    pub(crate) fn stage_authored_dragon_terminal_hide_for_test(
        &mut self,
        source_slot: u16,
    ) -> bool {
        let Some(source) = self.authored_class5.iter_mut().find(|source| {
            source.active
                && source.source_entity_index == source_slot
                && source.model == DRAGON_MODEL
        }) else {
            return false;
        };
        source.drawing_enabled = false;
        source.active = false;
        for part in &mut self.authored_dragon_parts {
            if part.active && part.entity_id == source_slot {
                part.drawing_enabled = false;
                part.active = false;
            }
        }
        true
    }
}

fn step_dragon_motion(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredDragonRuntime,
    terrain: &SessionTerrain,
) -> bool {
    // sub_1B8C0 consumes this one-shot suppression before terrain/motion.
    if state.control_flags & 0x800 != 0 {
        state.control_flags &= !0x800;
        return false;
    }
    let transition = crate::ground_creature_motion::step_ground_creature_motion(
        source.position,
        source.yaw,
        source.roll,
        source.actual_speed,
        terrain,
        crate::ground_creature_motion::GroundCreatureMotionConfig {
            half_width: state.render_pitch.max(state.render_roll),
            half_height: state.render_fov,
            ground_clearance: DRAGON_GROUND_CLEARANCE,
            descent: DRAGON_DESCENT,
            turn_limit: DRAGON_TURN_LIMIT,
            terrain_mask: u32::MAX,
            maximum_tile_slope: DRAGON_MAX_TILE_SLOPE,
        },
    );
    source.position = transition.position;
    source.yaw = transition.yaw;
    state.movement_blocked = transition.movement_blocked;
    if transition.killed {
        source.life = -1;
    }
    transition.killed
}

/// Consume the common creature mailbox/child-chain prefix shared by Dragon
/// actions 1 and 2. A damaged body segment is observed only when the root's
/// later numeric visit reaches this prefix.
fn consume_dragon_damage(
    source: &mut Level2AuthoredClass5Source,
    runtime: &mut Level2AuthoredClass5RuntimeState,
    parts: &[AuthoredDragonPart],
    terrain: &SessionTerrain,
) -> bool {
    let mut damaged = false;
    if source.pending_word_0x62 != 0 {
        source.life = source.life.wrapping_sub(runtime.pending_damage);
        source.word_0x26 = source.pending_word_0x62;
        source.pending_word_0x62 = 0;
        runtime.pending_damage = 0;
        damaged = true;
    } else {
        source.word_0x26 = 0;
    }

    let mut child_slot = runtime.linked_head;
    while child_slot != 0 {
        let Some(child) = parts
            .iter()
            .find(|part| part.source_entity_index == child_slot)
        else {
            break;
        };
        if child.life < source.life {
            source.life = child.life;
            source.word_0x26 = child.word_0x26;
            damaged = true;
            break;
        }
        child_slot = child.next_entity_index;
    }

    // Source 1BF90/1C310 checks negative life even without a fresh mailbox
    // or lower child: terrain 57390 can write life directly before this visit.
    if !damaged && source.life >= 0 {
        return false;
    }
    if source.life < 0 {
        source.word_0x24 = source.word_0x26;
        source.action = 4;
    } else {
        source.word_0x96_150 = source.word_0x26;
        source.action = DRAGON_ATTACK_ACTION;
        let ground = terrain.ground_height(source.position);
        if source.position.z > ground {
            source.position.z = source.position.z.wrapping_sub(4);
        }
        if source.position.z <= ground {
            source.position.z = ground;
        }
    }
    true
}

fn step_dragon_tails(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredDragonRuntime,
    terrain: &SessionTerrain,
    map_chains: &crate::renderable_map_chain::RenderableMapChains,
    projectiles: &[DragonProjectileThreat],
) {
    step_dragon_font_counter(source, state, map_chains, projectiles);
    step_dragon_vertical(source, terrain);
}

fn step_dragon_vertical(source: &mut Level2AuthoredClass5Source, terrain: &SessionTerrain) {
    source.position.z = source.position.z.wrapping_add(source.dword_0x10 as i16);
    source.dword_0x10 = source.dword_0x10.wrapping_sub(5);
    let ground = terrain.ground_height(source.position);
    if source.position.z < ground.wrapping_add(256) {
        source.dword_0x10 = 150;
    } else if let Some(ceiling) = terrain.ceiling_height(source.position)
        && source.position.z > ceiling.wrapping_sub(256)
    {
        source.dword_0x10 = -150;
    }
}

/// Source sub_1F0C0, activated by a guided projectile's real target-selection
/// write. The counter and dodge fields remain on the existing Dragon rows.
fn step_dragon_font_counter(
    source: &mut Level2AuthoredClass5Source,
    state: &mut AuthoredDragonRuntime,
    map_chains: &crate::renderable_map_chain::RenderableMapChains,
    projectiles: &[DragonProjectileThreat],
) {
    if state.font_type_index == 0 {
        return;
    }
    state.font_type_index = state.font_type_index.wrapping_sub(1);
    if source.byte_0x46_70 != 0 {
        if state.dodge_remaining != 0 {
            let threat = projectiles
                .iter()
                .find(|row| row.source_slot == source.word_0x24);
            if let Some(threat) =
                threat.filter(|row| row.life >= 0 && row.flags & ENTITY_DRAW_DISABLED_BIT == 0)
            {
                let yaw = if source.word_0x24 & 1 != 0 {
                    threat.yaw.wrapping_add(512)
                } else {
                    threat.yaw.wrapping_sub(512)
                } & 2047;
                crate::move_entity(
                    &mut source.position,
                    yaw,
                    0,
                    48_i16.wrapping_mul(state.dodge_remaining),
                );
                state.dodge_remaining = state.dodge_remaining.wrapping_sub(1);
            } else {
                source.byte_0x46_70 = 0;
                source.word_0x24 = 0;
            }
        } else {
            source.byte_0x46_70 = 0;
            source.word_0x24 = 0;
        }
    } else {
        let threat = crate::level1_mission::source_map_search_slots(
            map_chains,
            source.position,
            &crate::level1_mission::source_search_offsets(4),
        )
        .into_iter()
        .find(|slot| {
            projectiles
                .iter()
                .any(|row| row.source_slot == *slot && row.target == source.source_entity_index)
        });
        if let Some(threat) = threat {
            state.dodge_remaining = 5;
            source.byte_0x46_70 = source.byte_0x46_70.wrapping_add(1);
            source.word_0x24 = threat;
        }
    }
}

fn first_close_dragon(
    source: Level2AuthoredClass5Source,
    frozen_model0_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    separation: i16,
) -> Option<Level2AuthoredClass5Source> {
    frozen_model0_slots
        .iter()
        .find_map(|slot| {
            roots.iter().find(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.source_entity_index == *slot
                    && peer.source_entity_index != source.source_entity_index
                    && peer.model == DRAGON_MODEL
                    && i32::from(
                        peer.position
                            .x
                            .wrapping_sub(source.position.x)
                            .cast_signed(),
                    )
                    .abs()
                        < i32::from(separation)
                    && i32::from(
                        peer.position
                            .y
                            .wrapping_sub(source.position.y)
                            .cast_signed(),
                    )
                    .abs()
                        < i32::from(separation)
            })
        })
        .copied()
}

fn distance_3d_wrapped(left: Position, right: Position) -> u32 {
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

fn apply_dragon_wander(source: &mut Level2AuthoredClass5Source) {
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let direction = source.entity_rng;
    source.entity_rng = level2_next_rng16(source.entity_rng);
    let sign = if direction % 157 < 79 { -1 } else { 1 };
    let delta =
        i16::try_from((source.entity_rng & 0x00ff) + 85).expect("Dragon wander delta fits i16");
    source.roll = source.roll.wrapping_add(sign * delta) & 0x07ff;
}

fn nearest_dragon_player(
    source: Level2AuthoredClass5Source,
    players: &[GoatPlayerTarget],
) -> Option<u16> {
    let range_squared = u32::from(DRAGON_RANGE).pow(2);
    players
        .iter()
        .filter(|player| player.control_flags & 0x20 == 0)
        .filter_map(|player| {
            let distance = squared_wrapping_xy_distance(source.position, player.position);
            let heading = crate::heading_for_positions(source.position, player.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < DRAGON_VIEW_HALF_ANGLE)
                .then_some((distance, player.entity_index))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, entity_index)| entity_index)
}

fn nearest_unlinked_dragon(
    source: Level2AuthoredClass5Source,
    frozen_model0_slots: &[u16],
    roots: &[Level2AuthoredClass5Source],
    dragons: &[AuthoredDragonRuntime],
) -> Option<u16> {
    let range_squared = u32::from(DRAGON_RANGE).pow(2);
    frozen_model0_slots
        .iter()
        .filter_map(|slot| {
            let peer = roots.iter().find(|peer| {
                peer.active
                    && peer.life >= 0
                    && peer.source_entity_index == *slot
                    && peer.source_entity_index != source.source_entity_index
                    && peer.model == DRAGON_MODEL
                    && dragons
                        .iter()
                        .find(|state| state.source_entity_index == *slot)
                        .is_some_and(|state| state.linked_entity_index == 0)
            })?;
            let distance = squared_wrapping_xy_distance(source.position, peer.position);
            let heading = crate::heading_for_positions(source.position, peer.position);
            (distance <= range_squared
                && crate::angle_distance(i32::from(source.yaw), heading) < DRAGON_VIEW_HALF_ANGLE)
                .then_some((distance, *slot))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, slot)| slot)
}

fn squared_wrapping_xy_distance(left: Position, right: Position) -> u32 {
    let dx = i32::from(right.x.wrapping_sub(left.x).cast_signed());
    let dy = i32::from(right.y.wrapping_sub(left.y).cast_signed());
    dx.wrapping_mul(dx)
        .cast_unsigned()
        .wrapping_add(dy.wrapping_mul(dy).cast_unsigned())
}

#[cfg(test)]
mod lightning_consumer_tests {
    use super::*;

    #[test]
    fn alliance_impact_controls_dragon_attack_and_returns_original_actor() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let mut runtime = session.session.state.live.core.runtime.clone();
        let owner = session.state.entity_index;
        let position = session.state.position;
        let dragon = arena.economy.allocate_authored_dragon(575, position, DRAGON_LIFE, &terrain)
            .expect("real Dragon constructor");
        let slot = dragon.source_entity_index;
        arena.economy.authored_class5.iter_mut()
            .find(|row| row.source_entity_index == slot).unwrap().source_byte_3e = 0;
        bind_authored_stage_in_arena(&mut arena, &mut runtime, slot, 6).unwrap();
        assert_eq!(arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap().stage_var2, 2);
        let target = arena.allocate_source_slot_with_terrain(&mut terrain).expect("target slot");
        let mut target_row = crate::world::terrain::cave_features::new_event(target, 1, position);
        target_row.class = 3;
        target_row.model = 0;
        target_row.record[67] = 3;
        target_row.set_life(1_000);
        arena.entities[usize::from(target)] = target_row;
        arena.entities[usize::from(owner)].set_life(1_000);
        arena.entities[usize::from(owner)].set_source_word_96(target);
        let effect = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain)
            .expect("Alliance terminal");
        let effect_row = &mut arena.entities[usize::from(effect)];
        effect_row.source_id = owner;
        effect_row.record[30..32].copy_from_slice(&owner.to_le_bytes());
        effect_row.set_source_word_96(slot);
        effect_row.set_subspell_index(3);
        effect_row.record[74] = 16;
        let impact = crate::native_world::alliance_spell::step_impact(&mut arena, effect, &[slot]).unwrap();
        assert_eq!(impact.affected_entities, vec![slot]);
        let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((state.stage_var2, state.parent_entity_index, state.entity_id), (14, owner, slot));
        assert_eq!((state.stage_remaining, state.alliance_duration), (3, 3));
        assert!(arena.entities[usize::from(effect)].drawing_disabled());
        let step = step_alliance_stage_slot(&mut arena, slot, &[slot], &[], &mut terrain, Default::default()).unwrap();
        assert!(step.retained_for_epilogue);
        assert_eq!(step.spell_feedback, vec![(owner, 24)]);
        let source = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((source.action, source.word_0x96_150), (DRAGON_ATTACK_ACTION, target));
        let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!(state.stage_remaining, 2);
        assert_eq!(state.control_flags & 0x0600_0000, 0x0400_0000);
        assert!(arena.economy.authored_dragon_parts.iter().filter(|part| part.active && part.entity_id == slot)
            .all(|part| part.control_flags & 0x0600_0000 == 0x0400_0000));
        let attack = step_attack_in_arena(&mut arena, &mut terrain, slot, &[slot], Default::default()).unwrap();
        assert!(attack.fireball.is_some(), "the converted Dragon reaches its real attack producer");
        assert_eq!(arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap().stage_remaining, 1);
        let denied = step_attack_in_arena(&mut arena, &mut terrain, slot, &[slot], Default::default()).unwrap();
        assert!(denied.fireball.is_none());
        prepare_boss_stage_pass(&mut arena, &[slot]);
        step_alliance_stage_slot(&mut arena, slot, &[slot], &[], &mut terrain, Default::default()).unwrap();
        let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((state.stage_var2, state.parent_entity_index, state.entity_id), (10, 0, slot));
        assert_eq!(state.stage_var1, 6, "Alliance restores the source StageVar index");
        assert_eq!(state.control_flags & 0x0600_0000, 0);
        assert!(arena.economy.authored_dragon_parts.iter().filter(|part| part.active && part.entity_id == slot)
            .all(|part| part.control_flags & 0x0600_0000 == 0));
        prepare_alliance_stage_pass(&mut arena, &mut runtime, &[slot], Default::default()).unwrap();
        let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!((state.stage_var1, state.stage_var2), (6, 2));
        let resumed = step_authored_stage_in_arena(&mut arena, &runtime, &mut terrain, slot, &[], &[slot], &[], Default::default()).unwrap();
        assert!(resumed.retained_for_epilogue, "restored source guard executes on the original Dragon");

        // The same owner-level runtime supplies patrol kind1 and wander kind6.
        // Reapply the actual terminal and expire each binding before visiting
        // its restored callback, rather than only testing the route helper.
        for (stage_index, mode) in [(1, 1), (2, 6)] {
            bind_authored_stage_in_arena(&mut arena, &mut runtime, slot, stage_index).unwrap();
            let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!((state.stage_var1, state.stage_var2), (stage_index, mode));
            let effect = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
            let row = &mut arena.entities[usize::from(effect)];
            row.source_id = owner;
            row.record[30..32].copy_from_slice(&owner.to_le_bytes());
            row.set_source_word_96(slot);
            row.set_subspell_index(1);
            row.record[74] = 16;
            let impact = crate::native_world::alliance_spell::step_impact(&mut arena, effect, &[slot]).unwrap();
            assert_eq!(impact.affected_entities, vec![slot]);
            step_alliance_stage_slot(&mut arena, slot, &[slot], &[], &mut terrain, Default::default()).unwrap();
            let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!((state.stage_var1, state.stage_var2), (stage_index, 10));
            prepare_alliance_stage_pass(&mut arena, &mut runtime, &[slot], Default::default()).unwrap();
            let state = arena.economy.authored_dragons.iter().find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!((state.stage_var1, state.stage_var2), (stage_index, mode));
            let resumed = step_authored_stage_in_arena(&mut arena, &runtime, &mut terrain, slot, &[], &[slot], &[], Default::default()).unwrap();
            assert!(resumed.retained_for_epilogue, "restored Dragon stage {mode} executes");
        }
    }

    #[test]
    fn boss_dragon_constructor_launch_parent_loss_and_reclaim_use_real_parts() {
        let session = crate::native_world::tests::level_two_campaign_session();
        let mut arena = session.source_arena().clone();
        let mut terrain = session.session.state.live.terrain.clone();
        let particles = &session.session.definition.presentation.particles;
        let parent_slot = session.state.entity_index;
        let parent = arena.handle(parent_slot).expect("live source parent");
        let handle = allocate_boss_summoned(
            &mut arena,
            &mut terrain,
            session.state.position,
            parent,
            parent_slot,
            parent_slot,
            0,
        )
        .unwrap()
        .expect("real Dragon has capacity");
        let slot = handle.source_slot;
        assert_eq!(arena.creature_parent_binding(handle), Some(parent));
        let state = arena.economy.authored_dragons.iter()
            .find(|row| row.source_entity_index == slot).unwrap();
        assert_eq!(state.entity_id, parent_slot);
        assert_eq!(arena.economy.source_owner(slot), 0);
        let current = crate::native_world::source_target::read(
            &arena, crate::native_world::source_target::Context::default(), slot,
        ).unwrap().unwrap();
        assert_eq!(current.source_id, parent_slot, "boss root keeps the boss ID");
        let mut other_id = arena.clone();
        other_id.economy.authored_dragons.iter_mut()
            .find(|row| row.source_entity_index == slot).unwrap().entity_id ^= 1;
        assert_ne!(arena.reset_hash_with_terrain(&terrain), other_id.reset_hash_with_terrain(&terrain));
        assert_eq!(
            arena
                .economy
                .authored_dragon_parts
                .iter()
                .filter(|part| part.active && part.entity_id == slot)
                .count(),
            DRAGON_PART_COUNT
        );
        let part_slot = arena.economy.authored_dragon_parts.iter()
            .find(|part| part.active && part.entity_id == slot).unwrap().source_entity_index;
        // The typed body is a non-player target. A stale raw shadow must not
        // override its current life/position, and hiding the same body rejects it.
        let mut target_arena = arena.clone();
        target_arena.entities[usize::from(part_slot)].set_life(-1);
        let target = current_attack_target(&target_arena, Default::default(), part_slot).unwrap().unwrap();
        assert_eq!(target.position, target_arena.economy.authored_dragon_parts.iter()
            .find(|part| part.source_entity_index == part_slot).unwrap().position);
        let mut hidden_target = target_arena.clone();
        let hidden_part = hidden_target.economy.authored_dragon_parts.iter_mut()
            .find(|part| part.source_entity_index == part_slot).unwrap();
        hidden_part.drawing_enabled = false;
        hidden_part.control_flags |= 0x400;
        assert!(current_attack_target(&hidden_target, Default::default(), part_slot).unwrap().is_none());
        for hidden in [false, true] {
            let mut attack = if hidden { hidden_target.clone() } else { target_arena.clone() };
            let mut attack_world = terrain.clone();
            let source = attack.economy.authored_class5.iter_mut()
                .find(|row| row.source_entity_index == slot).unwrap();
            source.action = DRAGON_ATTACK_ACTION;
            source.source_byte_3e = 0;
            source.word_0x96_150 = part_slot;
            let step = step_attack_in_arena(&mut attack, &mut attack_world, slot, &[],
                Default::default()).unwrap();
            let source = attack.economy.authored_class5.iter()
                .find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!(step.fireball.is_some(), !hidden);
            assert_eq!(source.action, if hidden { DRAGON_ROAM_ACTION } else { DRAGON_ATTACK_ACTION });
            if let Some(launch) = step.fireball { assert_eq!(launch.target_entity_index, part_slot); }
        }
        let mut self_target = arena.clone();
        let mut self_world = terrain.clone();
        let source = self_target.economy.authored_class5.iter_mut()
            .find(|row| row.source_entity_index == slot).unwrap();
        source.action = DRAGON_ATTACK_ACTION;
        source.source_byte_3e = 0;
        source.word_0x96_150 = slot;
        let before = source.position;
        let step = step_attack_in_arena(&mut self_target, &mut self_world, slot, &[],
            Default::default()).unwrap();
        let moved = self_target.economy.authored_class5.iter()
            .find(|row| row.source_entity_index == slot).unwrap().position;
        assert_ne!(moved, before, "source motion must precede the target read");
        assert_eq!(step.fireball.unwrap().target_position, moved);
        for (is_part, suppressed) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut dying = arena.clone();
            let mut world = terrain.clone();
            let death_slot = if is_part { part_slot } else { slot };
            let old_handle = dying.handle(death_slot).unwrap();
            let events_before = dying.economy.events().len();
            let (position, expected_mana) = if is_part {
                let part = dying.economy.authored_dragon_parts.iter_mut()
                    .find(|part| part.source_entity_index == death_slot).unwrap();
                part.action = DRAGON_KILL_ACTION;
                part.source_byte_3e = 0;
                if suppressed { part.control_flags |= 0x10_0000; }
                (part.position, part.mana)
            } else {
                let root = dying.economy.authored_class5.iter_mut()
                    .find(|root| root.source_entity_index == death_slot).unwrap();
                root.action = DRAGON_KILL_ACTION;
                root.source_byte_3e = 0;
                let position = root.position;
                let mana = root.mana;
                if suppressed {
                    dying.economy.authored_dragons.iter_mut()
                        .find(|state| state.source_entity_index == death_slot).unwrap().control_flags |= 0x10_0000;
                }
                (position, mana)
            };
            if is_part {
                dying.economy.authored_dragon_parts.iter_mut()
                    .find(|part| part.source_entity_index == death_slot).unwrap()
                    .previous_entity_index = u16::MAX;
                let mut following = dying.clone();
                following.economy.authored_dragon_parts.iter_mut()
                    .find(|part| part.source_entity_index == death_slot).unwrap()
                    .action = DRAGON_PART_ACTION;
                assert!(step_part_in_arena(
                    &mut following, &mut world, particles, death_slot,
                    source_target::Context::default(),
                ).is_err(), "a live follower still requires its actual predecessor");
                assert!(step_part_in_arena(
                    &mut dying, &mut world, particles, death_slot,
                    source_target::Context::default(),
                ).unwrap());
                assert!(dying.economy.authored_dragon_parts.iter().any(|part|
                    part.source_entity_index == death_slot && part.active && !part.drawing_enabled));
            } else {
                let result = step_lifecycle_in_arena(&mut dying, &mut world, particles, death_slot).unwrap();
                assert!(result.handled && result.retained_for_epilogue);
                assert!(dying.economy.authored_class5.iter().any(|root|
                    root.source_entity_index == death_slot && root.active && !root.drawing_enabled));
            }
            assert!(dying.resolve(old_handle).is_some(), "death stays allocated through the source tail");
            assert!(dying.economy.events().iter().skip(events_before).any(|event| matches!(
                event, crate::native_world::Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index, mana, ..
                } if *source_entity_index == death_slot && *mana == expected_mana
            )), "Dragon death converts its real mana at the source address");
            let burst_id = dying.economy.events().iter().find_map(|event| match event {
                crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                    parent_entity_index, id_entity_index, ..
                } if *parent_entity_index == death_slot => Some(*id_entity_index),
                _ => None,
            });
            if suppressed { assert!(burst_id.is_none()); } else {
                assert_eq!(burst_id, Some(if is_part { slot } else { parent_slot }));
            }
            let tile = ((position.y >> 8) << 8) | (position.x >> 8);
            prepare_retiring_dragon_rows(&mut dying, &mut world).unwrap();
            retire_hidden_boss_dragons_at_source_boundary(&mut dying);
            dying.economy.reclaim_inactive_slots_at_source_boundary();
            assert!(dying.resolve(old_handle).is_none());
            assert!(!dying.economy.map_chains.entity_chain(tile as u8, (tile >> 8) as u8).contains(&death_slot));
            assert_eq!(world.planes().entity_indices[usize::from(tile)], dying.economy.map_chains.tile_head(tile) as i16);
        }
        let source = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        source.actual_speed = 24;
        source.source_byte_3e = 1;
        assert!(
            step_boss_stage_slot(
                &mut arena,
                &mut terrain,
                particles,
                slot,
                &[],
                &[],
                &[],
                &[],
                Default::default()
            )
            .unwrap()
            .retained_for_epilogue
        );
        let state = arena
            .economy
            .authored_dragons
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!((state.stage_var2, state.stage_remaining), (16, 250));
        for attacker in [parent_slot, 112] {
            let mut hit = arena.clone();
            let mut world = terrain.clone();
            let root = hit.economy.authored_class5.iter_mut()
                .find(|row| row.source_entity_index == slot).unwrap();
            root.word_0x96_150 = 0;
            root.pending_word_0x62 = attacker;
            hit.economy.authored_class5_runtime.iter_mut()
                .find(|row| row.source_entity_index == slot).unwrap().pending_damage = 1;
            let outcome = step_boss_stage_slot(&mut hit, &mut world, particles, slot,
                &[], &[], &[], &[], Default::default()).unwrap();
            assert!(outcome.retained_for_epilogue);
            assert_eq!(outcome.spell_feedback, (attacker != parent_slot).then_some((parent_slot, 19)));
            let root = hit.economy.authored_class5.iter()
                .find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!(root.pending_word_0x62, 0);
            assert_eq!(root.life, DRAGON_LIFE - 1);
            assert_eq!(root.action, if attacker == parent_slot { DRAGON_STAGE_ACTION } else { DRAGON_ATTACK_ACTION });
        }
        for parent_alive in [true, false] {
            let mut typed = arena.clone();
            let mut world = terrain.clone();
            let mut runtime = session.session.state.live.core.runtime.clone();
            let typed_parent = super::super::firefly::allocate_summoned(&mut typed, &mut runtime,
                &mut world, particles, session.state.position, parent_slot, parent_slot, parent_slot)
                .unwrap().unwrap();
            let row = typed.economy.authored_class5.iter_mut()
                .find(|row| row.source_entity_index == typed_parent.source_slot).unwrap();
            row.life = if parent_alive { 600 } else { -1 };
            typed.entities[usize::from(typed_parent.source_slot)].set_life(if parent_alive { -1 } else { 600 });
            let state = typed.economy.authored_dragons.iter_mut()
                .find(|state| state.source_entity_index == slot).unwrap();
            state.parent_entity_index = typed_parent.source_slot;
            state.parent_generation = typed_parent.generation;
            let outcome = step_boss_stage_slot(&mut typed, &mut world, particles, slot,
                &[], &[], &[], &[], Default::default()).unwrap();
            assert!(outcome.retained_for_epilogue);
            let root = typed.economy.authored_class5.iter()
                .find(|row| row.source_entity_index == slot).unwrap();
            assert_eq!(root.drawing_enabled, parent_alive);
        }
        let root = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        root.action = DRAGON_ROAM_ACTION;
        prepare_boss_stage_pass(&mut arena, &[slot]);
        let root = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(root.action, DRAGON_STAGE_ACTION);
        root.action = DRAGON_ATTACK_ACTION;
        prepare_boss_stage_pass(&mut arena, &[slot]);
        let root = arena
            .economy
            .authored_class5
            .iter_mut()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert_eq!(root.action, DRAGON_ATTACK_ACTION);
        root.action = DRAGON_STAGE_ACTION;
        arena.entities[usize::from(parent_slot)].disable_drawing();
        assert!(
            step_boss_stage_slot(
                &mut arena,
                &mut terrain,
                particles,
                slot,
                &[],
                &[],
                &[],
                &[],
                Default::default()
            )
            .unwrap()
            .retained_for_epilogue
        );
        let root = arena
            .economy
            .authored_class5
            .iter()
            .find(|row| row.source_entity_index == slot)
            .unwrap();
        assert!(
            root.active && !root.drawing_enabled,
            "root remains for its same-pass tail"
        );
        let tile = ((root.position.x >> 8) as u8, (root.position.y >> 8) as u8);
        assert!(
            arena
                .economy
                .map_chains
                .entity_chain(tile.0, tile.1)
                .contains(&slot)
        );
        let final_root_position = root.position;
        let final_root_yaw = root.yaw;
        prepare_retiring_dragon_rows(&mut arena, &mut terrain).unwrap();
        retire_hidden_boss_dragons_at_source_boundary(&mut arena);
        arena
            .rebuild_source_arena_with_terrain(&mut terrain, &[])
            .unwrap();
        assert!(arena.resolve(handle).is_none());
        assert_eq!(
            arena.entities[usize::from(slot)].position,
            final_root_position
        );
        assert_eq!(arena.entities[usize::from(slot)].yaw, final_root_yaw);
        assert!(
            !arena
                .economy
                .map_chains
                .entity_chain(tile.0, tile.1)
                .contains(&slot)
        );
        let mut predecessor_slot = slot;
        let mut parts = Vec::new();
        for _ in 0..DRAGON_PART_COUNT {
            let part = arena
                .economy
                .authored_dragon_parts()
                .iter()
                .find(|part| part.active && part.previous_entity_index == predecessor_slot)
                .expect("boss Dragon has the complete linked part chain");
            let part_slot = part.source_entity_index;
            parts.push(arena.handle(part_slot).unwrap());
            predecessor_slot = part_slot;
        }
        let flags_hash_before = arena.economy.reset_hash();
        arena
            .economy
            .authored_dragon_parts
            .iter_mut()
            .find(|part| part.source_entity_index == parts[0].source_slot)
            .unwrap()
            .control_flags ^= 0x0010_0000;
        assert_ne!(
            arena.economy.reset_hash(),
            flags_hash_before,
            "per-part flags affect future-state hash"
        );
        arena
            .economy
            .authored_dragon_parts
            .iter_mut()
            .find(|part| part.source_entity_index == parts[0].source_slot)
            .unwrap()
            .control_flags ^= 0x0010_0000;
        assert_eq!(arena.economy.reset_hash(), flags_hash_before);
        for (index, part_handle) in parts.iter().copied().enumerate() {
            let previous_slot = if index == 0 {
                slot
            } else {
                parts[index - 1].source_slot
            };
            let predecessor = &arena.entities[usize::from(previous_slot)];
            let missing = DragonPredecessor {
                class: predecessor.class,
                position: predecessor.position,
                yaw: predecessor.yaw,
            };
            assert_ne!(missing.class, 5, "preceding generation has been reclaimed");
            let part = arena
                .economy
                .authored_dragon_parts
                .iter_mut()
                .find(|part| part.source_entity_index == part_handle.source_slot)
                .unwrap();
            part.source_byte_39 = 1;
            part.pending_word_0x62 = parent_slot;
            part.pending_damage = 23;
            let old_life = part.life;
            assert!(arena.economy.step_authored_dragon_part_slot(
                part_handle.source_slot,
                &terrain,
                missing
            ));
            let part = arena
                .economy
                .authored_dragon_parts()
                .iter()
                .find(|part| part.source_entity_index == part_handle.source_slot)
                .unwrap();
            assert!(part.active && !part.drawing_enabled);
            assert_ne!(part.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
            assert_ne!(
                crate::native_world::area_damage::read_arena_actor(&arena, part_handle.source_slot)
                    .unwrap()
                    .control_flags
                    & ENTITY_DRAW_DISABLED_BIT,
                0
            );
            assert_eq!(
                (part.life, part.word_0x26, part.pending_word_0x62),
                (old_life - 23, parent_slot, 0)
            );
            let part_position = part.position;
            let part_yaw = part.yaw;
            let part_tile = ((part_position.x >> 8) as u8, (part_position.y >> 8) as u8);
            assert!(
                arena
                    .economy
                    .map_chains
                    .entity_chain(part_tile.0, part_tile.1)
                    .contains(&part_handle.source_slot)
            );
            if let Some(next) = parts.get(index + 1) {
                let next_part = arena
                    .economy
                    .authored_dragon_parts
                    .iter_mut()
                    .find(|part| part.source_entity_index == next.source_slot)
                    .unwrap();
                next_part.source_byte_39 = 0;
                next_part.source_byte_3e = 0;
                let hidden_predecessor = DragonPredecessor {
                    class: 5,
                    position: part_position,
                    yaw: part_yaw,
                };
                assert!(arena.economy.step_authored_dragon_part_slot(
                    next.source_slot,
                    &terrain,
                    hidden_predecessor
                ));
                let next_part = arena
                    .economy
                    .authored_dragon_parts()
                    .iter()
                    .find(|part| part.source_entity_index == next.source_slot)
                    .unwrap();
                assert!(
                    next_part.active && next_part.drawing_enabled,
                    "hidden predecessor stays class5 in the same pass"
                );
                assert_eq!(next_part.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
                assert_eq!(
                    (next_part.position, next_part.yaw),
                    (part_position, part_yaw)
                );
            }
            prepare_retiring_dragon_rows(&mut arena, &mut terrain).unwrap();
            retire_hidden_boss_dragons_at_source_boundary(&mut arena);
            arena
                .rebuild_source_arena_with_terrain(&mut terrain, &[])
                .unwrap();
            assert!(arena.resolve(part_handle).is_none());
            assert_eq!(
                arena.entities[usize::from(part_handle.source_slot)].position,
                part_position
            );
            assert_eq!(
                arena.entities[usize::from(part_handle.source_slot)].yaw,
                part_yaw
            );
            assert!(
                !arena
                    .economy
                    .map_chains
                    .entity_chain(part_tile.0, part_tile.1)
                    .contains(&part_handle.source_slot)
            );
        }
        assert!(
            allocate_boss_summoned(
                &mut arena,
                &mut terrain,
                session.state.position,
                SourceEntityHandle {
                    source_slot: parent_slot,
                    generation: parent.generation.wrapping_add(1)
                },
                parent_slot,
                parent_slot,
                0
            )
            .is_err()
        );
    }

    #[test]
    fn selected_dragon_tracks_current_projectile_and_clears_hidden_threat() {
        let mut session = crate::native_world::tests::level_two_campaign_session();
        let live = &mut session.session.state.live;
        let mut source = live
            .economy
            .economy
            .allocate_authored_dragon(575, live.core.state.position, DRAGON_LIFE, &live.terrain)
            .unwrap();
        let mut state = *live
            .economy
            .economy
            .authored_dragons
            .iter()
            .find(|state| state.source_entity_index == source.source_entity_index)
            .unwrap();
        state.font_type_index = 32;
        let slot = if source.source_entity_index & 1 == 0 {
            915
        } else {
            914
        };
        live.economy
            .economy
            .map_chains
            .sync_entity(RenderableMapSeed {
                entity_index: slot,
                active: true,
                x: source.position.x,
                y: source.position.y,
            })
            .unwrap();
        let mut projectile = DragonProjectileThreat {
            source_slot: slot,
            target: source.source_entity_index,
            life: 1,
            flags: 4,
            yaw: 0,
        };
        let rng = source.entity_rng;
        let position = source.position;
        step_dragon_font_counter(
            &mut source,
            &mut state,
            &live.economy.economy.map_chains,
            &[projectile],
        );
        assert_eq!(
            (
                state.font_type_index,
                state.dodge_remaining,
                source.byte_0x46_70,
                source.word_0x24
            ),
            (31, 5, 1, slot)
        );
        assert_eq!(source.position, position, "first scan arms the next visit");
        let mut expected = position;
        crate::move_entity(
            &mut expected,
            if slot & 1 != 0 { 512 } else { 1536 },
            0,
            240,
        );
        step_dragon_font_counter(
            &mut source,
            &mut state,
            &live.economy.economy.map_chains,
            &[projectile],
        );
        assert_eq!(
            source.position, expected,
            "dodge side uses projectile-slot parity"
        );
        assert_eq!(
            (
                state.font_type_index,
                state.dodge_remaining,
                source.entity_rng
            ),
            (30, 4, rng)
        );
        projectile.flags |= ENTITY_DRAW_DISABLED_BIT;
        step_dragon_font_counter(
            &mut source,
            &mut state,
            &live.economy.economy.map_chains,
            &[projectile],
        );
        assert_eq!(
            (
                state.font_type_index,
                state.dodge_remaining,
                source.byte_0x46_70,
                source.word_0x24
            ),
            (29, 4, 0, 0)
        );
        assert_eq!(source.position, expected);
    }
}
