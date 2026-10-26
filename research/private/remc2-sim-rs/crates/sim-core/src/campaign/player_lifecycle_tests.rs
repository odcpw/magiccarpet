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
        death_counts(session),
        (1, 1),
        "the following dying visit must not repeat the death transition"
    );
}
