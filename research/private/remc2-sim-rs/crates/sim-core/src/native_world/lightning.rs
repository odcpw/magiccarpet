//! Family7 Lightning and family22 LightningFire over the source arena.
//! Source scheduling, common phase advancement and following-pass reclaim
//! remain in the campaign adapter. No second projectile state is retained.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
use super::{area_damage, guided_projectile};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::LIGHTNING_PROPERTIES as RULES;
use crate::{FlightWorld, Position, move_entity};
pub(crate) use crate::spell_activation::{
    ProjectileSpellCastInput as CastInput, ProjectileSpellHolderOutcome as HolderOutcome,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Product { Ray, StormShot, Endpoint, Storm }

fn word(row: &EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}
fn put(row: &mut EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn owner(row: &mut EntitySnapshot, value: u16) {
    row.source_id = value;
    put(row, 30, value);
}

/// sub_6D8B0 only awards spell experience when its owner is a player entity.
fn source_player_owner(arena: &SourceArena, owner: u16) -> bool {
    owner != 0
        && arena
            .entities
            .get(usize::from(owner))
            .is_some_and(|row| row.class == 3 && row.model == 0)
}

/// The source `sub_58350` signed shortest-angle step. Its third argument is
/// ignored; the fourth is the maximum unsigned angle delta.
fn sub58350_delta(from: i16, to: i16, maximum: i16) -> i16 {
    let from = i32::from(from) & 0x07ff;
    let to = i32::from(to) & 0x07ff;
    let mut delta = to - from;
    if delta.abs() > 1024 {
        delta += if delta >= 0 { -2048 } else { 2048 };
    }
    if delta == 0 {
        return 0;
    }
    let magnitude = delta.abs().min(i32::from(maximum.max(0)));
    i16::try_from(magnitude * delta.signum()).expect("Lightning source turn delta fits i16")
}

/// Constructors4D860,4DA20,4F5F0,4FFB0, including their retained particles.
pub(crate) fn construct(
    arena: &mut SourceArena, terrain: &mut SessionTerrain,
    particles: &[ParticleParameter], product: Product, position: Position,
) -> Result<Option<u16>, String> {
    let (class, model, action, particle, life, parameter) = match product {
        Product::Ray => (9, 9, 9, 216, RULES.ray_range / i32::from(RULES.projectile_speed), 63),
        Product::StormShot => (9, 12, 12, 216, RULES.storm_shot_range / i32::from(RULES.projectile_speed), 60),
        Product::Endpoint => (10, 23, 23, 7, 8, 59),
        Product::Storm => (10, 38, 40, 272, 32, 59),
    };
    let particle_row = *particles.get(particle as usize)
        .ok_or_else(|| format!("Lightning requires retained particle{particle}"))?;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else { return Ok(None); };
    let mut row = super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = class;
    row.model = model;
    row.record[67] = class;
    row.record[68] = model;
    row.set_source_action(action);
    row.set_control_flags((row.control_flags & !8) | 4);
    row.parameter_relation = Some(parameter);
    row.record[164..168].copy_from_slice(&(u32::from(parameter) + 1).to_le_bytes());
    row.set_max_life(life);
    row.set_life(life);
    if class == 9 {
        row.set_actual_speed(RULES.projectile_speed);
        put(&mut row, 136, RULES.projectile_speed as u16);
        row.set_mana(50);
    }
    super::cave_features::bind_particle(&mut row, particle, particle_row)?;
    match product {
        Product::Endpoint => {
            row.set_subspell_index(25);
            row.set_control_flags((row.control_flags & !0x20008) | 0x20001);
            put(&mut row, 104, 200);
            put(&mut row, 106, 200);
        }
        Product::Storm => { put(&mut row, 104, 512); put(&mut row, 106, 512); }
        Product::Ray | Product::StormShot => {}
    }
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
    Ok(Some(slot))
}

fn launch(
    arena: &mut SourceArena, terrain: &mut SessionTerrain, particles: &[ParticleParameter],
    holder: &EntitySnapshot, parent_slot: u16, input: CastInput, side: usize,
) -> Result<Option<u16>, String> {
    let tier = crate::spell_activation::level2_applied_tier(holder.level_gate)
        .ok_or("Lightning holder tier is negative")?;
    let rule = RULES.tiers[usize::from(tier)];
    let parent = &arena.entities[usize::from(parent_slot)];
    let (position, inherited_speed) = (parent.position, parent.actual_speed);
    let product = if rule.product == 0 { Product::Ray } else { Product::StormShot };
    let Some(slot) = construct(arena, terrain, particles, product, position)? else { return Ok(None); };
    // NewEvent may replace a fixed address, so resolve the parent again.
    let parent = arena.entities[usize::from(parent_slot)].clone();
    let row = &mut arena.entities[usize::from(slot)];
    row.set_actual_speed(RULES.projectile_speed.wrapping_add(inherited_speed)
        .clamp(RULES.projectile_speed, RULES.maximum_launch_speed));
    row.record[71] = if rule.product == 0 { 10 } else { 9 };
    row.record[72] = if rule.product == 0 { 23 } else { 9 };
    let mut position = row.position;
    let hand = if parent.control_flags & 0x100 != 0 { Some(parent.yaw.wrapping_sub(512) & 2047) }
        else if parent.control_flags & 0x200 != 0 { Some(parent.yaw.wrapping_add(512) & 2047) }
        else { None };
    if let Some(yaw) = hand {
        move_entity(&mut position, yaw, 0, 256);
        if terrain.ground_height(position) > position.z { position = row.position; }
    }
    position.z = position.z.wrapping_add(parent.array_fov());
    row.set_position(position);
    put(row, 42, holder.index);
    owner(row, parent.source_id);
    row.set_mana(holder.mana);
    row.set_subspell_index(rule.damage);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));
    let yaw = parent.yaw.wrapping_add(input.aim_yaw);
    let pitch = parent.pitch.wrapping_add(input.aim_pitch);
    let mut axis = parent.position;
    move_entity(&mut axis, yaw, pitch, RULES.aim_distance);
    put(row, 158, axis.x); put(row, 160, axis.y); put(row, 162, axis.z as u16);
    row.set_yaw(if rule.product == 2 {
        yaw.wrapping_add(if side == 0 { 113 } else { -113 }) & 2047
    } else { yaw });
    row.set_pitch(pitch);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// sub_6A5C0. Allocation failure still charges and consumes the opening pulse.
pub(crate) fn step_holder(
    shared: &mut Level2SharedSpellResetSession, arena: &mut SourceArena,
    terrain: &mut SessionTerrain, particles: &[ParticleParameter], slot: u16,
    mut input: CastInput,
) -> Result<HolderOutcome, String> {
    let mut holder = shared.pickup(slot).cloned().ok_or("Lightning holder is absent")?;
    if (holder.class, holder.model, holder.action) != (15, 7, 21) {
        return Err("Lightning active holder has a different source selector".to_owned());
    }
    arena.entities[usize::from(slot)] = holder.clone();
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut result = HolderOutcome::default();
    if holder.active_value > 0 {
        if parent_slot != 0 && let Some(parent) = arena.entities.get(usize::from(parent_slot))
            .filter(|row| row.active && row.class == 3 && matches!(row.model, 0 | 1)) {
            let fresh = holder.active_value == holder.activation_value as i16;
            let admitted = parent.life >= 0 && parent.mana >= 0
                && (holder.mana_regen == 0 || input.castle_mana.is_some_and(|m| m >= holder.mana_regen))
                && (!fresh || parent.mana >= holder.max_mana);
            if admitted {
                if fresh {
                    let tier = crate::spell_activation::level2_applied_tier(holder.level_gate)
                        .ok_or("Lightning holder tier is negative")?;
                    let count = if RULES.tiers[usize::from(tier)].product == 2 { 2 } else { 1 };
                    let mut pair = [None, None];
                    for (side, shot) in pair.iter_mut().enumerate().take(count) {
                        *shot = launch(arena, terrain, particles, &holder, parent_slot, input, side)?;
                        if let Some(shot) = *shot {
                            result.projectile = result.projectile.or(Some(shot));
                            result.clear_player_aux = Some(parent_slot);
                            input.auxiliary_byte = 0;
                            arena.economy.prepare_sound(slot, parent_slot, -1, if tier == 0 { 23 } else { 9 });
                        }
                    }
                    // Pinned C++ dereferences the absent first allocation when
                    // only the second succeeds. Preserve surviving shots safely.
                    if let [Some(first), Some(second)] = pair {
                        put(&mut arena.entities[usize::from(first)], 56, second);
                        put(&mut arena.entities[usize::from(second)], 56, first);
                    }
                }
                let parent = &mut arena.entities[usize::from(parent_slot)];
                parent.set_mana_regen(if fresh {
                    if parent.mana_regen >= 0 { holder.max_mana.wrapping_neg() }
                    else { parent.mana_regen.wrapping_sub(holder.max_mana) }
                } else { parent.mana_regen.min(0) });
                shared.sync_player_entity_from_authority(parent);
                if parent_slot == shared.player_entity.index { shared.sync_typed_spell_parent_caches_from_player_entity(); }
            } else { holder.active_value = 1; }
        }
        holder.active_value = holder.active_value.wrapping_sub(1);
        if holder.active_value == 0 {
            let deferred = holder.source_word_2c();
            if deferred != 0 {
                let tier = crate::spell_activation::level2_applied_tier(deferred.wrapping_sub(1) as i8)
                    .ok_or("Lightning deferred tier is negative")?;
                let profile = crate::spell_activation::level2_set_spell_profile(7, tier)
                    .ok_or("Lightning deferred profile is absent")?;
                holder.apply_nyphur_spell_profile(profile, tier);
                holder.set_source_word_2c(0);
            }
        }
    }
    let cooldown = word(&holder, 58) as i16;
    if cooldown != 0 { put(&mut holder, 58, cooldown.wrapping_sub(1) as u16); }
    let active_value = holder.active_value as u16;
    put(&mut holder, 50, active_value);
    if (arena.entities[usize::from(slot)].class, arena.entities[usize::from(slot)].model) == (15, 7) {
        if let Some(row) = shared.pickup_mut(slot) { *row = holder.clone(); }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(result)
}

fn distance(a: Position, b: Position) -> u32 {
    let dx = i32::from(a.x.wrapping_sub(b.x) as i16);
    let dy = i32::from(a.y.wrapping_sub(b.y) as i16);
    let dz = i32::from(a.z) - i32::from(b.z);
    crate::legacy_sqrt(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)).wrapping_add(dz.wrapping_mul(dz)) as u32)
}

/// The distinct model9 and model12 branches of67CB0, after68940.
fn acquire(arena: &mut SourceArena, row: &mut EntitySnapshot,
    targets: &guided_projectile::Targets<'_>, outcome: &mut guided_projectile::StepOutcome,
) -> Result<bool, String> {
    if guided_projectile::acquire_capture(arena, row, targets)? { return Ok(true); }
    let mut best = u32::MAX;
    let mut chosen = None;
    for &slot in targets.class3 {
        let Some(actor) = area_damage::source_actor(arena, None, slot) else { continue; };
        let range = if row.model == 9 { i32::from(row.minimum_speed()).wrapping_mul(row.max_life).max(0) as u32 }
            else { u32::from(targets.owner_target_range) };
        if actor.source_id == row.source_id || actor.control_flags & 0x20 != 0 || distance(actor.position, row.position) > range { continue; }
        let value = guided_projectile::score_with_pitch(row,
            guided_projectile::centre(actor.position, actor.model, actor.shape.vertical_offset), 113);
        if value < best { best = value; chosen = Some(slot); }
    }
    if row.model == 9 {
        for &slot in targets.class5.iter().flatten() {
            let Some(actor) = area_damage::source_actor(arena, None, slot) else { continue; };
            if actor.source_id == row.source_id || !guided_projectile::class5_eligible(arena, slot, row.source_id, true) { continue; }
            let value = guided_projectile::score_with_pitch(row,
                guided_projectile::centre(actor.position, actor.model, actor.shape.vertical_offset), 512);
            if value < best { best = value; chosen = Some(slot); }
        }
    }
    if let Some(slot) = chosen {
        let actor = area_damage::source_actor(arena, None, slot).ok_or("Lightning target disappeared")?;
        let position = guided_projectile::centre(actor.position, actor.model, actor.shape.vertical_offset);
        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(row.position, position);
        put(row, 154, slot); row.set_source_roll(yaw); row.set_fov(pitch);
        if row.model == 12 && (actor.class, actor.model) == (3, 0) { outcome.music_player = Some(slot); }
    }
    Ok(chosen.is_some())
}

/// The ray's66610 step has no guided turning after its initial acquisition.
fn ray_advance(arena: &mut SourceArena, terrain: &SessionTerrain, row: &mut EntitySnapshot,
    targets: Option<&guided_projectile::Targets<'_>>, outcome: &mut guided_projectile::StepOutcome,
) -> Result<(), String> {
    let bound_target = word(row, 154);
    let bound_target_is_live = bound_target != 0 && arena.handle(bound_target).is_some();
    if !bound_target_is_live && row.control_flags & 2 == 0 {
        row.set_control_flags(row.control_flags | 2);
        if let Some(targets) = targets && acquire(arena, row, targets, outcome)? {
            row.set_yaw(row.source_roll()); row.set_pitch(row.fov);
        } else { row.set_source_roll(row.yaw); row.set_fov(row.pitch); }
    }
    let mut position = row.position;
    move_entity(&mut position, row.yaw, row.pitch, row.actual_speed);
    row.set_position(position);
    if let Some((_, position)) = guided_projectile::collision(arena, row) {
        row.set_position(position); row.disable_drawing();
    } else if terrain.ground_height(position) > position.z
        || terrain.ceiling_height(position).is_some_and(|z| position.z > z.wrapping_sub(row.array_fov())) {
        row.disable_drawing();
    } else {
        row.set_life(row.life.wrapping_sub(1));
        if row.life < 0 { row.disable_drawing(); }
    }
    Ok(())
}

fn draw(row: &mut EntitySnapshot) -> u16 {
    row.set_entity_rng(row.entity_rng.wrapping_mul(9377).wrapping_add(9439));
    row.entity_rng
}

///66750's instantaneous ray, visual segments and damage endpoint allocation.
fn ray(arena: &mut SourceArena, terrain: &mut SessionTerrain, particles: &[ParticleParameter],
    slot: u16, targets: Option<&guided_projectile::Targets<'_>>,
) -> Result<guided_projectile::StepOutcome, String> {
    let mut outcome = guided_projectile::StepOutcome::default();
    let mut row = arena.entities[usize::from(slot)].clone();
    row.set_actual_speed(row.minimum_speed());
    let start = row.position;
    crate::world::effects::sync_position(arena, terrain, slot, start, false, false)?;
    ray_advance(arena, terrain, &mut row, targets, &mut outcome)?;
    let angles = (row.yaw, row.pitch);
    let mut steps = 1;
    while row.control_flags & 0x400 == 0 {
        ray_advance(arena, terrain, &mut row, targets, &mut outcome)?;
        steps += 1;
    }
    row.set_yaw(angles.0); row.set_pitch(angles.1);
    let speed = row.actual_speed / 8;
    let mut delta = Position::default();
    move_entity(&mut delta, row.yaw, row.pitch, speed);
    let mut predicted = start;
    let mut position = start;
    let mut bend = 0_i16;
    let mut unused_bend = 0_i16;
    for remaining in (0..=steps * 8).rev() {
        // Publish before every NewEvent; it may replace this fixed address.
        arena.entities[usize::from(slot)] = row.clone();
        let generation = arena.economy.slot_generation(slot);
        if let Some(segment) = arena.allocate_source_slot_with_terrain(terrain) {
            let mut visual = super::cave_features::new_event(segment, arena.economy.new_event_rng(segment), position);
            visual.class = 9; visual.model = 9; visual.record[67] = 9; visual.record[68] = 9;
            visual.set_source_action(14); owner(&mut visual, row.source_id);
            visual.set_max_life(if segment >= slot { 0 } else { -1 }); visual.set_life(visual.max_life);
            super::cave_features::bind_particle(&mut visual, 216, particles[216])?;
            arena.entities[usize::from(segment)] = visual;
            crate::world::effects::sync_position(arena, terrain, segment, position, true, true)?;
        }
        if arena.economy.slot_generation(slot) != generation { row = arena.entities[usize::from(slot)].clone(); }
        let width = (remaining / 2).clamp(0, 8) as i16;
        for value in [&mut bend, &mut unused_bend] {
            *value = if *value > width { value.wrapping_sub(1) }
                else if *value < -width { value.wrapping_add(1) }
                else { value.wrapping_add(2 * ((draw(&mut row) % 157) / 79) as i16 - 1) };
        }
        predicted.x = predicted.x.wrapping_add(delta.x);
        predicted.y = predicted.y.wrapping_add(delta.y);
        predicted.z = predicted.z.wrapping_add(delta.z);
        position = predicted;
        position.z = position.z.wrapping_add(bend.wrapping_mul(speed / 4));
        move_entity(&mut position, row.yaw.wrapping_add(512) & 2047, 0, bend.wrapping_mul(speed / 4));
    }
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row.clone();
    let hit = guided_projectile::collision(arena, &row).map(|(slot, _)| slot);
    let intended_target = word(&row, 154);
    let Some(endpoint) = construct(arena, terrain, particles, Product::Endpoint, position)? else { return Ok(outcome); };
    if let Some(target) = hit {
        if let Some(parent) = arena.entities.get_mut(usize::from(row.source_id))
            && parent.class == 3 && matches!(parent.model, 0 | 1) { put(parent, 154, target); }
        if source_player_owner(arena, row.source_id) {
            outcome.experience = Some((row.source_id, 7, 1));
        }
    }
    // sub_68AC0 is called for side effects after endpoint allocation, even
    // when the captured ray is already hidden.
    guided_projectile::absorb(arena, terrain, particles, slot, hit, &mut outcome)?;
    let current = arena.entities[usize::from(slot)].clone();
    outcome.accuracy = guided_projectile::source_accuracy(
        arena,
        current.model,
        current.source_id,
        hit,
        intended_target,
    );
    let damage = hit.and_then(|target| arena.entities.get(usize::from(target)))
        .filter(|target| target.class == 3 && target.control_flags & 0x8010 != 0)
        .map_or(current.subspell_index, |target| {
            if target.control_flags & 0x8000 == 0 { current.subspell_index / 2 }
            else if current.mana / 4 <= target.mana { current.subspell_index / 4 }
            else { current.subspell_index }
        });
    let effect = &mut arena.entities[usize::from(endpoint)];
    owner(effect, current.source_id); effect.set_yaw(current.yaw); effect.set_pitch(current.pitch);
    put(effect, 154, hit.unwrap_or(0xae02)); effect.set_subspell_index(damage);
    outcome.impact_slot = Some(endpoint);
    Ok(outcome)
}

///66FD0's moving storm shot. Failed emitter allocation leaves it retryable.
fn storm_shot(arena: &mut SourceArena, terrain: &mut SessionTerrain, particles: &[ParticleParameter],
    slot: u16, targets: Option<&guided_projectile::Targets<'_>>,
) -> Result<guided_projectile::StepOutcome, String> {
    let mut outcome = guided_projectile::StepOutcome::default();
    let mut row = arena.entities[usize::from(slot)].clone();
    if word(&row, 154) == 0 && row.control_flags & 2 == 0 {
        row.set_control_flags(row.control_flags | 2);
        let captured = if let Some(targets) = targets { guided_projectile::acquire_capture(arena, &mut row, targets)? } else { false };
        if captured {
            row.set_yaw(row.source_roll()); row.set_pitch(row.fov);
            row.set_life(row.life.wrapping_add(32)); row.set_max_life(row.life);
            let paired = word(&row, 56);
            if paired != 0 {
                crate::world::effects::sync_position(arena, terrain, paired, arena.entities[usize::from(paired)].position, false, false)?;
                arena.economy.release_slot(paired, Some(arena.entities[usize::from(paired)].entity_rng));
                arena.entities[usize::from(paired)].class = 0;
                arena.entities[usize::from(paired)].active = false;
                put(&mut row, 56, 0);
            }
        } else if let Some(targets) = targets && acquire(arena, &mut row, targets, &mut outcome)? {
            row.set_yaw(row.source_roll()); row.set_pitch(row.fov);
        } else { row.set_source_roll(row.yaw); row.set_fov(row.pitch); }
    } else if let Some(target) = area_damage::source_actor(arena, None, word(&row, 154)) {
        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(row.position,
            guided_projectile::centre(target.position, target.model, target.shape.vertical_offset));
        row.set_source_roll(yaw); row.set_fov(pitch);
        row.set_yaw(row.yaw.wrapping_add(sub58350_delta(row.yaw, yaw, 5)) & 2047);
        row.set_pitch(row.pitch.wrapping_add(sub58350_delta(row.pitch, pitch, 5)) & 2047);
    }
    let delta = i32::from(row.minimum_speed()) - i32::from(row.actual_speed);
    row.set_actual_speed(row.actual_speed.wrapping_add((2 * delta.signum()) as i16));
    let mut position = row.position;
    move_entity(&mut position, row.yaw, row.pitch, row.actual_speed); row.set_position(position);
    arena.entities[usize::from(slot)] = row.clone();
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    let hit = guided_projectile::collision(arena, &row);
    let terminal = if let Some((_, position)) = hit { row.set_position(position); true }
        else if terrain.ground_height(position) > position.z { row.set_position(Position { z: terrain.ground_height(position), ..position }); true }
        else if let Some(z) = terrain.ceiling_height(position).map(|z| z.wrapping_sub(row.array_fov())).filter(|&z| position.z > z) { row.set_position(Position { z, ..position }); true }
        else { row.set_life(row.life.wrapping_sub(1)); row.life < 0 };
    let position = row.position;
    arena.entities[usize::from(slot)] = row.clone();
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    if !terminal { return Ok(outcome); }
    if hit.is_none() && terrain.material_mask(position) == 1 && row.life >= 0 {
        let splash = crate::world::effects::allocate(arena, terrain,
            sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Ash), position, particles)?;
        if let Some(splash) = splash { owner(&mut arena.entities[usize::from(splash)], row.source_id); }
        arena.entities[usize::from(slot)].disable_drawing(); return Ok(outcome);
    }
    if guided_projectile::absorb(arena, terrain, particles, slot, hit.map(|(slot, _)| slot), &mut outcome)? {
        arena.entities[usize::from(slot)].disable_drawing(); return Ok(outcome);
    }
    let Some(storm) = construct(arena, terrain, particles, Product::Storm, position)? else { return Ok(outcome); };
    let row = arena.entities[usize::from(slot)].clone();
    if let Some((target, _)) = hit {
        if let Some(parent) = arena.entities.get_mut(usize::from(row.source_id))
            && parent.class == 3 && matches!(parent.model, 0 | 1) { put(parent, 154, target); }
        outcome.experience = Some((row.source_id, 7, 1));
    }
    let effect = &mut arena.entities[usize::from(storm)];
    owner(effect, row.source_id); effect.set_yaw(row.yaw); effect.set_pitch(row.pitch);
    put(effect, 154, hit.map_or(0xae02, |(slot, _)| slot));
    effect.set_subspell_index(row.subspell_index); effect.record[71] = row.record[71]; effect.record[72] = row.record[72];
    arena.entities[usize::from(slot)].disable_drawing(); outcome.impact_slot = Some(storm);
    Ok(outcome)
}

///35640. Rise to ground+1024, then emit two opposite short rays per visit.
fn storm(arena: &mut SourceArena, terrain: &mut SessionTerrain, particles: &[ParticleParameter], slot: u16)
    -> Result<guided_projectile::StepOutcome, String> {
    let mut row = arena.entities[usize::from(slot)].clone();
    let mut outcome = guided_projectile::StepOutcome::default();
    let height = terrain.ground_height(row.position).wrapping_add(1024);
    let adjust = row.position.z != height;
    if row.position.z < height { row.set_position(Position { z: row.position.z.wrapping_add(64), ..row.position }); }
    if row.position.z > height { row.set_position(Position { z: height, ..row.position }); }
    if !adjust {
        let life = row.life; row.set_life(life.wrapping_sub(1));
        if life < 0 { row.disable_drawing(); }
        else {
            row.set_pitch(56); let yaw = draw(&mut row) as i16 & 2047; row.set_yaw(yaw);
            for side in 0..2 {
                row.set_yaw(row.yaw.wrapping_add(1024) & 2047);
                arena.entities[usize::from(slot)] = row.clone();
                let generation = arena.economy.slot_generation(slot);
                if let Some(ray) = construct(arena, terrain, particles, Product::Ray, row.position)? {
                    if arena.economy.slot_generation(slot) != generation { row = arena.entities[usize::from(slot)].clone(); }
                    let child = &mut arena.entities[usize::from(ray)];
                    owner(child, row.source_id); child.set_life(child.life / 3);
                    child.set_yaw(row.yaw); child.set_pitch(row.pitch);
                    child.record[71] = 10; child.record[72] = 23; child.set_subspell_index(row.subspell_index);
                    if side == 0 { outcome.sounds.push((ray, 23)); }
                }
            }
        }
    }
    arena.economy.source_rng.insert(slot, row.entity_rng);
    let position = row.position; let active = row.active;
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, active, false)?;
    Ok(outcome)
}

/// Endpoint33D80. The composition supplies the same10C80 damage consumer
/// used by other effects, so typed actors receive their real mailboxes.
pub(crate) fn step_endpoint<F>(arena: &mut SourceArena, slot: u16, mut damage: F)
    -> Result<guided_projectile::StepOutcome, String>
where F: FnMut(&mut SourceArena, crate::world::effects::DamagePulse) -> Result<u16, String> {
    let mut row = arena.entities[usize::from(slot)].clone();
    let mut outcome = guided_projectile::StepOutcome::default();
    let life = row.life; row.set_life(life.wrapping_sub(1));
    row.set_source_dword_10(row.source_dword_10().wrapping_add(1));
    if life < 0 { row.disable_drawing(); }
    else if row.control_flags & 2 == 0 {
        let hits = damage(arena, crate::world::effects::DamagePulse { position: row.position,
            owner: row.source_id, payload: row.subspell_index, half_width: row.array_pitch(),
            vertical_offset: row.array_yaw(), half_height: row.array_fov(), continuous: false,
            selector: (row.record[69], row.record[70]) })?;
        if hits != 0 && source_player_owner(arena, row.source_id) {
            outcome.experience = Some((row.source_id, 7, hits));
        }
        outcome.sounds.push((slot, 24)); row.set_life(1); row.set_control_flags(row.control_flags | 2);
    }
    arena.entities[usize::from(slot)] = row;
    Ok(outcome)
}

pub(crate) fn step(arena: &mut SourceArena, terrain: &mut SessionTerrain,
    particles: &[ParticleParameter], slot: u16, targets: Option<&guided_projectile::Targets<'_>>,
) -> Result<guided_projectile::StepOutcome, String> {
    match (arena.entities[usize::from(slot)].class, arena.entities[usize::from(slot)].action) {
        (9, 9) => ray(arena, terrain, particles, slot, targets),
        (9, 12) => storm_shot(arena, terrain, particles, slot, targets),
        (9, 14) => {
            let row = &mut arena.entities[usize::from(slot)];
            row.set_life(row.life.wrapping_sub(1));
            if row.life < -1 { row.disable_drawing(); }
            Ok(guided_projectile::StepOutcome::default())
        }
        (10, 40) => storm(arena, terrain, particles, slot),
        selector => Err(format!("Lightning callback has unowned selector{selector:?}")),
    }
}
