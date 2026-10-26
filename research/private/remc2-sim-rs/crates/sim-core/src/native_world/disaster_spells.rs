//! MC2 terrain spell holder adapter. Source records remain authoritative;
//! family flyers deliver into the existing scheduled terrain kernels.

use super::{EntitySnapshot, Level2SharedSpellResetSession, ParticleParameter, SessionTerrain};
use crate::{
    campaign::source_arena::SourceArena,
    spells::disasters::{DisasterCaster, DisasterHolder, DisasterSpell},
};

pub(crate) use crate::spell_activation::{
    ProjectileSpellCastInput as CastInput, ProjectileSpellHolderOutcome as HolderOutcome,
};

pub(crate) mod crater;
pub(crate) mod earthquake;
pub(crate) mod tremor;
pub(crate) mod volcano;

fn spell(family: u8) -> Result<DisasterSpell, String> {
    match family {
        15 => Ok(DisasterSpell::Tremor),
        16 => Ok(DisasterSpell::Crater),
        17 => Ok(DisasterSpell::Earthquake),
        18 => Ok(DisasterSpell::Volcano),
        _ => Err(format!("terrain spell family {family} is unsupported")),
    }
}

pub(crate) fn construct_effect(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    projectile: &EntitySnapshot,
) -> Result<Option<u16>, String> {
    match (projectile.record[71], projectile.record[72]) {
        (10, 71) => tremor::construct_effect(arena, terrain, projectile),
        (10, 11) => crater::construct_effect(arena, terrain, projectile),
        (10, 15) => earthquake::construct_effect(arena, terrain, projectile),
        (10, 9) => volcano::construct_effect(arena, terrain, projectile),
        selector => Err(format!("unsupported terrain spell child {selector:?}")),
    }
}

pub(crate) fn step_projectile(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    targets: Option<&super::guided_projectile::Targets<'_>>,
) -> Result<super::guided_projectile::StepOutcome, String> {
    match arena
        .entities
        .get(usize::from(slot))
        .map(|row| (row.class, row.model, row.action))
    {
        Some((9, 23, 24)) => tremor::step_projectile(arena, terrain, particles, slot, targets),
        Some((9, 5, 5)) => crater::step_projectile(arena, terrain, particles, slot, targets),
        Some((9, 2, 2)) => earthquake::step_projectile(arena, terrain, particles, slot, targets),
        Some((9, 4, 4)) => volcano::step_projectile(arena, terrain, particles, slot, targets),
        selector => Err(format!("unsupported terrain spell flyer {selector:?}")),
    }
}

/// No common scheduler phase advancement occurs here. The caller publishes
/// the holder and parent caches before the ordinary source epilogue.
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
        .ok_or("terrain holder absent")?;
    let family = holder.model;
    let kind = spell(family)?;
    if holder.class != 15 || holder.action != family * 3 {
        return Err("terrain holder has an unrelated source callback".into());
    }
    arena.entities[usize::from(slot)] = holder.clone();
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut caster = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|_| parent_slot != 0)
        .map(|parent| DisasterCaster {
            life: parent.life,
            mana: parent.mana,
            mana_regen: parent.mana_regen,
            castle_mana: input.castle_mana,
        });
    let mut state = DisasterHolder {
        active: holder.active_value,
        duration: holder.activation_value as i16,
        cooldown: u16::from_le_bytes([holder.record[58], holder.record[59]]),
        mana_cost: holder.max_mana,
        castle_mana_required: holder.mana_regen,
    };
    let visit = state.step(kind, caster.as_mut());
    let mut outcome = HolderOutcome::default();
    if visit.launch {
        outcome.projectile = match kind {
            DisasterSpell::Tremor => {
                tremor::launch(arena, terrain, particles, slot, parent_slot, input)?
            }
            DisasterSpell::Crater => {
                crater::launch(arena, terrain, particles, slot, parent_slot, input)?
            }
            DisasterSpell::Earthquake => {
                earthquake::launch(arena, terrain, particles, slot, parent_slot, input)?
            }
            DisasterSpell::Volcano => {
                volcano::launch(arena, terrain, particles, slot, parent_slot, input)?
            }
        };
        if outcome.projectile.is_some() {
            outcome.clear_player_aux = Some(parent_slot);
            crate::mark_source_audio_call(crate::SourceAudioCallTag::ProjectileDeliveryLaunch);
            shared.emit_flight_effect(crate::FlightEvent::PrepareSound {
                #[cfg(test)]
                audio_site: crate::MissionAudioSite::MeteorLaunch,
                entity_index: parent_slot,
                player_index: -1,
                sound_id: 15,
            });
        }
    }
    if let Some(caster) = caster {
        // Reborrow after allocation: NewEvent may have replaced this address.
        let parent = &mut arena.entities[usize::from(parent_slot)];
        let regen = if visit.launch {
            if parent.mana_regen >= 0 {
                holder.max_mana.wrapping_neg()
            } else {
                parent.mana_regen.wrapping_sub(holder.max_mana)
            }
        } else {
            caster.mana_regen
        };
        parent.set_mana_regen(regen);
        shared.sync_player_entity_from_authority(parent);
        if parent_slot == shared.player_entity.index {
            shared.sync_typed_spell_parent_caches_from_player_entity();
        }
    }
    holder.active_value = state.active;
    holder.record[50..52].copy_from_slice(&state.active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&state.cooldown.to_le_bytes());
    if visit.cleanup && holder.source_word_2c() != 0 {
        let tier = crate::spell_activation::level2_applied_tier(
            holder.source_word_2c().wrapping_sub(1) as i8,
        )
        .ok_or("terrain holder deferred tier is negative")?;
        let profile = crate::spell_activation::level2_set_spell_profile(family, tier)
            .ok_or("terrain holder deferred tier is absent")?;
        holder.apply_nyphur_spell_profile(profile, tier);
        holder.set_source_word_2c(0);
    }
    let current = &arena.entities[usize::from(slot)];
    if current.class == 15 && current.model == family {
        if let Some(target) = shared.pickup_mut(slot) {
            *target = holder.clone();
        }
        arena.entities[usize::from(slot)] = holder;
    }
    Ok(outcome)
}
