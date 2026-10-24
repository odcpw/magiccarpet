//! Lossless MC2 holder adapter for the support spell behavior owner.

use super::{EntitySnapshot, Level2SharedSpellResetSession, RenderableMapSeed};
use crate::Position;
use crate::campaign::{source_arena::SourceArena, state::CanonicalRuntimeState};
use crate::spell_activation::{level2_set_spell_profile, support_spell_rule};
use crate::spells::support::{
    SupportSpellHolder, SupportSpellPlayer, SupportSpellSound, SupportSpellStep, TeleportCastles,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReboundReflection {
    pub target_slot: u16,
    pub mana_after: i32,
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

/// Compose the authoritative holder/player write with existing numeric arena,
/// flight and presentation consumers. Spell decisions remain in support::step.
pub(super) fn step_source_slot(
    shared: &mut Level2SharedSpellResetSession,
    arena: &mut SourceArena,
    core: &mut CanonicalRuntimeState,
    source_slot: u16,
) -> Result<(), String> {
    let mut holder = shared
        .pickup(source_slot)
        .filter(|holder| {
            holder.active
                && holder.class == 15
                && matches!(holder.model, 5 | 8 | 10)
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
        let local_castle = arena.economy.castle().filter(|castle| {
            castle.active
                && castle.owner_entity_index == player.entity_index
                && arena.economy.player_castle_entity_index() == castle.source_entity_index
        });
        let imported_slot = u16::from_le_bytes([player.record[42], player.record[43]]);
        let imported_castle = arena
            .entities
            .get(usize::from(imported_slot))
            .filter(|castle| imported_slot != 0 && castle.active);
        let castle = local_castle
            .map(|castle| (castle.position, castle.mana))
            .or_else(|| imported_castle.map(|castle| (castle.position, castle.mana)));
        castles.positions[color] = castle.map(|(position, _)| position);
        if player.entity_index == parent_slot {
            castle_mana = castle.map(|(_, mana)| mana);
        }
    }
    let result = step_holder(
        &mut holder,
        parent.as_mut(),
        castle_mana,
        player_color,
        &castles,
    )
    .ok_or_else(|| format!("support spell holder {source_slot} has invalid family/tier"))?;
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
        if result.award_experience && parent.class == 3 && parent.model == 0 {
            shared.award_spell_experience(parent_slot, holder.model, 1);
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::PlayerCommand;
    use crate::level1_mission::parse_authored_features;
    use crate::native_world::{Level2DispatchEvent, NativeWorld};
    use std::path::PathBuf;

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
                    .materialize_authored_spell_pickup(&mut live.economy, feature, &live.terrain)
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
                        .all(|e| !matches!(e, Level2DispatchEvent::Error { .. })),
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
                    .all(|e| !matches!(e, Level2DispatchEvent::Error { .. })),
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
                        },
                        0,
                    )
                    .expect("ordinary incoming Fireball constructor");
                session.step_outer_turn(PlayerCommand::default(), 0);
                assert!(
                    session
                        .dispatch_events()
                        .iter()
                        .all(|event| !matches!(event, Level2DispatchEvent::Error { .. })),
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
    }
}
