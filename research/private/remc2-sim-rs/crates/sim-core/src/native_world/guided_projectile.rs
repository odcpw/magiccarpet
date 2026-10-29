//! The shared sub_65820 flyer used by Meteor and Spider web.
//! SourceArena owns fixed addresses; the scheduler owns phase and reclaim.

pub(crate) mod boss;

use super::{EntitySnapshot, ParticleParameter, SessionTerrain, area_damage};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::{EffectKind, FieldKind, TilePosition};
use crate::level1_mission::{
    projectile_guidance_angles, source_map_search_slots, source_search_offsets,
};
use crate::spells::support::{ReboundProjectile, ReboundSpread, ReboundTarget, reflect_projectile};
use crate::{FlightWorld, Position, SIN_16_16, angle_distance, move_entity, turn_toward};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Guidance {
    pub yaw_turn: i16,
    pub pitch_turn: i16,
}

/// Lists frozen at UpdateEntities' ordinary pass prefix, in source order.
pub(crate) struct Targets<'a> {
    pub class3: &'a [u16],
    pub class5: &'a [Vec<u16>; 29],
    pub class10: &'a [u16],
    pub owner_target_range: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct StepOutcome {
    pub impact_slot: Option<u16>,
    pub experience: Option<(u16, u8, u16)>,
    pub sounds: Vec<(u16, u16)>,
    pub mana_updates: Vec<(u16, i32)>,
    pub music_player: Option<u16>,
    pub accuracy: Option<(u16, bool)>,
    pub captured_holder: Option<u16>,
}

/// Source `sub_65780`: only its projectile model allowlist increments the
/// human player's accuracy counters. The hit counts only when the actual
/// recipient and the projectile's captured target belong to the same source
/// actor. A `None` result means this source row is not an accuracy attempt.
pub(super) fn source_accuracy(
    arena: &SourceArena,
    projectile_model: u8,
    owner_slot: u16,
    hit_slot: Option<u16>,
    original_target: u16,
) -> Option<(u16, bool)> {
    if !matches!(projectile_model, 0 | 1 | 3 | 7..=9 | 12 | 17 | 25 | 28) {
        return None;
    }
    let owner = arena.entities.get(usize::from(owner_slot))?;
    if (owner.class, owner.model) != (3, 0) {
        return None;
    }
    let matched = hit_slot
        .zip((original_target != 0).then_some(original_target))
        .is_some_and(|(hit, target)| {
            arena.entities[usize::from(hit)].source_id
                == arena.entities[usize::from(target)].source_id
        });
    Some((owner_slot, matched))
}

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_owner(row: &mut EntitySnapshot, owner: u16) {
    row.source_id = owner;
    put(row, 30, owner);
}

pub(crate) fn centre(position: Position, model: u8, vertical_offset: i16) -> Position {
    Position {
        z: if model == 2 {
            position.z
        } else {
            position.z.wrapping_add(vertical_offset)
        },
        ..position
    }
}

fn target_position(
    arena: &SourceArena,
    slot: u16,
    context: super::source_target::Context<'_>,
) -> Result<Position, String> {
    let target = super::source_target::read(arena, context, slot)?
        .ok_or("guided projectile target address is outside arena")?;
    if target.model == 2 {
        return Ok(target.position);
    }
    let shape = target
        .collision
        .ok_or_else(|| format!("guided projectile target {slot} lacks current source axes"))?;
    Ok(centre(target.position, target.model, shape.vertical_offset))
}

fn aim_at(row: &mut EntitySnapshot, position: Position) {
    let (yaw, pitch) = projectile_guidance_angles(row.position, position);
    row.set_source_roll(yaw);
    row.set_fov(pitch);
}

/// Acquisition reads the current byte39/StageVar2/parentId, not the decoded
/// packet left at an address now occupied by a typed creature. Membership
/// stays frozen; this projects only fields used by sub_67CB0's class5 gate.
fn class5_gate_fields(arena: &SourceArena, slot: u16) -> Option<(u8, u8, u16)> {
    let fields = if let Some(row) = arena
        .authored_class5()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        let stage = arena
            .authored_class5_archer()
            .iter()
            .find(|state| state.source_entity_index == slot)
            .map(|state| (state.stage.mode, state.stage.parent))
            .or_else(|| {
                arena
                    .authored_fireflies
                    .iter()
                    .find(|state| state.source_entity_index == slot)
                    .map(|state| (state.stage_var2, 0))
            })
            .unwrap_or((0, 0));
        Some((row.source_byte_39, stage.0, stage.1))
    } else if let Some(row) = arena
        .authored_dragon_parts()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        Some((row.source_byte_39, 0, row.parent_entity_index))
    } else if let Some(row) = arena
        .authored_class5_model3_children()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        Some((row.source_byte_39, 0, row.parent_entity_index))
    } else if let Some(row) = arena
        .house_residents()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        Some((row.source_byte_39, row.stage_var_2 as u8, row.stage.parent))
    } else if let Some(row) = arena
        .bee_creatures()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        Some((
            row.source_byte_39 as u8,
            row.stage_var2,
            row.parent_entity_index,
        ))
    } else if let Some(row) = arena
        .castle_defenders()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        Some((row.source_byte_39, row.stage.mode, row.stage.parent))
    } else if let Some(row) = arena
        .castle_children()
        .iter()
        .find(|row| row.active && row.source_entity_index == slot)
    {
        Some((row.source_byte_39, 0, row.parent_entity_index))
    } else if arena.live_arena_cursor_row(slot).is_some() {
        // A different current typed occupant cannot expose an old packet.
        None
    } else {
        arena
            .entities
            .get(usize::from(slot))
            .map(|row| (row.record[61], row.record[77], word(row, 44)))
    };
    fields
}

pub(super) fn class5_eligible(arena: &SourceArena, slot: u16, owner: u16, ray: bool) -> bool {
    class5_gate_fields(arena, slot).is_some_and(|(cadence, stage, parent)| {
        // The ray's source branch compares word2C; ordinary guided shots
        // compare parentId. Keep that distinction even for allied actors.
        let allegiance = if ray {
            word(&arena.entities[usize::from(slot)], 48)
        } else {
            parent
        };
        cadence != 0 && !(stage == 14 && allegiance == owner)
    })
}

/// sub_10780 consumes sub_10130 only while it returns 1. Its final call
/// supplies a coordinate but returns 2, so the last cell of the final search
/// class is not visited. A negative limit still starts with class zero.
pub(super) fn collision_search_offsets(pitch: i16) -> Vec<(i8, i8)> {
    let width = i32::from(pitch) + 255;
    // my_sign32 is -1 for negative inputs and zero otherwise.
    let limit = ((width + if width < 0 { 255 } else { 0 }) >> 8).max(0) as u8;
    let mut offsets = source_search_offsets(limit);
    offsets.pop();
    offsets
}

/// sub_10780: preserve map-chain order, selectors and strict source extents.
pub(super) fn collision(
    arena: &SourceArena,
    row: &EntitySnapshot,
    context: super::source_target::Context<'_>,
) -> Result<Option<(u16, Position)>, String> {
    for slot in source_map_search_slots(
        &arena.economy.map_chains,
        row.position,
        &collision_search_offsets(row.array_pitch()),
    ) {
        let Some(actor) = super::source_target::read(arena, context, slot)? else {
            continue;
        };
        if actor.control_flags & 8 == 0
            || actor.source_id == row.source_id
            || !(row.record[69] == 255
                || row.record[69] == actor.class
                    && (row.record[70] == 255 || row.record[70] == actor.model))
        {
            continue;
        }
        let shape = actor.collision.ok_or_else(|| {
            format!("projectile collision target {slot} lacks current source axes")
        })?;
        let dx = i32::from(actor.position.x.wrapping_sub(row.position.x) as i16).abs();
        let dy = i32::from(actor.position.y.wrapping_sub(row.position.y) as i16).abs();
        let dz = (i32::from(actor.position.z) + i32::from(shape.vertical_offset)
            - i32::from(row.position.z)
            - i32::from(row.array_yaw()))
        .abs();
        if dx < i32::from(row.array_pitch()) + i32::from(shape.half_width)
            && dy < i32::from(row.array_roll()) + i32::from(shape.half_depth)
            && dz < i32::from(row.array_fov()) + i32::from(shape.half_height)
        {
            return Ok(Some((
                slot,
                centre(actor.position, actor.model, shape.vertical_offset),
            )));
        }
    }
    Ok(None)
}

/// sub_68490/685D0 use wrapped XY distance despite the source helper's name.
fn score(row: &EntitySnapshot, position: Position) -> u32 {
    score_with_pitch(row, position, 113)
}

pub(super) fn score_with_pitch(row: &EntitySnapshot, position: Position, pitch_limit: i32) -> u32 {
    let (yaw, pitch) = projectile_guidance_angles(row.position, position);
    let yaw = angle_distance(i32::from(row.yaw), i32::from(yaw));
    let pitch = angle_distance(i32::from(row.pitch), i32::from(pitch));
    if yaw > 113 || i32::from(pitch) > pitch_limit {
        return u32::MAX;
    }
    let dx = i32::from(position.x.wrapping_sub(row.position.x) as i16);
    let dy = i32::from(position.y.wrapping_sub(row.position.y) as i16);
    let distance =
        crate::legacy_sqrt(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as u32) as i32;
    if distance > 5120 {
        return u32::MAX;
    }
    let yaw_forward = distance.wrapping_mul(SIN_16_16[512 + yaw as usize]) >> 16;
    let yaw_side = distance
        .wrapping_mul(SIN_16_16[yaw as usize])
        .wrapping_mul(4)
        >> 16;
    let pitch_forward = distance.wrapping_mul(SIN_16_16[512 + pitch as usize]) >> 16;
    let pitch_side = 4_i32
        .wrapping_mul(SIN_16_16[pitch as usize])
        .wrapping_mul(distance)
        >> 16;
    yaw_forward
        .wrapping_mul(yaw_forward)
        .wrapping_add(yaw_side.wrapping_mul(yaw_side))
        .wrapping_add(pitch_forward.wrapping_mul(pitch_forward))
        .wrapping_add(pitch_side.wrapping_mul(pitch_side)) as u32
}

/// sub_68940 precedes ordinary acquisition, and deliberately measures range
/// from the trap to the projectile's owner rather than to the projectile.
pub(super) fn acquire_capture(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    targets: &Targets<'_>,
    context: super::source_target::Context<'_>,
) -> Result<bool, String> {
    // sub_68940 rejects other models before consulting the owner or traps.
    if !matches!(row.model, 0 | 2..=5 | 8..=9 | 12 | 22..=23 | 26 | 28 | 30) {
        return Ok(false);
    }
    let Some(owner) = arena.entities.get(usize::from(row.source_id)) else {
        return Err("guided projectile owner address is outside arena".to_owned());
    };
    if row.source_id == 0 || owner.class != 3 {
        return Ok(false);
    }
    let owner_position = owner.position;
    let mut best = 65536;
    let mut selected = None;
    for &slot in targets.class10 {
        let target = &arena.entities[usize::from(slot)];
        if target.model != 78 || word(target, 54) != row.source_id || word(target, 58) != u16::MAX {
            continue;
        }
        // sub_583F0 takes the radix after wrapping each signed-16 delta and
        // the 32-bit sum; the economy's similarly named helper is squared.
        let dx = i32::from(owner_position.x.wrapping_sub(target.position.x) as i16);
        let dy = i32::from(owner_position.y.wrapping_sub(target.position.y) as i16);
        let dz = i32::from(owner_position.z.wrapping_sub(target.position.z));
        let distance = crate::legacy_sqrt(
            dx.wrapping_mul(dx)
                .wrapping_add(dy.wrapping_mul(dy))
                .wrapping_add(dz.wrapping_mul(dz)) as u32,
        );
        if distance < u32::from(targets.owner_target_range) && distance < best {
            let yaw = crate::heading_for_positions(row.position, target.position);
            if angle_distance(i32::from(row.yaw), i32::from(yaw)) < 170 {
                best = distance;
                selected = Some(slot);
            }
        }
    }
    if let Some(slot) = selected {
        put(row, 154, slot);
        aim_at(row, target_position(arena, slot, context)?);
    }
    Ok(selected.is_some())
}

/// Models3/4 share this sub_67CB0 branch. The other terrain flyers may
/// acquire a capture trap, but are absent from ordinary target acquisition.
fn acquire(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    targets: &Targets<'_>,
    outcome: &mut StepOutcome,
    context: super::source_target::Context<'_>,
) -> Result<bool, String> {
    if acquire_capture(arena, row, targets, context)? {
        return Ok(true);
    }
    let mut best = u32::MAX;
    let mut selected = None;
    for &slot in targets.class3 {
        let Some(target) = super::source_target::read(arena, context, slot)? else {
            continue;
        };
        if target.source_id == row.source_id || target.control_flags & 0x20 != 0 {
            continue;
        }
        // Literal source compares distance(target.position,target.position)
        // <= owner.word1C. Its self-distance is zero for every unsigned range.
        let value = score(row, target_position(arena, slot, context)?);
        if value < best {
            best = value;
            selected = Some(slot);
        }
    }
    for (model, slots) in targets.class5.iter().enumerate() {
        if model == 22 {
            continue;
        }
        for &slot in slots {
            let Some(target) = super::source_target::read(arena, context, slot)? else {
                continue;
            };
            if target.source_id == row.source_id
                || !class5_eligible(arena, slot, row.source_id, false)
            {
                continue;
            }
            let value = score(row, target_position(arena, slot, context)?);
            if value < best {
                best = value;
                selected = Some(slot);
            }
        }
    }
    if selected.is_none() {
        for &root in &targets.class5[22] {
            let root = &arena.entities[usize::from(root)];
            if root.record[61] == 0 {
                continue;
            }
            let mut child = word(root, 56);
            let mut visited = 0;
            while child != 0 {
                if visited == arena.entities.len() {
                    return Err("guided projectile model22 child chain is cyclic".to_owned());
                }
                visited += 1;
                let value = score(row, target_position(arena, child, context)?);
                if value < best {
                    best = value;
                    selected = Some(child);
                }
                child = word(&arena.entities[usize::from(child)], 56);
            }
        }
    }
    if let Some(slot) = selected {
        put(row, 154, slot);
        aim_at(row, target_position(arena, slot, context)?);
        let identity = super::source_target::read(arena, context, slot)?
            .map(|actor| (actor.class, actor.model));
        if identity == Some((5, 0)) {
            let target = &mut arena.entities[usize::from(slot)];
            target.font_type_index = 32;
            target.record[65] = 32; // sub_68BD0, fontTypeIndex.
            if let Some(dragon) = arena
                .economy
                .authored_dragons
                .iter_mut()
                .find(|dragon| dragon.source_entity_index == slot)
            {
                dragon.font_type_index = 32;
            }
        }
        if identity == Some((3, 0)) {
            outcome.music_player = Some(slot); // sub_5EF70.
        }
    }
    Ok(selected.is_some())
}

/// sub_67CB0 models7/8 search only class3, bounded by the owner's descriptor
/// range. Model8 first admits the source capture-trap branch; model7 does not.
fn acquire_directional(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    targets: &Targets<'_>,
    outcome: &mut StepOutcome,
    context: super::source_target::Context<'_>,
) -> Result<bool, String> {
    if acquire_capture(arena, row, targets, context)? {
        return Ok(true);
    }
    let mut best = u32::MAX;
    let mut selected = None;
    for &slot in targets.class3 {
        let Some(target) = super::source_target::read(arena, context, slot)? else {
            continue;
        };
        if target.class != 3
            || target.source_id == row.source_id
            || target.control_flags & 0x20 != 0
        {
            continue;
        }
        let position = target_position(arena, slot, context)?;
        let dx = i32::from(target.position.x.wrapping_sub(row.position.x) as i16);
        let dy = i32::from(target.position.y.wrapping_sub(row.position.y) as i16);
        let dz = i32::from(target.position.z) - i32::from(row.position.z);
        let distance = crate::legacy_sqrt(
            dx.wrapping_mul(dx)
                .wrapping_add(dy.wrapping_mul(dy))
                .wrapping_add(dz.wrapping_mul(dz)) as u32,
        );
        if distance > u32::from(targets.owner_target_range) {
            continue;
        }
        let value = score(row, position);
        if value < best {
            best = value;
            selected = Some((slot, target, position));
        }
    }
    if let Some((slot, target, position)) = selected {
        put(row, 154, slot);
        aim_at(row, position);
        if (target.class, target.model) == (3, 0) {
            outcome.music_player = Some(slot);
        }
    }
    Ok(selected.is_some())
}

/// sub_67CB0 model25 searches only the current class5 families admitted by
/// sub_3A7F0. Keep frozen list order while reading each live StageVar carrier.
fn acquire_alliance(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    targets: &Targets<'_>,
    context: super::source_target::Context<'_>,
) -> Result<bool, String> {
    let mut best = u32::MAX;
    let mut selected = None;
    for slots in targets.class5 {
        for &slot in slots {
            let Some(actor) = super::source_target::read(arena, context, slot)? else {
                continue;
            };
            if actor.source_id == row.source_id {
                continue;
            }
            let Some((cadence, stage, _)) = class5_gate_fields(arena, slot) else {
                continue;
            };
            let mut target = arena.entities[usize::from(slot)].clone();
            target.class = actor.class;
            target.model = actor.model;
            target.record[77] = stage;
            if cadence == 0 || !crate::spells::late::alliance::eligible_target(&target) {
                continue;
            }
            let value = score(row, target_position(arena, slot, context)?);
            if value < best {
                best = value;
                selected = Some(slot);
            }
        }
    }
    if let Some(slot) = selected {
        put(row, 154, slot);
        aim_at(row, target_position(arena, slot, context)?);
    }
    Ok(selected.is_some())
}

fn reflect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    hit: u16,
    outcome: &mut StepOutcome,
) -> Result<bool, String> {
    let row = arena.entities[usize::from(slot)].clone();
    // Of the two callers, only Meteor's 10/17 product is reflectable.
    if (row.record[71], row.record[72]) != (10, 17) {
        return Ok(false);
    }
    let target = &arena.entities[usize::from(hit)];
    let mut target = ReboundTarget {
        source_id: target.source_id,
        flags: target.control_flags,
        mana: target.mana,
        position: target.position,
        fov: target.array_fov(),
    };
    if let Some(actor) = area_damage::source_actor(arena, None, hit) {
        target.source_id = actor.source_id;
        target.flags = actor.control_flags;
        target.position = actor.position;
        target.fov = actor.shape.half_height;
    }
    let mut projectile = ReboundProjectile {
        owner: row.source_id,
        target: word(&row, 154),
        position: row.position,
        yaw: row.yaw,
        pitch: row.pitch,
        roll: row.source_roll(),
        fov: row.fov,
        damage: row.subspell_index,
        mana: row.mana,
        life: row.life,
        max_life: row.max_life,
        rng: row.entity_rng,
    };
    let Some(mana) = reflect_projectile(&mut projectile, target, ReboundSpread::Narrow) else {
        return Ok(false);
    };
    let owner = &arena.entities[usize::from(projectile.target)];
    let selectors = (owner.class, owner.model);
    arena.entities[usize::from(hit)].set_mana(mana);
    outcome.mana_updates.push((hit, mana));
    outcome.sounds.push((hit, 28));
    outcome.experience = Some((hit, 8, 1));
    let row = &mut arena.entities[usize::from(slot)];
    set_owner(row, projectile.owner);
    put(row, 154, projectile.target);
    row.record[69] = selectors.0;
    row.record[70] = selectors.1;
    row.set_yaw(projectile.yaw);
    row.set_pitch(projectile.pitch);
    row.set_source_roll(projectile.roll);
    row.set_fov(projectile.fov);
    row.set_subspell_index(projectile.damage);
    row.set_life(projectile.life);
    row.set_entity_rng(projectile.rng);
    arena.economy.source_rng.insert(slot, projectile.rng);
    crate::world::effects::sync_position(arena, terrain, slot, projectile.position, true, false)?;
    Ok(true)
}

pub(super) fn absorb(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    hit: Option<u16>,
    outcome: &mut StepOutcome,
) -> Result<bool, String> {
    let row = &arena.entities[usize::from(slot)];
    let Some(hit) = hit.filter(|_| matches!(row.model, 3 | 9 | 12)) else {
        return Ok(false);
    };
    let target = &arena.entities[usize::from(hit)];
    if (target.class, target.model) != (10, 78)
        || word(target, 54) != row.source_id
        || word(target, 58) != u16::MAX
    {
        return Ok(false);
    }
    let position = row.position;
    let visual = crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
        position,
        particles,
    )?;
    if visual == Some(slot) {
        // The original call passes &projectile.position through NewEvent.
        crate::world::effects::sync_position(
            arena,
            terrain,
            slot,
            Position::default(),
            true,
            false,
        )?;
    }
    outcome.sounds.push((slot, 26));
    // NewEvent may have replaced any of these fixed addresses.
    let holder = word(&arena.entities[usize::from(slot)], 42);
    if holder != 0
        && let Some(capture) =
            crate::world::fields::magic_mine::capture(&mut arena.entities, hit, slot)
    {
        outcome.captured_holder = Some(capture.holder_slot);
    }
    Ok(true)
}

fn construct_impact(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
) -> Result<Option<u16>, String> {
    let parent = &arena.entities[usize::from(slot)];
    let position = parent.position;
    match (parent.record[71], parent.record[72]) {
        (10, 72) => super::summon_army::construct_ring_impact(arena, terrain, particles, slot),
        (10, 74) => Ok(super::alliance_spell::construct_impact(arena, terrain)),
        (10, 25 | 26) => {
            super::directional_spells::construct_impact(arena, terrain, particles, slot)
        }
        (10, 22) => crate::world::fields::construct_spell_wind(arena, terrain, particles, position),
        (10, 67) => crate::world::fields::gravity::construct(arena, terrain, position, 20_000, 120),
        (10, 9 | 11 | 15 | 71) => {
            let projectile = parent.clone();
            super::disaster_spells::construct_effect(arena, terrain, &projectile)
        }
        (10, model @ (65 | 66)) => crate::world::fields::status::construct(
            arena,
            terrain,
            position,
            sim_definition::adapter::mc2_field_definition(
                if model == 65 {
                    FieldKind::Slowing
                } else {
                    FieldKind::Web
                },
                TilePosition { x: 0, y: 0 },
            ),
        ),
        (10, 17) => {
            let Some(impact) = arena.allocate_source_slot_with_terrain(terrain) else {
                return Ok(None);
            };
            // The constructor receives &parent.position across NewEvent's memset.
            let position = if impact == slot {
                Position::default()
            } else {
                position
            };
            let source = arena
                .economy
                .initialize_authored_model17(impact, u16::MAX, position);
            let mut row = super::cave_features::new_event(impact, source.entity_rng, position);
            row.class = 10;
            row.model = 17;
            row.record[67] = 10;
            row.record[68] = 17;
            row.set_source_action(17);
            row.set_subspell_index(source.subspell_index);
            row.set_max_life(i32::from(source.max_life));
            row.set_life(i32::from(source.life));
            row.set_control_flags(source.control_flags);
            arena.entities[usize::from(impact)] = row;
            Ok(Some(impact))
        }
        selector => Err(format!(
            "guided projectile has unsupported impact {selector:?}"
        )),
    }
}

pub(crate) fn step(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    guidance: Guidance,
    targets: Option<&Targets<'_>>,
    context: super::source_target::Context<'_>,
) -> Result<StepOutcome, String> {
    let context = super::source_target::Context {
        particles: Some(particles),
        map_type: Some(terrain.map_type()),
        ..context
    };
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.active
                && matches!(
                    (row.class, row.model, row.action),
                    (9, 2, 2)
                        | (9, 3, 3)
                        | (9, 4, 4)
                        | (9, 5, 5)
                        | (9, 7, 7)
                        | (9, 8, 8)
                        | (9, 20, 21)
                        | (9, 21, 22)
                        | (9, 22, 23)
                        | (9, 23, 24)
                        | (9, 24, 25)
                        | (9, 25, 26)
                        | (9, 26, 27)
                )
        })
        .ok_or_else(|| format!("guided projectile callback lost slot{slot}"))?
        .clone();
    let mut outcome = StepOutcome::default();
    let original_target = word(&row, 154);
    if original_target != 0 {
        aim_at(&mut row, target_position(arena, original_target, context)?);
        row.set_yaw(
            row.yaw
                .wrapping_add(turn_toward(row.yaw, row.source_roll(), guidance.yaw_turn))
                & 2047,
        );
        row.set_pitch(
            row.pitch
                .wrapping_add(turn_toward(row.pitch, row.fov, guidance.pitch_turn))
                & 2047,
        );
    } else if row.control_flags & 2 == 0 {
        row.set_control_flags(row.control_flags | 2);
        let acquired = if let Some(targets) = targets {
            match row.model {
                7 | 8 => acquire_directional(arena, &mut row, targets, &mut outcome, context)?,
                25 => acquire_alliance(arena, &mut row, targets, context)?,
                3 | 4 | 22 | 26 => acquire(arena, &mut row, targets, &mut outcome, context)?,
                2 | 5 | 20 | 23 => acquire_capture(arena, &mut row, targets, context)?,
                _ => false,
            }
        } else {
            false
        };
        if acquired {
            row.set_yaw(row.source_roll());
            row.set_pitch(row.fov);
        } else {
            row.set_source_roll(row.yaw);
            row.set_fov(row.pitch);
        }
    }
    let delta = i32::from(row.minimum_speed()) - i32::from(row.actual_speed);
    row.set_actual_speed(row.actual_speed.wrapping_add((2 * delta.signum()) as i16));
    let mut position = row.position;
    move_entity(&mut position, row.yaw, row.pitch, row.actual_speed);
    row.set_position(position);
    arena.entities[usize::from(slot)] = row.clone();
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let hit = collision(arena, &row, context)?;
    let terminal = if let Some((target, position)) = hit {
        if reflect(arena, terrain, slot, target, &mut outcome)? {
            return Ok(outcome);
        }
        row.set_position(position);
        true
    } else {
        let ground = terrain.ground_height(row.position);
        let limit = if ground > row.position.z {
            Some(ground)
        } else {
            terrain
                .ceiling_height(row.position)
                .map(|ceiling| ceiling.wrapping_sub(row.array_fov()))
                .filter(|limit| row.position.z > *limit)
        };
        if let Some(z) = limit {
            row.set_position(Position { z, ..row.position });
            arena.entities[usize::from(slot)] = row.clone();
            if !matches!(row.model, 4 | 22 | 24 | 26) && terrain.material_mask(row.position) == 1 {
                let splash = crate::world::effects::allocate(
                    arena,
                    terrain,
                    sim_definition::adapter::mc2_effect_definition(EffectKind::Ash),
                    row.position,
                    particles,
                )?;
                if let Some(splash) = splash {
                    if splash == slot {
                        let mut position = Position::default();
                        position.z = terrain.ground_height(position);
                        crate::world::effects::sync_position(
                            arena, terrain, splash, position, true, false,
                        )?;
                    }
                    let owner = arena.entities[usize::from(slot)].source_id;
                    set_owner(&mut arena.entities[usize::from(splash)], owner);
                }
                arena.entities[usize::from(slot)].disable_drawing();
                return Ok(outcome);
            }
            true
        } else {
            row.set_life(row.life.wrapping_sub(1));
            row.life < 0
        }
    };
    let position = row.position;
    arena.entities[usize::from(slot)] = row;
    if hit.is_some() {
        crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    }
    if !terminal {
        return Ok(outcome);
    }
    let hit_slot = hit.map(|(slot, _)| slot);
    if absorb(arena, terrain, particles, slot, hit_slot, &mut outcome)? {
        arena.entities[usize::from(slot)].disable_drawing();
        return Ok(outcome);
    }
    let Some(impact_slot) = construct_impact(arena, terrain, particles, slot)? else {
        // Source retries on the next visit; allocation failure does not hide.
        return Ok(outcome);
    };
    outcome.impact_slot = Some(impact_slot);
    let current = arena.entities[usize::from(slot)].clone();
    // sub_65780 runs after allocation, against the current fixed addresses.
    outcome.accuracy = source_accuracy(
        arena,
        current.model,
        current.source_id,
        hit_slot,
        original_target,
    );
    if let Some(target) = hit_slot {
        // sub_686D0 precedes the spell XP award.
        if let Some(owner) = arena.entities.get_mut(usize::from(current.source_id))
            && owner.class == 3
            && matches!(owner.model, 0 | 1)
        {
            put(owner, 154, target);
        }
        let holder = word(&current, 42);
        if holder != 0 {
            let model = arena
                .entities
                .get(usize::from(holder))
                .ok_or("guided projectile spell holder address is outside arena")?
                .model;
            outcome.experience = Some((current.source_id, model, 1));
        }
    }
    let impact = &mut arena.entities[usize::from(impact_slot)];
    set_owner(impact, current.source_id);
    impact.set_yaw(current.yaw);
    impact.set_pitch(current.pitch);
    if let Some(target) = hit_slot {
        put(impact, 154, target);
    }
    impact.set_subspell_index(current.subspell_index);
    impact.record[74] = current.record[74];
    // Existing Meteor17 is authoritative; publish the source constructor's
    // post-allocation ownership/damage writes to that same carrier.
    if let Some(effect) = arena
        .economy
        .authored_model17
        .iter_mut()
        .find(|effect| effect.active && effect.source_entity_index == impact_slot)
    {
        effect.entity_id = current.source_id;
        effect.yaw = current.yaw;
        effect.subspell_index = current.subspell_index;
    }
    crate::world::fields::sync_wind_impact(arena, impact_slot);
    arena.entities[usize::from(slot)].disable_drawing();
    if impact_slot == slot
        && let Some(effect) = arena
            .economy
            .authored_model17
            .iter_mut()
            .find(|effect| effect.active && effect.source_entity_index == slot)
    {
        effect.control_flags = arena.entities[usize::from(slot)].control_flags;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_collision_search_excludes_final_cell_and_keeps_negative_limit_bounded() {
        let ordinary = collision_search_offsets(48);
        assert!(ordinary.contains(&(-1, 0)));
        assert!(!ordinary.contains(&(2, 2)));
        assert_eq!(ordinary.len(), 15);
        assert_eq!(collision_search_offsets(-512), vec![(0, 0), (1, 0), (0, 1)]);
    }

    #[test]
    fn accepted_mine_collision_without_holder_hides_flyer_without_capturing_zero() {
        let mut simulation = super::super::tests::level_two_campaign_session();
        let particles = simulation.session.definition.presentation.particles.clone();
        let live = &mut simulation.session.state.live;
        let owner = live.core.state.entity_index;
        let search_offsets = source_search_offsets(1);
        let tile = (0..65_536_u32)
            .map(|tile| Position {
                x: (((tile & 255) << 8) | 128) as u16,
                y: (((tile >> 8) << 8) | 128) as u16,
                z: 0,
            })
            .find(|position| {
                source_map_search_slots(&live.economy.map_chains, *position, &search_offsets)
                    .is_empty()
            })
            .expect("Level2 fixture has an empty collision cell");
        let position = Position {
            z: live.terrain.ground_height(tile),
            ..tile
        };
        let mine_slot = crate::world::fields::magic_mine::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            *particles
                .get(66)
                .expect("Level2 fixture retains Magic Mine particle66"),
        )
        .unwrap()
        .expect("Magic Mine source slot available");
        assert_ne!(owner, 0);
        assert_ne!(mine_slot, owner);
        {
            let mine = &mut live.economy.entities[usize::from(mine_slot)];
            mine.record[54..56].copy_from_slice(&owner.to_le_bytes());
            mine.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
            mine.record[90..92].copy_from_slice(&256_i16.to_le_bytes());
            mine.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
        }

        let flyer_slot = live
            .economy
            .allocate_source_slot_with_terrain(&mut live.terrain)
            .expect("guided flyer source slot available");
        let mut flyer = crate::world::terrain::cave_features::new_event(
            flyer_slot,
            live.economy.new_event_rng(flyer_slot),
            position,
        );
        flyer.class = 9;
        flyer.model = 3;
        flyer.record[67] = 9;
        flyer.record[68] = 3;
        flyer.set_source_action(3);
        flyer.source_id = owner;
        flyer.record[30..32].copy_from_slice(&owner.to_le_bytes());
        flyer.record[42..44].fill(0);
        flyer.record[69] = 10;
        flyer.record[70] = 78;
        flyer.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
        flyer.record[90..92].copy_from_slice(&256_i16.to_le_bytes());
        flyer.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
        flyer.set_actual_speed(0);
        flyer.record[136..138].copy_from_slice(&0_i16.to_le_bytes());
        flyer.set_life(100);
        live.economy.source_rng.insert(flyer_slot, flyer.entity_rng);
        live.economy.entities[usize::from(flyer_slot)] = flyer;
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            flyer_slot,
            position,
            true,
            false,
        )
        .unwrap();

        let zero_before = live.economy.entities[0].clone();
        let outcome = step(
            &mut live.economy,
            &mut live.terrain,
            &particles,
            flyer_slot,
            Guidance {
                yaw_turn: 0,
                pitch_turn: 0,
            },
            None,
            super::super::source_target::Context {
                spells: Some(&live.shared_spells),
                ..super::super::source_target::Context::from_ordinary(&live.goats, &live.ordinary)
            },
        )
        .expect("accepted model78 collision is absorbed");

        assert_eq!(outcome.captured_holder, None);
        assert_eq!(outcome.sounds.as_slice(), &[(flyer_slot, 26)]);
        assert!(live.economy.entities[usize::from(flyer_slot)].drawing_disabled());
        assert_eq!(
            u16::from_le_bytes(
                live.economy.entities[usize::from(mine_slot)].record[58..60]
                    .try_into()
                    .unwrap()
            ),
            u16::MAX
        );
        assert_eq!(
            live.economy.entities[0].active_value,
            zero_before.active_value
        );
        assert_eq!(
            &live.economy.entities[0].record[50..52],
            &zero_before.record[50..52]
        );
    }
}
