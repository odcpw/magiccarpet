//! Lossless MC2 holder adapter for the support spell behavior owner.

use super::{EntitySnapshot, Level2SharedSpellResetSession, RenderableMapSeed, SessionTerrain};
use crate::campaign::{source_arena::SourceArena, state::CanonicalRuntimeState};
use crate::spell_activation::{
    BeyondSightState, InvisibleSpecialWrite, InvisibleState, ShieldState, level2_applied_tier,
    level2_set_spell_profile, support_spell_rule,
};
use crate::spells::support::{
    SupportSpellHolder, SupportSpellPlayer, SupportSpellSound, SupportSpellStep, TeleportCastles,
};
use crate::{FlightWorld, Position};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReboundReflection {
    pub target_slot: u16,
    pub mana_after: i32,
}

/// The shared detached holder callback, source sub_692C0/sub_59DC0.
/// Parent and thief are current fixed-address views, read after the producer
/// has committed its source mutations. The ordinary scheduler owns phase.
pub(crate) fn step_detached_holder<W: FlightWorld>(
    holder: &mut EntitySnapshot,
    parent: &EntitySnapshot,
    thief: &EntitySnapshot,
    terrain: &W,
) -> Result<(), String> {
    if holder.class != 15 || holder.model >= 26 || holder.action != 78 {
        return Err("detached spell callback requires class15/action78".to_owned());
    }
    let mut complete = parent.life < 0 || parent.control_flags & 0x400 != 0;
    if !complete {
        let counter = holder.source_dword_10();
        if counter <= 5 {
            let mut position = parent.position;
            holder.set_yaw(parent.yaw);
            holder.set_pitch(parent.pitch);
            crate::move_entity(
                &mut position,
                parent.yaw,
                parent.pitch.wrapping_sub((counter as i16).wrapping_mul(16)),
                384,
            );
            holder.set_position(position);
        } else if thief.life < 0 || thief.control_flags & 0x400 != 0 {
            complete = true;
        } else {
            let mut target = thief.position;
            crate::move_entity(&mut target, thief.yaw, 0, 384);
            let (yaw, pitch) =
                crate::level1_mission::projectile_guidance_angles(holder.position, target);
            let mut position = holder.position;
            let speed = counter.wrapping_sub(5).wrapping_mul(32) as i16;
            crate::move_entity(&mut position, yaw, pitch, speed);
            let ground = terrain.ground_height(position);
            if i32::from(position.z) < i32::from(ground) + 64 {
                position.z = ground;
                complete = true;
            }
            holder.set_position(position);
        }
        holder.set_source_dword_10(counter.wrapping_add(1));
    }
    if complete {
        holder.set_source_action(holder.model * 3 + 1);
        holder.record[42..44].copy_from_slice(&0_u16.to_le_bytes());
        holder.set_control_flags(holder.control_flags | 0x0200_0000);
    }
    Ok(())
}

/// Commit the detached callback against the same holder and map carriers as
/// ordinary pickup/acquisition. No parallel spell state is retained here.
pub(crate) fn step_detached_source_slot(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    source_slot: u16,
) -> Result<(), String> {
    let mut holder = shared
        .pickup(source_slot)
        .filter(|row| row.active && row.class == 15 && row.action == 78)
        .cloned()
        .ok_or_else(|| format!("detached holder {source_slot} is not retained"))?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let thief_slot = u16::from_le_bytes([holder.record[42], holder.record[43]]);
    let parent = arena
        .entities
        .get(usize::from(parent_slot))
        .ok_or("detached holder parent address is outside arena")?;
    let thief = arena
        .entities
        .get(usize::from(thief_slot))
        .ok_or("detached holder thief address is outside arena")?;
    step_detached_holder(&mut holder, parent, thief, terrain)?;
    shared.sync_pickup_from_authority(&holder);
    if let Some(raw) = arena
        .entities
        .get_mut(usize::from(source_slot))
        .filter(|row| row.active && row.class == 15)
    {
        *raw = holder.clone();
    }
    crate::world::effects::sync_position(arena, terrain, source_slot, holder.position, true, false)
}

pub(super) fn commit_reflection(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    core: &mut CanonicalRuntimeState,
    reflection: ReboundReflection,
) {
    let Some(target) = arena.entities.get_mut(usize::from(reflection.target_slot)) else {
        return;
    };
    target.set_mana(reflection.mana_after);
    let target = target.clone();
    shared.sync_player_entity_from_authority(&target);
    if reflection.target_slot == shared.player_entity.index {
        shared.player_mana = reflection.mana_after;
        shared.caster.mana = reflection.mana_after;
        shared.sync_typed_spell_parent_caches_from_player_entity();
    }
    // sub_6D8B0 awards experience only to a source model-0 wizard.
    if target.class == 3
        && target.model == 0
        && shared
            .players
            .iter()
            .any(|player| player.entity_index == target.index)
    {
        shared.award_spell_experience(target.index, 8, 1);
        for player in core
            .players
            .iter_mut()
            .filter(|player| player.entity_index == target.index)
        {
            if let Some(source) = shared
                .players
                .iter()
                .find(|player| player.entity_index == target.index)
            {
                player.sync_spell_progression_family_from(source, 8);
            }
        }
    }
    if let Some(fire) = core.context.spell_fire.as_mut()
        && fire.caster.entity_index == target.index
    {
        fire.caster.mana = reflection.mana_after;
        fire.spell_experience[8] = shared.spell_experience[8];
        fire.spell_levels[8] = shared.spell_levels[8];
    }
    crate::mark_source_audio_call(crate::SourceAudioCallTag::ReboundReflection);
    shared.emit_flight_effect(crate::FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: crate::MissionAudioSite::ReboundReflection,
        entity_index: target.index,
        player_index: -1,
        sound_id: 28,
    });
}

/// Opcode 31/32's CopyAxisForSpellWithLife_6D830 is independent of whether
/// SetSpell applies immediately or queues a profile for the expiry boundary.
pub(super) fn capture_return_position(holder: &mut EntitySnapshot, tier: i8, position: Position) {
    if holder.model == 10 && tier == 1 {
        holder.record[158..160].copy_from_slice(&position.x.to_le_bytes());
        holder.record[160..162].copy_from_slice(&position.y.to_le_bytes());
        holder.record[162..164].copy_from_slice(&position.z.to_le_bytes());
    }
}

pub(super) fn sound_event(sound: SupportSpellSound, entity_index: u16) -> crate::FlightEvent {
    crate::mark_source_audio_call(match sound {
        SupportSpellSound::Heal => crate::SourceAudioCallTag::HealSpell,
        SupportSpellSound::Teleport => crate::SourceAudioCallTag::TeleportSpell,
    });
    crate::FlightEvent::PrepareSound {
        #[cfg(test)]
        audio_site: match sound {
            SupportSpellSound::Heal => crate::MissionAudioSite::HealSpell,
            SupportSpellSound::Teleport => crate::MissionAudioSite::TeleportSpell,
        },
        entity_index,
        player_index: -1,
        sound_id: match sound {
            SupportSpellSound::Heal => 25,
            SupportSpellSound::Teleport => 22,
        },
    }
}

pub(super) fn step_holder(
    holder: &mut EntitySnapshot,
    parent: Option<&mut EntitySnapshot>,
    castle_mana: Option<i32>,
    player_color: u8,
    castles: &TeleportCastles,
) -> Option<SupportSpellStep> {
    let tier = u8::try_from(holder.level_gate).ok()?;
    let rule = support_spell_rule(holder.model, tier, holder.subspell_index)?;
    let mut state = SupportSpellHolder {
        active: holder.active_value,
        duration: holder.activation_value.cast_signed(),
        presentation_timer: u16::from_le_bytes([holder.record[58], holder.record[59]]),
        mana_cost: holder.max_mana,
        castle_mana_required: holder.mana_regen,
        teleport_selector: holder.source_word_96(),
        return_position: Position {
            x: u16::from_le_bytes([holder.record[158], holder.record[159]]),
            y: u16::from_le_bytes([holder.record[160], holder.record[161]]),
            z: i16::from_le_bytes([holder.record[162], holder.record[163]]),
        },
        rng: holder.entity_rng,
    };
    let mut player = parent.as_deref().map(|parent| SupportSpellPlayer {
        life: parent.life,
        max_life: parent.max_life,
        mana: parent.mana,
        mana_regen: parent.mana_regen,
        flags: parent.control_flags,
        position: parent.position,
        yaw: parent.yaw,
        castle_mana,
        color: player_color,
    });
    let result = state.step(rule, player.as_mut(), castles);
    if let (Some(parent), Some(player)) = (parent, player) {
        parent.set_life(player.life);
        parent.set_mana_regen(player.mana_regen);
        parent.set_control_flags(player.flags);
        parent.set_position(player.position);
    }
    holder.active_value = state.active;
    holder.record[50..52].copy_from_slice(&state.active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&state.presentation_timer.to_le_bytes());
    holder.set_source_word_96(state.teleport_selector);
    holder.record[158..160].copy_from_slice(&state.return_position.x.to_le_bytes());
    holder.record[160..162].copy_from_slice(&state.return_position.y.to_le_bytes());
    holder.record[162..164].copy_from_slice(&state.return_position.z.to_le_bytes());
    holder.set_entity_rng(state.rng);
    if result.cleanup {
        let queued = holder.source_word_2c();
        if let Some(tier) = queued
            .checked_sub(1)
            .and_then(|tier| u8::try_from(tier).ok())
            && let Some(profile) = level2_set_spell_profile(holder.model, tier)
        {
            holder.apply_nyphur_spell_profile(profile, tier);
            holder.set_source_word_2c(0);
        }
    }
    Some(result)
}

/// These kernels borrow their pulse state from the current holder/player
/// records. A rival never selects or overwrites the local player's lifecycle.
enum StatusProjection {
    Shield(ShieldState),
    Invisible(InvisibleState),
    BeyondSight(BeyondSightState),
}

fn step_status_holder(
    holder: &mut EntitySnapshot,
    parent: Option<&mut EntitySnapshot>,
    castle_mana: Option<i32>,
    beyond_sight_experience: i32,
) -> Result<
    (
        SupportSpellStep,
        StatusProjection,
        Option<InvisibleSpecialWrite>,
    ),
    String,
> {
    let tier = level2_applied_tier(holder.level_gate)
        .ok_or_else(|| format!("status holder {} has a negative tier", holder.index))?;
    let parent_slot = parent.as_ref().map_or(0, |row| row.index);
    let life = parent.as_ref().map_or(0, |row| row.life);
    let mana = parent.as_ref().map_or(0, |row| row.mana);
    let regen = parent.as_ref().map_or(0, |row| row.mana_regen);
    let flags = parent.as_ref().map_or(0, |row| row.control_flags);
    macro_rules! hydrate {
        ($state:expr) => {{
            let mut state = $state.ok_or("status holder has an invalid source tier")?;
            state.action = holder.action;
            state.active_value = holder.active_value;
            state.activation_value = holder.activation_value.cast_signed();
            state.mana_cost = holder.max_mana;
            state.parent_mana_regen = regen;
            state.countdown = i16::from_le_bytes([holder.record[58], holder.record[59]]);
            state
        }};
    }
    let (projection, active, countdown, next_regen, next_flags, cleanup, award, special) =
        match holder.model {
            6 => {
                let mut state = hydrate!(ShieldState::new(
                    tier,
                    holder.index,
                    parent_slot,
                    mana,
                    life
                ));
                state.parent_status_flags = flags;
                let step = state.step();
                (
                    StatusProjection::Shield(state),
                    state.active_value,
                    state.countdown,
                    state.parent_mana_regen,
                    state.parent_status_flags,
                    step.cleanup_requested,
                    0,
                    None,
                )
            }
            11 => {
                let mut state = hydrate!(InvisibleState::new(
                    tier,
                    holder.index,
                    parent_slot,
                    mana,
                    life
                ));
                state.mana_regen_requirement = holder.mana_regen;
                state.parent_status_flags = flags;
                state.spell_status_flags = holder.control_flags;
                let step = state.step(castle_mana);
                holder.set_control_flags(state.spell_status_flags);
                (
                    StatusProjection::Invisible(state),
                    state.active_value,
                    state.countdown,
                    state.parent_mana_regen,
                    state.parent_status_flags,
                    step.cleanup_requested,
                    step.experience_awarded,
                    step.special_state_write,
                )
            }
            12 => {
                let mut state = hydrate!(BeyondSightState::new(
                    tier,
                    holder.index,
                    parent_slot,
                    mana,
                    life
                ));
                state.mana_regen_requirement = holder.mana_regen;
                state.experience_awarded = beyond_sight_experience;
                let step = state.step_after_experience_award(castle_mana, beyond_sight_experience);
                (
                    StatusProjection::BeyondSight(state),
                    state.active_value,
                    state.countdown,
                    state.parent_mana_regen,
                    flags,
                    step.cleanup_requested,
                    0,
                    None,
                )
            }
            _ => return Err("status callback received a different spell family".to_owned()),
        };
    holder.active_value = active;
    holder.record[50..52].copy_from_slice(&active.to_le_bytes());
    holder.record[58..60].copy_from_slice(&countdown.to_le_bytes());
    if let Some(parent) = parent {
        parent.set_mana_regen(next_regen);
        parent.set_control_flags(next_flags);
    }
    if cleanup {
        let queued = holder.source_word_2c();
        if let Some(tier) = queued
            .checked_sub(1)
            .and_then(|tier| u8::try_from(tier).ok())
            && let Some(profile) = level2_set_spell_profile(holder.model, tier)
        {
            holder.apply_nyphur_spell_profile(profile, tier);
            holder.set_source_word_2c(0);
        }
    }
    Ok((
        SupportSpellStep {
            award_experience: award != 0,
            cleanup,
            ..SupportSpellStep::default()
        },
        projection,
        special,
    ))
}

/// Compose the authoritative holder/player write with existing numeric arena,
/// flight and presentation consumers. Spell decisions remain in support::step.
pub(super) fn step_source_slot(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    core: &mut CanonicalRuntimeState,
    source_slot: u16,
) -> Result<Option<(u16, bool)>, String> {
    let mut holder = shared
        .pickup(source_slot)
        .filter(|holder| {
            holder.active
                && holder.class == 15
                && matches!(holder.model, 5 | 6 | 8 | 10 | 11 | 12)
                && holder.action == holder.model * 3
        })
        .cloned()
        .ok_or_else(|| format!("support spell holder {source_slot} is not live"))?;
    let parent_slot = holder.source_spell_parent_entity_index();
    let mut parent = arena
        .entities
        .get(usize::from(parent_slot))
        .filter(|parent| parent_slot != 0 && parent.active && parent.class == 3)
        .cloned();
    // The accepted Shield hit mailbox is consumed at this holder's visit.
    // Refresh its parent cache from the current arena first, and carry the
    // damage result into the same raw parent that the pulse will publish.
    if holder.model == 6
        && let Some(actor) = parent.as_ref()
        && actor.index == shared.player_entity.index
        && shared
            .shield_state()
            .is_some_and(|state| state.holder_entity_index == source_slot)
    {
        shared.sync_player_entity_from_authority(actor);
        shared.sync_typed_spell_parent_caches_from_player_entity();
        if shared.consume_shield_damage_mailbox(source_slot).is_some() {
            parent = Some(shared.player_entity.clone());
        }
    }
    let parent_feedback = parent.as_ref().map(|_| parent_slot);
    let player_color = shared
        .players
        .iter()
        .find(|player| player.entity_index == parent_slot)
        .map_or(0, |player| player.player_color_index());
    let mut castles = TeleportCastles {
        player_count: core.runtime.player_count,
        ..TeleportCastles::default()
    };
    let mut castle_mana = None;
    for player in shared
        .players
        .iter()
        .take(usize::from(core.runtime.player_count))
    {
        let color = usize::from(player.player_color_index());
        if color >= castles.positions.len() {
            continue;
        }
        let castle = arena
            .economy
            .castle_for_owner(player.entity_index)
            .map(|castle| (castle.position, castle.mana));
        castles.positions[color] = castle.map(|(position, _)| position);
        if player.entity_index == parent_slot {
            castle_mana = castle.map(|(_, mana)| mana);
        }
    }
    // Source6B310 publishes XP before 68D50. Refresh the actual holder
    // afterwards so a threshold's queued SetSpell survives this pulse.
    let can_award_experience = shared.mission_rewards().allows_experience()
        && parent
            .as_ref()
            .is_some_and(|row| (row.class, row.model) == (3, 0));
    let beyond_sight_experience = i32::from(
        can_award_experience
            && holder.model == 12
            && holder.active_value > 0
            && holder.active_value == holder.activation_value.cast_signed(),
    );
    if beyond_sight_experience != 0 {
        shared.award_spell_experience(parent_slot, 12, beyond_sight_experience);
        shared.publish_spell_progression_family(
            &mut core.players,
            core.context.spell_fire.as_mut(),
            parent_slot,
            12,
        );
        holder = shared
            .pickup(source_slot)
            .cloned()
            .ok_or("Beyond Sight award lost its holder")?;
    }
    let (result, status) = if matches!(holder.model, 6 | 11 | 12) {
        let (step, state, special) = step_status_holder(
            &mut holder,
            parent.as_mut(),
            castle_mana,
            beyond_sight_experience,
        )?;
        if let Some(write) = special {
            if write.clear_protection {
                shared.global_special_state.protection_ticks = 0;
            }
            shared.global_special_state.visibility_mode = write.visibility_mode;
        }
        (step, Some(state))
    } else {
        (
            step_holder(
                &mut holder,
                parent.as_mut(),
                castle_mana,
                player_color,
                &castles,
            )
            .ok_or_else(|| format!("support spell holder {source_slot} has invalid family/tier"))?,
            None,
        )
    };
    if parent_slot == core.state.entity_index {
        macro_rules! current_profile {
            ($state:ident) => {
                $state.tier = level2_applied_tier(holder.level_gate)
                    .ok_or("status cleanup left a negative tier")?;
                $state.active_value = holder.active_value;
                $state.activation_value = holder.activation_value.cast_signed();
                $state.mana_cost = holder.max_mana;
                $state.cleanup_requested = false;
            };
        }
        match status {
            Some(StatusProjection::Shield(mut state)) => {
                current_profile!(state);
                state.experience_awarded = shared
                    .shield
                    .filter(|old| old.holder_entity_index == source_slot)
                    .map_or(0, |old| old.experience_awarded);
                shared.shield = Some(state);
            }
            Some(StatusProjection::Invisible(mut state)) => {
                current_profile!(state);
                state.mana_regen_requirement = holder.mana_regen;
                if !can_award_experience {
                    state.experience_awarded = 0;
                }
                state.experience_awarded = state.experience_awarded.wrapping_add(
                    shared
                        .invisible
                        .filter(|old| old.holder_entity_index == source_slot)
                        .map_or(0, |old| old.experience_awarded),
                );
                shared.invisible = Some(state);
            }
            Some(StatusProjection::BeyondSight(mut state)) => {
                current_profile!(state);
                state.mana_regen_requirement = holder.mana_regen;
                state.experience_awarded = state.experience_awarded.wrapping_add(
                    shared
                        .beyond_sight
                        .filter(|old| old.holder_entity_index == source_slot)
                        .map_or(0, |old| old.experience_awarded),
                );
                shared.beyond_sight = Some(state);
            }
            None => {}
        }
    }
    if let Some(parent) = parent {
        shared.sync_player_entity_from_authority(&parent);
        arena.entities[usize::from(parent_slot)] = parent.clone();
        if parent_slot == core.state.entity_index {
            core.state.position = parent.position;
            if result.stop_player {
                core.state.target_speed = 0;
            }
            shared.caster.control_flags = parent.control_flags;
            shared.caster.mana = parent.mana;
            shared.player_mana = parent.mana;
            shared.sync_typed_spell_parent_caches_from_player_entity();
        }
        arena.map_chains.sync_entity(RenderableMapSeed {
            entity_index: parent_slot,
            active: parent.active,
            x: parent.position.x,
            y: parent.position.y,
        })?;
        if result.stop_player {
            for player in shared
                .players
                .iter_mut()
                .filter(|player| player.entity_index == parent_slot)
            {
                player.record[20..22].fill(0);
            }
            if shared.local_player.entity_index == parent_slot {
                shared.local_player.record[20..22].fill(0);
            }
        }
        if result.award_experience && can_award_experience {
            // XP can queue SetSpell on this same live holder. Publish the
            // callback first, then retain those source-visible queue writes.
            shared.sync_pickup_from_authority(&holder);
            shared.award_spell_experience(parent_slot, holder.model, 1);
            holder = shared
                .pickup(source_slot)
                .cloned()
                .ok_or("support spell award lost its holder")?;
        }
        for player in core
            .players
            .iter_mut()
            .filter(|player| player.entity_index == parent_slot)
        {
            if let Some(source) = shared
                .players
                .iter()
                .find(|player| player.entity_index == parent_slot)
            {
                player.sync_spell_progression_family_from(source, usize::from(holder.model));
            }
            if result.stop_player {
                player.record[20..22].fill(0);
            }
        }
        if let Some(fire) = core.context.spell_fire.as_mut() {
            if fire.caster.entity_index == parent_slot {
                fire.caster.control_flags = parent.control_flags;
                fire.caster.mana = parent.mana;
                fire.spell_experience[usize::from(holder.model)] =
                    shared.spell_experience[usize::from(holder.model)];
                fire.spell_levels[usize::from(holder.model)] =
                    shared.spell_levels[usize::from(holder.model)];
            }
        }
        if let Some(sound) = result.sound {
            shared.emit_flight_effect(sound_event(sound, parent_slot));
        }
    }
    if let Some(target) = shared.pickup_mut(source_slot) {
        *target = holder.clone();
    }
    // Imported rows and dynamic NewEvent rows share this lossless arena row;
    // later visits, cleanup and replay hash the same bytes.
    if let Some(target) = arena.entities.get_mut(usize::from(source_slot)) {
        *target = holder.clone();
    }
    if let Some(fire) = core.context.spell_fire.as_mut()
        && let Some(target) = fire
            .holders
            .iter_mut()
            .find(|holder| holder.entity_index == source_slot)
    {
        target.active_value = holder.active_value;
        target.activation_value = holder.activation_value;
        target.level_gate = holder.level_gate;
        target.mana_cost = holder.mana_cost;
    }
    // Dynamic holders have a typed source cursor in addition to their raw
    // record. Advance both once; the scheduler verifies the common epilogue.
    let dynamic_kind = arena
        .economy
        .live_arena_cursor_rows()
        .into_iter()
        .find(|row| row.source_slot == source_slot)
        .map(|row| row.kind);
    let advanced = match dynamic_kind {
        Some(super::Level2DynamicArenaKind::SpellPickup) => {
            arena.economy.step_spell_pickup_slot(source_slot)
        }
        Some(super::Level2DynamicArenaKind::SpellReplacement) => {
            arena.economy.step_spell_replacement_slot(source_slot)
        }
        _ => false,
    };
    if advanced {
        if let Some(holder) = shared.pickup_mut(source_slot) {
            holder.phase = holder.phase.wrapping_add(1);
            holder.record[66] = holder.phase;
            arena.entities[usize::from(source_slot)].clone_from(holder);
        }
    }
    Ok(parent_feedback.map(|owner| (owner, result.stop_player)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerCommand;
    use crate::level1_mission::parse_authored_features;
    use crate::native_world::{Mc2DispatchEvent, NativeWorld};
    use std::path::PathBuf;

    struct DetachedGround;

    impl FlightWorld for DetachedGround {
        fn ground_height(&self, _: Position) -> i16 {
            0
        }
        fn ceiling_height(&self, _: Position) -> Option<i16> {
            None
        }
        fn material_mask(&self, _: Position) -> u32 {
            1
        }
        fn angle_flags(&self, _: crate::Tile) -> u8 {
            0
        }
    }

    fn detached_holder() -> EntitySnapshot {
        let mut row = crate::native_world::cave_features::new_event(
            3,
            7,
            Position {
                x: 4096,
                y: 4096,
                z: 1000,
            },
        );
        row.class = 15;
        row.model = 8;
        row.set_source_action(78);
        row.record[42..44].copy_from_slice(&2_u16.to_le_bytes());
        row
    }

    #[test]
    fn detached_holder_orbits_parent_before_chasing_thief() {
        let mut holder = detached_holder();
        let mut parent = holder.clone();
        parent.set_yaw(512);
        parent.set_pitch(64);
        let thief = parent.clone();
        for counter in 0..=5 {
            let mut expected = parent.position;
            crate::move_entity(&mut expected, parent.yaw, parent.pitch - counter * 16, 384);
            step_detached_holder(&mut holder, &parent, &thief, &DetachedGround).unwrap();
            assert_eq!(holder.position, expected);
            assert_eq!(holder.source_dword_10(), i32::from(counter + 1));
            assert_eq!((holder.yaw, holder.pitch, holder.action), (512, 64, 78));
        }
        let before = holder.position;
        let mut target = thief.position;
        crate::move_entity(&mut target, thief.yaw, 0, 384);
        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(before, target);
        let mut expected = before;
        crate::move_entity(&mut expected, yaw, pitch, 32);
        step_detached_holder(&mut holder, &parent, &thief, &DetachedGround).unwrap();
        assert_eq!(holder.position, expected);
        assert_eq!(holder.source_dword_10(), 7);
        assert_eq!(holder.action, 78);
    }

    #[test]
    fn detached_holder_returns_on_parent_thief_or_ground_boundary() {
        let parent = detached_holder();
        let mut thief = parent.clone();
        thief.set_life(-1);
        let mut holder = detached_holder();
        holder.set_source_dword_10(6);
        step_detached_holder(&mut holder, &parent, &thief, &DetachedGround).unwrap();
        assert_eq!(holder.source_dword_10(), 7);
        assert_eq!(holder.action, 25);
        assert_eq!(&holder.record[42..44], &[0, 0]);
        assert_ne!(holder.control_flags & 0x0200_0000, 0);

        let mut dead_parent = parent.clone();
        dead_parent.set_life(-1);
        let mut holder = detached_holder();
        holder.set_source_dword_10(6);
        step_detached_holder(&mut holder, &dead_parent, &parent, &DetachedGround).unwrap();
        assert_eq!(holder.source_dword_10(), 6);
        assert_eq!(holder.action, 25);

        let mut holder = detached_holder();
        holder.set_source_dword_10(6);
        holder.set_position(Position {
            z: 20,
            ..parent.position
        });
        step_detached_holder(&mut holder, &parent, &parent, &DetachedGround).unwrap();
        assert_eq!(holder.position.z, 0);
        assert_eq!(holder.source_dword_10(), 7);
        assert_eq!(holder.action, 25);
    }

    #[test]
    fn owner_support_pickups_cast_through_numeric_scheduler_and_player_consumers() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
        let spells = std::fs::read(
            root.parent()
                .unwrap()
                .join("remc2-native-runtime/CD_Files/DATA/SPELLS.DAT"),
        )
        .expect("actual owner SPELLS.DAT");
        assert_eq!(spells.len(), 26 * 80);
        for family in [5_u8, 8, 10] {
            for tier in 0_u8..3 {
                let offset = usize::from(family) * 80 + 2 + usize::from(tier) * 26;
                let row = &spells[offset..offset + 26];
                let word = |offset| u16::from_le_bytes(row[offset..offset + 2].try_into().unwrap());
                let dword =
                    |offset| i32::from_le_bytes(row[offset..offset + 4].try_into().unwrap());
                let profile =
                    crate::spell_activation::level2_set_spell_profile(family, tier).unwrap();
                assert_eq!(
                    profile.subspell_index,
                    dword(0) as u16,
                    "owner family{family} tier{tier} power"
                );
                assert_eq!(
                    profile.max_mana,
                    dword(4),
                    "owner family{family} tier{tier} mana cost"
                );
                assert_eq!(
                    profile.mana_regen,
                    dword(8),
                    "owner family{family} tier{tier} Castle gate"
                );
                assert_eq!(
                    profile.word_0x30,
                    word(22),
                    "owner family{family} tier{tier} duration"
                );
                assert_eq!(profile.byte_0x3b, u8::from(row[25] & 1 == 0));
                assert_eq!(profile.mana, dword(4) / i32::from(word(22)));
            }
        }
        let owner_rows =
            std::fs::read(root.join("mc-bhq6-ordinary-20261024-b/level16/authored-level.bin"))
                .expect("actual Level16 owner export");
        let authored = parse_authored_features(&owner_rows).expect("owner authored records");
        let inspection =
            NativeWorld::inspect_level2(crate::native_world::tests::level_two_fixture_path())
                .expect("actual Level2 reset package");
        for (row, family) in [(371, 5), (684, 8), (370, 10)] {
            let mut session = inspection
                .level2_reset_session()
                .expect("real source session");
            session.select_level2_campaign_entry_stage();
            let feature = authored[row];
            assert_eq!(
                (feature.kind, feature.subtype, feature.stage_tag),
                (15, family as i16, 2)
            );
            let (slot, owner, start) = {
                let live = &mut session.session.state.live;
                let slot = live
                    .shared_spells
                    .materialize_authored_spell_pickup(
                        &mut live.economy,
                        feature,
                        &live.terrain,
                        &session.session.definition.presentation.particles,
                    )
                    .expect("real persistent constructor")
                    .expect("NewEvent slot");
                let holder = live.shared_spells.pickup(slot).expect("new source");
                let mut start = holder.position;
                start.z = start.z.wrapping_add(100);
                let owner = live.core.state.entity_index;
                live.core.state.position = start;
                live.core.state.target_speed = 0;
                live.core.state.actual_speed = 0;
                let mut player = live.shared_spells.player_entity.clone();
                player.set_position(start);
                player.set_life(player.max_life / 2);
                player.set_mana(50_000);
                player.max_mana = 50_000;
                player.record[144..148].copy_from_slice(&50_000_i32.to_le_bytes());
                live.shared_spells
                    .sync_player_entity_from_authority(&player);
                live.shared_spells.caster.mana = player.mana;
                live.shared_spells.player_mana = player.mana;
                live.economy.entities[usize::from(owner)] = player;
                live.economy
                    .map_chains
                    .sync_entity(super::RenderableMapSeed {
                        entity_index: owner,
                        active: true,
                        x: start.x,
                        y: start.y,
                    })
                    .expect("move player map link");
                (slot, owner, start)
            };
            let castle = session
                .economy_mut()
                .build_castle(start, owner)
                .expect("source Castle prerequisite");
            assert!(
                session
                    .economy_mut()
                    .set_castle_mana_prerequisite_for_test(castle, 50_000)
            );
            for _ in 0..16 {
                session.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    session
                        .dispatch_events()
                        .iter()
                        .all(|e| !matches!(e, Mc2DispatchEvent::Error { .. })),
                    "{:?}",
                    session.dispatch_events()
                );
                if session.shared_spells().players()[0].enabled_spell_holders[family] == slot {
                    break;
                }
            }
            assert_eq!(
                session.shared_spells().players()[0].enabled_spell_holders[family],
                slot,
                "actual overlap acquires family{family}"
            );
            assert!(
                session
                    .economy()
                    .spell_replacements()
                    .iter()
                    .any(|r| r.active && r.family == family as u8),
                "persistent source publishes replacement"
            );
            let before = session
                .session
                .state
                .live
                .shared_spells
                .player_entity
                .clone();
            let xp = session.shared_spells().spell_experience()[family];
            session.step_outer_turn(
                PlayerCommand {
                    opcode: 6,
                    argument_1: family as i8,
                    flags: 0x40,
                    ..PlayerCommand::default()
                },
                0,
            );
            for _ in 0..2 {
                session.step_outer_turn(PlayerCommand::default(), 0);
            }
            assert!(
                session
                    .dispatch_events()
                    .iter()
                    .all(|e| !matches!(e, Mc2DispatchEvent::Error { .. })),
                "{:?}",
                session.dispatch_events()
            );
            let live = &session.session.state.live;
            let player = &live.shared_spells.player_entity;
            assert!(
                live.shared_spells.pickup(slot).unwrap().active_value > 0,
                "ordinary command runs family{family}"
            );
            match family {
                5 => {
                    assert!(player.life > before.life, "Heal reaches real player health");
                    assert_eq!(live.shared_spells.spell_experience()[5], xp + 1);
                }
                8 => assert_ne!(
                    player.control_flags & 0x8000,
                    0,
                    "Rebound reaches real player collision flags"
                ),
                10 => {
                    assert_ne!(
                        (player.position.x, player.position.y),
                        (before.position.x, before.position.y),
                        "Teleport reaches player flight position"
                    );
                    assert_eq!(player.position, live.core.state.position);
                    assert_eq!(live.shared_spells.spell_experience()[10], xp + 1);
                }
                _ => unreachable!(),
            }
            assert_eq!(player.life, live.economy.entities[usize::from(owner)].life);
            assert_eq!(
                player.control_flags,
                live.economy.entities[usize::from(owner)].control_flags
            );
            assert!(
                player.mana < before.mana,
                "source charge reaches player upkeep"
            );
            if family == 8 {
                use crate::native_world::FireballProjectileOrigin;
                use crate::native_world::level2_dynamic_economy::Level2FireballProjectileLaunchPayload;
                let (target, mana, experience, enemy) = {
                    let live = &session.session.state.live;
                    (
                        live.shared_spells.player_entity.position,
                        live.shared_spells.player_entity.mana,
                        live.shared_spells.spell_experience()[8],
                        live.core
                            .players
                            .iter()
                            .find(|player| player.entity_index != owner)
                            .unwrap()
                            .entity_index,
                    )
                };
                let mut launch = target;
                crate::move_entity(&mut launch, 0, 0, -crate::level1_mission::FIREBALL_SPEED);
                let projectile = session
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .spawn_fireball_projectile_with_payload(
                        Level2FireballProjectileLaunchPayload {
                            origin: FireballProjectileOrigin::Dragon,
                            position: launch,
                            yaw: 0,
                            pitch: 0,
                            axis_position: target,
                            source_id_entity_index: enemy,
                            holder_entity_index: 0,
                            source_subspell_index: 250,
                            source_mana: 1_000,
                            source_dword_0x10: 0,
                            target_source_entity_index: owner,
                            target_class: 0,
                            target_model: 0,
                        },
                        0,
                    )
                    .expect("ordinary incoming Fireball constructor");
                session.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    session
                        .dispatch_events()
                        .iter()
                        .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                    "{:?}",
                    session.dispatch_events()
                );
                let live = &session.session.state.live;
                let reflected = live
                    .economy
                    .economy
                    .fireball_projectiles()
                    .iter()
                    .find(|row| row.source_entity_index == projectile)
                    .unwrap();
                assert!(reflected.active && !reflected.drawing_disabled);
                assert_eq!(
                    (
                        reflected.owner_entity_index,
                        reflected.target_source_entity_index
                    ),
                    (owner, enemy)
                );
                assert_eq!(live.shared_spells.spell_experience()[8], experience + 1);
                assert_eq!(live.shared_spells.player_entity.mana, mana - 250);
                assert!(live.core.effect_journal.records().iter().any(|record| matches!(record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound { sound_id: 28, entity_index, .. }) if entity_index == owner)));
                // A projection regression, separate from the ordinary cast
                // above: later self-spell caches must retain the real current
                // Rebound bits rather than replaying a stale full flag word.
                let mut caches = live.shared_spells.clone();
                let parent = caches.player_entity.clone();
                caches.shield = Some(
                    crate::spell_activation::ShieldState::new(
                        0,
                        slot,
                        owner,
                        parent.mana,
                        parent.life,
                    )
                    .expect("source Shield tier"),
                );
                caches.invisible = Some(
                    crate::spell_activation::InvisibleState::new(
                        0,
                        slot,
                        owner,
                        parent.mana,
                        parent.life,
                    )
                    .expect("source Invisible tier"),
                );
                caches.sync_typed_spell_parent_caches_from_player_entity();
                assert_eq!(
                    caches.shield.unwrap().parent_status_flags,
                    parent.control_flags
                );
                assert_eq!(
                    caches.invisible.unwrap().parent_status_flags,
                    parent.control_flags
                );
            }
        }

        // Real imported rival holders exercise the same adapter with a
        // controlled fresh pulse and its otherwise identical unaffordable
        // pulse. The ordinary campaign checks cover their scheduler route.
        let ordinary_root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
            || root.join("mc-bhq6-ordinary-20261025-zclu5fjj"),
            PathBuf::from,
        );
        let status_base = crate::campaign::CampaignSession::open(
            ordinary_root.join("level18"),
            crate::campaign::spec::CampaignSelection::ordinary(18).unwrap(),
        )
        .expect("real Mission18 rival holders")
        .simulation()
        .clone();
        for family in [6_u8, 11, 12] {
            for affordable in [true, false] {
                let mut session = status_base.clone();
                let live = &mut session.session.state.live;
                let rival = live
                    .shared_spells
                    .players
                    .iter()
                    .find(|player| player.is_ai)
                    .unwrap();
                let owner = rival.entity_index;
                let slot = rival.enabled_spell_holders[usize::from(family)];
                let mut holder = live.shared_spells.pickup(slot).unwrap().clone();
                assert_eq!((holder.class, holder.model), (15, family));
                let profile = level2_set_spell_profile(family, 0).unwrap();
                holder.apply_nyphur_spell_profile(profile, 0);
                holder.set_source_action(family * 3);
                holder.active_value = holder.activation_value.cast_signed();
                holder.record[50..52].copy_from_slice(&holder.active_value.to_le_bytes());
                holder.record[58..60].copy_from_slice(&64_i16.to_le_bytes());
                if !affordable {
                    holder.set_source_word_2c(2);
                }
                let cost = holder.max_mana;
                live.shared_spells.sync_pickup_from_authority(&holder);
                live.economy.entities[usize::from(slot)] = holder.clone();
                let parent = &mut live.economy.entities[usize::from(owner)];
                parent.set_mana(if affordable { cost } else { cost - 1 });
                parent.set_mana_regen(100);
                let castle = live
                    .economy
                    .economy
                    .build_castle_at(Position::default(), Position::default(), owner, false, true)
                    .unwrap();
                assert!(
                    live.economy
                        .economy
                        .set_castle_mana_prerequisite_for_test(castle, 50_000)
                );
                let local_before = (
                    live.shared_spells.shield,
                    live.shared_spells.invisible,
                    live.shared_spells.beyond_sight,
                );
                let experience_before = live
                    .shared_spells
                    .players
                    .iter()
                    .find(|player| player.entity_index == owner)
                    .unwrap()
                    .runtime_spell_experience;
                let feedback = step_source_slot(
                    &mut live.shared_spells,
                    &mut live.economy,
                    &mut live.core,
                    slot,
                )
                .unwrap();
                assert_eq!(feedback, Some((owner, false)));
                let current = live.shared_spells.pickup(slot).unwrap();
                if affordable {
                    assert_eq!(current.active_value, holder.active_value - 1);
                    assert!(live.economy.entities[usize::from(owner)].mana_regen <= 0);
                    if family == 6 {
                        assert_ne!(
                            live.economy.entities[usize::from(owner)].control_flags
                                & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
                            0
                        );
                    } else if family == 11 {
                        assert_ne!(
                            current.control_flags & crate::spell_activation::INVISIBLE_STATUS_BIT,
                            0
                        );
                    }
                } else {
                    assert_eq!(current.active_value, 0);
                    assert_eq!(current.source_word_2c(), 0);
                    assert_eq!(
                        current.level_gate, 1,
                        "expiry applies the actual queued profile"
                    );
                }
                assert_eq!(current.record[50..52], current.active_value.to_le_bytes());
                assert_eq!(current.record[58..60], 63_i16.to_le_bytes());
                assert_eq!(
                    (
                        live.shared_spells.shield,
                        live.shared_spells.invisible,
                        live.shared_spells.beyond_sight
                    ),
                    local_before,
                    "a rival pulse cannot replace the local lifecycle projection"
                );
                assert_eq!(
                    live.shared_spells
                        .players
                        .iter()
                        .find(|player| player.entity_index == owner)
                        .unwrap()
                        .runtime_spell_experience,
                    experience_before,
                    "sub6D8B0 awards source class3/model0 only"
                );
            }
        }
        // Castle callbacks address the actual owner's retained holder, not
        // the first family-2 row in the shared spell projection.
        let mut session = status_base.clone();
        let live = &mut session.session.state.live;
        let owners = live.core.players.iter().filter(|player| player.is_ai)
            .filter(|player| player.enabled_spell_holders[2] != 0)
            .take(2).map(|player| (player.entity_index, player.enabled_spell_holders[2]))
            .collect::<Vec<_>>();
        assert_eq!(owners.len(), 2);
        for &(_, slot) in &owners {
            let mut holder = live.shared_spells.pickup(slot).unwrap().clone();
            holder.active_value = 7;
            holder.record[50..52].copy_from_slice(&7_i16.to_le_bytes());
            live.shared_spells.sync_pickup_from_authority(&holder);
            live.economy.entities[usize::from(slot)] = holder;
        }
        let (owner, slot) = owners[0];
        let other_slot = owners[1].1;
        let expected_busy = live.shared_spells.pickup(slot).unwrap()
            .activation_value.wrapping_sub(1).cast_signed();
        assert_eq!(live.shared_spells.apply_castle_holder_source_update(
            &mut live.economy, &live.core.players, &live.nyphur_dispatcher,
            None, owner, true,
        ).unwrap(), Some(slot));
        assert_eq!(live.shared_spells.pickup(slot).unwrap().active_value, expected_busy);
        assert_eq!(live.economy.entities[usize::from(slot)].active_value, expected_busy);
        assert_eq!(live.shared_spells.pickup(other_slot).unwrap().active_value, 7);
        assert_eq!(live.shared_spells.apply_castle_holder_source_update(
            &mut live.economy, &live.core.players, &live.nyphur_dispatcher,
            None, owner, false,
        ).unwrap(), Some(slot));
        assert_eq!(live.shared_spells.pickup(slot).unwrap().active_value, 0);
        assert_eq!(live.economy.entities[usize::from(slot)].active_value, 0);
        let player = live.core.players.iter_mut().find(|player| player.entity_index == owner).unwrap();
        assert!(player.set_enabled_spell_holder_index(2, other_slot));
        assert_eq!(live.shared_spells.apply_castle_holder_source_update(
            &mut live.economy, &live.core.players, &live.nyphur_dispatcher,
            None, owner, false,
        ).unwrap(), None, "another owner's holder cannot satisfy a stale relation");
        assert_eq!(live.shared_spells.pickup(other_slot).unwrap().active_value, 7);
        assert_eq!(live.economy.entities[usize::from(other_slot)].active_value, 7);
    }
}
