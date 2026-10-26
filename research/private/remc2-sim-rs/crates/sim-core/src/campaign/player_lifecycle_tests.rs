use super::*;
use crate::campaign::{CampaignSession, journal::CampaignEffect, spec::CampaignSelection};
use std::path::PathBuf;

#[test]
fn acquired_shield_lethal_hit_publishes_death_once() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level3-day-block32-campaign-recipe-v2");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3)
        .expect("actual Level 3 opening package");
    let player = campaign.simulation().state.entity_index;
    let pickup = campaign.simulation().entities()[124].clone();
    assert_eq!((pickup.class, pickup.model, pickup.action), (15, 6, 19));
    campaign
        .simulation_mut()
        .set_level2_entity_position(player, pickup.position)
        .expect("approach the original Shield pickup");
    for _ in 0..4 {
        campaign.step(PlayerCommand::default(), 0);
        if campaign
            .simulation()
            .shared_spells()
            .local_player()
            .enabled_spell_holders[6]
            == pickup.index
        {
            break;
        }
    }
    assert_eq!(
        campaign
            .simulation()
            .shared_spells()
            .local_player()
            .enabled_spell_holders[6],
        pickup.index,
        "the ordinary pickup callback acquires Shield"
    );
    let ui_index = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 6)
        .expect("original Shield menu binding") as i8;
    campaign.step(
        PlayerCommand {
            opcode: 31,
            argument_1: ui_index,
            argument_2: 0,
            ..PlayerCommand::default()
        },
        0,
    );
    for _ in 0..8 {
        campaign.step(
            PlayerCommand {
                flags: 0x10,
                ..PlayerCommand::default()
            },
            0,
        );
        if campaign
            .simulation()
            .shared_spells()
            .shield_state()
            .is_some_and(|shield| shield.active_value > 0)
        {
            break;
        }
    }
    assert!(
        campaign
            .simulation()
            .shared_spells()
            .shield_state()
            .is_some_and(|shield| shield.active_value > 0),
        "ordinary spell input arms the acquired holder"
    );
    campaign.step(PlayerCommand::default(), 0);
    assert_ne!(
        campaign
            .simulation()
            .shared_spells()
            .shield_state()
            .unwrap()
            .parent_status_flags
            & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
        0,
        "the scheduled Shield pulse establishes quarter-damage protection"
    );

    let session = campaign.simulation_mut();
    let shield_experience_before = session.shared_spells().spell_experience()[6];
    let attacker = session
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 9)
        .expect("real opening Skeleton supplies the hit owner")
        .index;
    // Isolate a low-life incoming hit, preserving the actual acquired/cast
    // Shield and its numeric schedule. No status or activation is injected.
    {
        let live = &mut session.session.state.live;
        live.player_protection_ticks = 0;
        live.player_life = 10;
        live.economy.entities[usize::from(player)].set_life(10);
        live.shared_spells.set_player_entity_life(10);
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
    }
    session.queue_level2_player_contact_damage(200, attacker);
    let journal_start = session.effect_journal().records().len();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "the real source turn must not roll back: {:?}",
        session.dispatch_events()
    );
    assert_eq!(session.player_life, -40);
    assert_eq!(session.entities()[usize::from(player)].action, 2);
    assert_eq!(
        session.shared_spells().spell_experience()[6],
        shield_experience_before + 1,
        "a real Shield hit awards source family6 experience even when lethal"
    );
    assert_eq!(
        session
            .session
            .state
            .live
            .core
            .players
            .iter()
            .find(|linked| linked.entity_index == player)
            .unwrap()
            .runtime_spell_experience[6],
        shield_experience_before + 1,
        "the canonical player and spell view retain the same hit award"
    );
    let death_counts = |session: &Mc2Simulation| {
        let records = &session.effect_journal().records()[journal_start..];
        let sound = records
            .iter()
            .filter(|record| {
                matches!(
                    record.effect,
                    CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index,
                        sound_id: 16,
                        ..
                    }) if entity_index == player
                )
            })
            .count();
        let scene = records
            .iter()
            .filter(|record| matches!(record.effect, CampaignEffect::SceneChange { scene: 4 }))
            .count();
        (sound, scene)
    };
    assert_eq!(death_counts(session), (1, 1));
    assert_eq!(
        session.effect_journal().records()[journal_start..]
            .iter()
            .filter(|record| matches!(
                record.effect,
                CampaignEffect::Flight(FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::PlayerPain,
                    entity_index,
                    sound_id: 54..=57,
                    ..
                }) if entity_index == player
            ))
            .count(),
        1,
        "Shield reduction retains the source pain feedback before death"
    );
    assert!(
        session.effect_journal().records()[journal_start..]
            .iter()
            .any(|record| matches!(
                record.effect,
                CampaignEffect::SharedSpell(Level2SharedSpellEvent::ShieldDamageApplied {
                    damage,
                    ..
                }) if damage.raw_damage == 200 && damage.applied_damage == 50
            )),
        "the fatal hit still travels through Shield's ordinary damage consumer"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.shared_spells().spell_experience()[6],
        shield_experience_before + 1,
        "an already-dead player's retained damage mailbox cannot award twice"
    );
    assert_eq!(
        death_counts(session),
        (1, 1),
        "the following dying visit must not repeat the death transition"
    );
}

#[test]
fn ordinary_player_hit_reaches_flight_music_rng_and_source_player_views() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level3-day-block32-campaign-recipe-v2");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL3)
        .expect("actual Level 3 opening package");
    let session = campaign.simulation_mut();
    let player = session.state.entity_index;
    let attacker = session
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 9)
        .expect("opening Skeleton")
        .index;
    assert!(
        attacker < player,
        "the attacker moves before the hit consumer"
    );
    let mut position = session.state.position;
    position.z = session.canonical_terrain().ground_height(position) + 8_192;
    session
        .set_level2_entity_position(player, position)
        .expect("isolate one incoming mailbox above ordinary creature contacts");
    let rng_before = session.entities()[usize::from(player)].entity_rng;
    let rng_after = rng_before.wrapping_mul(9_377).wrapping_add(9_439);
    {
        let live = &mut session.session.state.live;
        live.player_protection_ticks = 0;
        live.player_life = 10_000;
        live.economy.entities[usize::from(player)].set_life(10_000);
        live.shared_spells.set_player_entity_life(10_000);
        live.core.state.target_speed = 0;
        live.core.state.actual_speed = 0;
        live.core.state.recoil_speed = 0;
        live.core.state.music_countdown = 0;
    }
    session.queue_level2_player_contact_damage(500, attacker);
    let journal_start = session.effect_journal().records().len();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
        "hit turn must commit: {:?}",
        session.dispatch_events()
    );
    let state = &session.session.state.live.core.state;
    let expected_yaw =
        crate::heading_for_positions(session.entities()[usize::from(attacker)].position, position)
            as i16;
    assert_eq!(state.recoil_yaw, expected_yaw);
    assert_eq!(
        state.recoil_speed, 46,
        "damage/10 recoil is consumed and decays by four in the same flight visit"
    );
    assert_eq!(state.music_countdown, 99);
    assert_ne!(
        (state.position.x, state.position.y),
        (position.x, position.y),
        "the source body applies hit recoil as real movement"
    );
    assert_eq!(
        session.entities()[usize::from(player)].entity_rng,
        rng_after
    );
    assert_eq!(
        session.shared_spells().player_entity().entity_rng,
        rng_after,
        "the shared spell projection cannot retain the pre-hit source RNG"
    );
    for linked in session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .chain(session.shared_spells().players().iter())
        .chain(std::iter::once(session.shared_spells().local_player()))
        .filter(|linked| linked.entity_index == player)
    {
        assert_eq!(&linked.record[32..34], &state.recoil_speed.to_le_bytes());
        assert_eq!(&linked.record[34..36], &state.recoil_yaw.to_le_bytes());
        assert_eq!(&linked.record[38..40], &state.effective_pitch.to_le_bytes());
    }
    let records = &session.effect_journal().records()[journal_start..];
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record.effect,
                CampaignEffect::Flight(FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::PlayerPain,
                    entity_index,
                    sound_id,
                    ..
                }) if entity_index == player && sound_id == 54 + (rng_after & 3)
            ))
            .count(),
        1
    );
    assert!(records.iter().any(|record| matches!(
        record.effect,
        CampaignEffect::Flight(FlightEvent::UpdateMusic { mode: 2, .. })
    )));
    assert!(
        records
            .iter()
            .all(|record| !matches!(record.effect, CampaignEffect::SceneChange { scene: 4 }))
    );
}
