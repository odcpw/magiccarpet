//! The shared sub_65820 flyer used by Lightning and Spider web.
//! SourceArena owns fixed addresses; the scheduler owns phase and reclaim.

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

fn target_position(arena: &mut SourceArena, slot: u16) -> Result<Position, String> {
    let raw = arena
        .entities
        .get(usize::from(slot))
        .ok_or("guided projectile target address is outside arena")?;
    let position = centre(raw.position, raw.model, raw.array_yaw());
    Ok(
        area_damage::source_actor(arena, None, slot).map_or(position, |actor| {
            centre(actor.position, actor.model, actor.shape.vertical_offset)
        }),
    )
}

fn aim_at(row: &mut EntitySnapshot, position: Position) {
    let (yaw, pitch) = projectile_guidance_angles(row.position, position);
    row.set_source_roll(yaw);
    row.set_fov(pitch);
}

/// Acquisition reads the current byte39/StageVar2/parentId, not the decoded
/// packet left at an address now occupied by a typed creature. Membership
/// stays frozen; this projects only fields used by sub_67CB0's class5 gate.
fn class5_eligible(arena: &SourceArena, slot: u16, owner: u16) -> bool {
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
    fields.is_some_and(|(cadence, stage, parent)| cadence != 0 && !(stage == 14 && parent == owner))
}

/// sub_10780: preserve map-chain order, selectors and strict source extents.
fn collision(arena: &mut SourceArena, row: &EntitySnapshot) -> Option<(u16, Position)> {
    let width = i32::from(row.array_pitch()) + 255;
    let radius = ((width - width.signum() * 255) >> 8) as u8;
    for slot in source_map_search_slots(
        &arena.economy.map_chains,
        row.position,
        &source_search_offsets(radius),
    ) {
        let Some(actor) = area_damage::source_actor(arena, None, slot) else {
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
        let dx = i32::from(actor.position.x.wrapping_sub(row.position.x) as i16).abs();
        let dy = i32::from(actor.position.y.wrapping_sub(row.position.y) as i16).abs();
        let dz = (i32::from(actor.position.z) + i32::from(actor.shape.vertical_offset)
            - i32::from(row.position.z)
            - i32::from(row.array_yaw()))
        .abs();
        if dx < i32::from(row.array_pitch()) + i32::from(actor.shape.half_width)
            && dy < i32::from(row.array_roll()) + i32::from(actor.shape.half_depth)
            && dz < i32::from(row.array_fov()) + i32::from(actor.shape.half_height)
        {
            return Some((
                slot,
                centre(actor.position, actor.model, actor.shape.vertical_offset),
            ));
        }
    }
    None
}

/// sub_68490/685D0 use wrapped XY distance despite the source helper's name.
fn score(row: &EntitySnapshot, position: Position) -> u32 {
    let (yaw, pitch) = projectile_guidance_angles(row.position, position);
    let yaw = angle_distance(i32::from(row.yaw), i32::from(yaw));
    let pitch = angle_distance(i32::from(row.pitch), i32::from(pitch));
    if yaw > 113 || pitch > 113 {
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
fn acquire_capture(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    targets: &Targets<'_>,
) -> Result<bool, String> {
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
        aim_at(row, target_position(arena, slot)?);
    }
    Ok(selected.is_some())
}

/// Only model3's exercised sub_67CB0 branch is admitted here. Web21 is
/// explicitly absent from the original acquisition selectors.
fn acquire(
    arena: &mut SourceArena,
    row: &mut EntitySnapshot,
    targets: &Targets<'_>,
    outcome: &mut StepOutcome,
) -> Result<bool, String> {
    if acquire_capture(arena, row, targets)? {
        return Ok(true);
    }
    let mut best = u32::MAX;
    let mut selected = None;
    for &slot in targets.class3 {
        let target = &arena.entities[usize::from(slot)];
        if target.source_id == row.source_id || target.control_flags & 0x20 != 0 {
            continue;
        }
        // Literal source compares distance(target.position,target.position)
        // <= owner.word1C. Its self-distance is zero for every unsigned range.
        let value = score(row, target_position(arena, slot)?);
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
            let Some(target) = area_damage::source_actor(arena, None, slot) else {
                continue;
            };
            if target.source_id == row.source_id || !class5_eligible(arena, slot, row.source_id) {
                continue;
            }
            let value = score(
                row,
                centre(target.position, target.model, target.shape.vertical_offset),
            );
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
                let value = score(row, target_position(arena, child)?);
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
        aim_at(row, target_position(arena, slot)?);
        let identity =
            area_damage::source_actor(arena, None, slot).map(|actor| (actor.class, actor.model));
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

fn reflect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
    hit: u16,
    outcome: &mut StepOutcome,
) -> Result<bool, String> {
    let row = arena.entities[usize::from(slot)].clone();
    // Of the two callers, only Lightning's 10/17 product is reflectable.
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

fn absorb(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    hit: Option<u16>,
    outcome: &mut StepOutcome,
) -> Result<bool, String> {
    let row = &arena.entities[usize::from(slot)];
    let Some(hit) = hit.filter(|_| row.model == 3) else {
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
    if holder != 0 {
        let row = arena
            .entities
            .get(usize::from(holder))
            .ok_or("guided projectile captured holder address is outside arena")?;
        let (model, duration) = (row.model, row.record[74]);
        let target = &mut arena.entities[usize::from(hit)];
        put(target, 58, u16::from(model));
        put(target, 56, u16::from(duration));
        let row = &mut arena.entities[usize::from(holder)];
        row.active_value = 1;
        put(row, 50, 1);
        outcome.captured_holder = Some(holder);
    }
    Ok(true)
}

fn construct_impact(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    slot: u16,
) -> Result<Option<u16>, String> {
    let parent = &arena.entities[usize::from(slot)];
    let position = parent.position;
    match (parent.record[71], parent.record[72]) {
        (10, 66) => crate::world::fields::status::construct(
            arena,
            terrain,
            position,
            sim_definition::adapter::mc2_field_definition(
                FieldKind::Web,
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
) -> Result<StepOutcome, String> {
    let mut row = arena
        .entities
        .get(usize::from(slot))
        .filter(|row| {
            row.active && matches!((row.class, row.model, row.action), (9, 3, 3) | (9, 21, 22))
        })
        .ok_or_else(|| format!("guided projectile callback lost slot{slot}"))?
        .clone();
    let mut outcome = StepOutcome::default();
    let original_target = word(&row, 154);
    if original_target != 0 {
        aim_at(&mut row, target_position(arena, original_target)?);
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
        let acquired = if row.model == 3
            && let Some(targets) = targets
        {
            acquire(arena, &mut row, targets, &mut outcome)?
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
    let hit = collision(arena, &row);
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
            if terrain.material_mask(row.position) == 1 {
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
    let Some(impact_slot) = construct_impact(arena, terrain, slot)? else {
        // Source retries on the next visit; allocation failure does not hide.
        return Ok(outcome);
    };
    outcome.impact_slot = Some(impact_slot);
    let current = arena.entities[usize::from(slot)].clone();
    // sub_65780 runs after allocation, against the current fixed addresses.
    if matches!(current.model, 0 | 1 | 3 | 7..=9 | 12 | 17 | 25 | 28)
        && let Some(owner) = arena.entities.get(usize::from(current.source_id))
        && (owner.class, owner.model) == (3, 0)
    {
        let matched = hit_slot
            .zip((original_target != 0).then_some(original_target))
            .is_some_and(|(hit, target)| {
                arena.entities[usize::from(hit)].source_id
                    == arena.entities[usize::from(target)].source_id
            });
        outcome.accuracy = Some((current.source_id, matched));
    }
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
