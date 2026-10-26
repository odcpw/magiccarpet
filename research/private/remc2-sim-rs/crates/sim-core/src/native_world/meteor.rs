//! Meteor's MC2 active holder, projectile constructor and trail epilogue.
//! The shared flyer owns sub_65820; shared effects own the real 10/17 pulse.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
use crate::campaign::source_arena::SourceArena;
use crate::level_definition::{EffectKind, METEOR_PROPERTIES as RULES};
use crate::{FlightWorld, Position, move_entity};

#[cfg(test)]
mod tests;

const PARTICLE: u16 = 76;

pub(crate) use crate::spell_activation::{
    ProjectileSpellCastInput as CastInput, ProjectileSpellHolderOutcome as HolderOutcome,
};

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

/// sub_4D500 followed by sub_6DCA0's family9 and successful sub_6AB00 launch.
pub(crate) fn launch(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    holder_slot: u16,
    parent_slot: u16,
    input: CastInput,
) -> Result<Option<u16>, String> {
    let particle = *particles
        .get(usize::from(PARTICLE))
        .ok_or("Meteor requires retained particle76")?;
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .ok_or("Meteor parent address is outside source arena")?;
    let position = parent.position;
    // sub_6DCA0 receives the pre-constructor speed by value.
    let inherited_speed = parent.actual_speed;
    let Some(slot) = arena.allocate_source_slot_with_terrain(terrain) else {
        return Ok(None);
    };
    let position = if slot == parent_slot {
        Position { x: 0, y: 0, z: 0 }
    } else {
        position
    };
    let mut row =
        super::cave_features::new_event(slot, arena.economy.new_event_rng(slot), position);
    row.class = 9;
    row.model = 3;
    row.record[67] = 9;
    row.record[68] = 3;
    row.set_source_action(3);
    row.set_control_flags((row.control_flags & !8) | 4);
    row.set_actual_speed(
        RULES
            .projectile_speed
            .wrapping_add(inherited_speed)
            .clamp(RULES.projectile_speed, RULES.maximum_launch_speed),
    );
    put(&mut row, 136, RULES.projectile_speed as u16);
    row.set_mana(50);
    row.set_max_life(RULES.projectile_range / i32::from(RULES.projectile_speed));
    row.set_life(row.max_life);
    row.parameter_relation = Some(60);
    row.record[164..168].copy_from_slice(&61_u32.to_le_bytes());
    super::cave_features::bind_particle(&mut row, PARTICLE, particle)?;
    row.record[71] = 10;
    row.record[72] = 17;
    // Publish the new fixed address before borrowing parent/holder again.
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, true)?;
    let parent = arena.entities[usize::from(parent_slot)].clone();
    let holder = arena
        .entities
        .get(usize::from(holder_slot))
        .ok_or("Meteor holder address is outside source arena")?
        .clone();
    let tier = usize::from(
        crate::spell_activation::level2_applied_tier(holder.level_gate)
            .ok_or("Meteor holder has a negative source tier")?,
    );
    let rule = RULES.tiers[tier];
    let row = &mut arena.entities[usize::from(slot)];
    // sub_68E50's ordinary left/right hand offset, before the height lift.
    let mut position = row.position;
    let hand = if parent.control_flags & 0x100 != 0 {
        Some(parent.yaw.wrapping_sub(512) & 2047)
    } else if parent.control_flags & 0x200 != 0 {
        Some(parent.yaw.wrapping_add(512) & 2047)
    } else {
        None
    };
    if let Some(yaw) = hand {
        move_entity(&mut position, yaw, 0, 256);
        if terrain.ground_height(position) > position.z {
            position = row.position;
        }
    }
    position.z = position.z.wrapping_add(parent.array_fov());
    row.set_position(position);
    put(row, 42, holder_slot);
    set_owner(row, parent.source_id);
    row.set_mana(holder.mana);
    row.set_subspell_index(rule.damage);
    row.set_source_byte_46(rule.impact_duration);
    row.set_source_dword_10(i32::from(input.auxiliary_byte));
    let yaw = input.aim_yaw.wrapping_add(parent.yaw);
    let pitch = input.aim_pitch.wrapping_add(parent.pitch);
    let mut axis = parent.position;
    move_entity(&mut axis, yaw, pitch, RULES.aim_distance);
    put(row, 158, axis.x);
    put(row, 160, axis.y);
    put(row, 162, axis.z as u16);
    row.set_yaw(yaw);
    row.set_pitch(pitch);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)?;
    Ok(Some(slot))
}

/// Source active action27. No scheduler phase advancement occurs here.
pub(crate) fn step_holder(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    input: CastInput,
) -> Result<HolderOutcome, String> {
    let mut holder = shared
        .pickup(slot)
        .cloned()
        .ok_or("Meteor holder is absent")?;
    if (holder.class, holder.model, holder.action) != (15, 9, 27) {
        return Err("Meteor active holder has a different source selector".to_owned());
    }
    // Shared spell state owns this row. Publish it before a constructor can
    // inspect or replace the fixed address, never after a replacement.
    arena.entities[usize::from(slot)] = holder.clone();
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut outcome = HolderOutcome::default();
    if holder.active_value > 0 {
        if parent_slot != 0
            && let Some(parent) = arena.entities.get(usize::from(parent_slot))
        {
            let admitted = parent.mana >= 0
                && parent.life >= 0
                && (holder.mana_regen == 0
                    || input
                        .castle_mana
                        .is_some_and(|mana| mana >= holder.mana_regen))
                && (holder.active_value != holder.activation_value as i16
                    || parent.mana >= holder.max_mana);
            if admitted {
                if holder.active_value == holder.activation_value as i16 {
                    outcome.projectile =
                        launch(arena, terrain, particles, slot, parent_slot, input)?;
                    if outcome.projectile.is_some() {
                        outcome.clear_player_aux = Some(parent_slot);
                        crate::mark_source_audio_call(
                            crate::SourceAudioCallTag::ProjectileDeliveryLaunch,
                        );
                        shared.emit_flight_effect(crate::FlightEvent::PrepareSound {
                            #[cfg(test)]
                            audio_site: crate::MissionAudioSite::MeteorLaunch,
                            entity_index: parent_slot,
                            player_index: -1,
                            sound_id: 15,
                        });
                    }
                }
                // sub_68DE0 runs even when NewEvent fails, and on each pulse.
                let parent = &mut arena.entities[usize::from(parent_slot)];
                let regen = if holder.active_value == holder.activation_value as i16 {
                    if parent.mana_regen >= 0 {
                        holder.max_mana.wrapping_neg()
                    } else {
                        parent.mana_regen.wrapping_sub(holder.max_mana)
                    }
                } else {
                    parent.mana_regen.min(0)
                };
                parent.set_mana_regen(regen);
                shared.sync_player_entity_from_authority(parent);
                if parent_slot == shared.player_entity.index {
                    shared.sync_typed_spell_parent_caches_from_player_entity();
                }
            } else {
                holder.active_value = 1;
            }
        }
        holder.active_value = holder.active_value.wrapping_sub(1);
        if holder.active_value == 0 {
            holder.record[50..52].copy_from_slice(&0_i16.to_le_bytes());
            let deferred = holder.source_word_2c();
            if deferred != 0 {
                let tier =
                    crate::spell_activation::level2_applied_tier(deferred.wrapping_sub(1) as i8)
                        .ok_or("Meteor deferred tier is negative")?;
                let profile = crate::spell_activation::level2_set_spell_profile(9, tier)
                    .ok_or("Meteor deferred tier has no source profile")?;
                holder.apply_nyphur_spell_profile(profile, tier);
                holder.set_source_word_2c(0);
            }
        }
    }
    let cooldown = word(&holder, 58) as i16;
    if cooldown != 0 {
        put(&mut holder, 58, cooldown.wrapping_sub(1) as u16);
    }
    holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
    if arena.entities[usize::from(slot)].class == 15 && arena.entities[usize::from(slot)].model == 9
    {
        if let Some(current) = shared.pickup_mut(slot) {
            *current = holder.clone();
        }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(outcome)
}

/// sub_66180 after the canonical 65820 movement/collision/impact operation.
/// The caller supplies its actual returned impact address (not a second hit).
pub(crate) fn after_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    impact_slot: Option<u16>,
) -> Result<(), String> {
    if arena.entities[usize::from(slot)].class == 0 {
        return Ok(());
    }
    let row = &mut arena.entities[usize::from(slot)];
    row.set_entity_rng(row.entity_rng.wrapping_mul(9377).wrapping_add(9439));
    let x = row
        .position
        .x
        .wrapping_add(row.entity_rng % 129)
        .wrapping_sub(160);
    row.set_entity_rng(row.entity_rng.wrapping_mul(9377).wrapping_add(9439));
    let position = Position {
        x,
        y: row
            .position
            .y
            .wrapping_add(row.entity_rng % 129)
            .wrapping_sub(160),
        z: row.position.z,
    };
    arena.economy.source_rng.insert(slot, row.entity_rng);
    let trail = crate::world::effects::allocate(
        arena,
        terrain,
        sim_definition::adapter::mc2_effect_definition(EffectKind::Impact),
        position,
        particles,
    )?;
    if let Some(trail) = trail {
        let parent = arena.entities[usize::from(slot)].clone();
        let row = &mut arena.entities[usize::from(trail)];
        row.set_control_flags(row.control_flags | 0x10080);
        set_owner(row, parent.source_id);
        row.set_life(4);
        row.record[96] = 3;
        row.set_yaw(parent.yaw);
    }
    if let Some(impact_slot) = impact_slot {
        let duration = arena.entities[usize::from(slot)].record[74];
        if let Some(effect) = arena
            .economy
            .authored_model17
            .iter_mut()
            .find(|row| row.active && row.source_entity_index == impact_slot)
        {
            effect.max_life = i16::from(duration);
            effect.life = i16::from(duration);
        }
        // Keep the lossless header projection current before any later-slot
        // consumer, while the typed effect remains its authoritative carrier.
        let impact = &mut arena.entities[usize::from(impact_slot)];
        impact.set_max_life(i32::from(duration));
        impact.set_life(i32::from(duration));
    }
    Ok(())
}

/// The original action3 wrapper: shared flyer first, then its source-local
/// trail/impact-duration epilogue, before the scheduler's common phase tail.
pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
) -> Result<super::guided_projectile::StepOutcome, String> {
    let outcome = super::guided_projectile::step(
        arena,
        terrain,
        particles,
        slot,
        super::guided_projectile::Guidance {
            yaw_turn: RULES.projectile_yaw_turn,
            pitch_turn: RULES.projectile_pitch_turn,
        },
        targets,
    )?;
    after_projectile(arena, terrain, particles, slot, outcome.impact_slot)?;
    Ok(outcome)
}
