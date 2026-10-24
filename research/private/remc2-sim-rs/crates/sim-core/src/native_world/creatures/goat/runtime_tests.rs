use super::{GoatStepEvent, next_rng};
use crate::{
    PlayerCommand,
    campaign::{CampaignSession, spec::CampaignSelection},
    native_world::Level2EconomyEvent,
};
use std::path::PathBuf;

#[test]
fn authored_group_goats_use_live_damage_mana_projection_and_following_pass_reclaim() {
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
        },
        PathBuf::from,
    );
    let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3).unwrap();
    // Stage the actual group-5 producer boundary, not a second constructor.
    // The eleven placements, descriptors and health come from owner content.
    campaign
        .level2_compatibility_mut()
        .activate_authored_group_prerequisite_for_test(5)
        .unwrap();
    let slots = campaign
        .level2_compatibility()
        .goats()
        .iter()
        .filter(|goat| goat.active)
        .map(|goat| goat.entity_index)
        .collect::<Vec<_>>();
    assert_eq!(slots.len(), 11);
    assert!(
        campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .rebuilt_class5_model_presence()[1]
    );
    let owner = campaign.level2_compatibility().state.entity_index;
    let step = |campaign: &mut CampaignSession| {
        campaign.step(PlayerCommand::default(), 0);
        assert!(
            campaign.events().iter().all(|event| !matches!(
                event,
                crate::native_world::Level2DispatchEvent::Error { .. }
            )),
            "{:?}",
            campaign.events()
        );
    };
    let first = slots[0];
    let before = campaign
        .level2_compatibility()
        .goats()
        .iter()
        .find(|goat| goat.entity_index == first)
        .unwrap()
        .clone();
    assert_eq!((before.life, before.mana, before.action), (600, 300, 9));
    campaign
        .level2_compatibility_mut()
        .apply_fireball_impact_to_firefly_for_test(before.position, owner, 500);
    step(&mut campaign);
    let goat = campaign
        .level2_compatibility()
        .goats()
        .iter()
        .find(|goat| goat.entity_index == first)
        .unwrap();
    assert_eq!(
        (goat.life, goat.action, goat.target_entity_index),
        (100, 14, Some(owner))
    );
    let projected = campaign
        .level2_compatibility()
        .economy()
        .authored_class5()
        .iter()
        .find(|row| row.source_entity_index == first)
        .unwrap();
    assert_eq!(
        (projected.life, projected.action, projected.word_0x96_150),
        (100, 14, owner)
    );

    for slot in &slots {
        let goat = campaign
            .level2_compatibility()
            .goats()
            .iter()
            .find(|goat| goat.entity_index == *slot)
            .unwrap()
            .clone();
        campaign
            .level2_compatibility_mut()
            .apply_fireball_impact_to_firefly_for_test(goat.position, owner, 500);
        if *slot != first {
            campaign
                .level2_compatibility_mut()
                .apply_fireball_impact_to_firefly_for_test(goat.position, owner, 500);
        }
    }
    step(&mut campaign);
    assert!(
        campaign
            .level2_compatibility()
            .goats()
            .iter()
            .filter(|goat| slots.contains(&goat.entity_index))
            .all(|goat| goat.life < 0 && goat.action == 12)
    );
    // Membership was frozen before callbacks. Negative-life roots disappear
    // from absence/countdown input on the following list rebuild.
    assert!(
        !campaign
            .level2_compatibility()
            .session
            .state
            .live
            .economy
            .rebuilt_class5_model_presence()[1]
    );
    for _ in 0..12 {
        step(&mut campaign);
    }
    let session = campaign.level2_compatibility();
    for slot in slots {
        let goat = session
            .goats()
            .iter()
            .find(|goat| goat.entity_index == slot)
            .unwrap();
        assert!(!goat.active);
        assert!(
            session
                .economy()
                .authored_class5()
                .iter()
                .all(|row| row.source_entity_index != slot)
        );
        assert_eq!(session.economy().events().iter().filter(|event| matches!(event, Level2EconomyEvent::ManaSphereSpawned { source_entity_index, mana: 300, .. } if *source_entity_index == slot)).count(), 1);
        assert!(
            session
                .campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .all(|row| row.entity_index != slot
                    || row.render_parameter_index != super::GOAT_RENDER_PARAMETER_INDEX)
        );
    }
}

#[test]
fn start_action_is_stationary_and_retaliation_uses_its_own_wrapper_each_visit() {
    let mut family = super::tests::level_two_family();
    let index = family
        .goats
        .iter()
        .position(|goat| goat.entity_index == 40)
        .unwrap();
    let goat = &mut family.goats[index];
    goat.action = 8;
    goat.phase = 1;
    goat.actual_speed = 200;
    goat.position.z = 0;
    let before = goat.clone();
    family.step_source_slot(40, &super::tests::OpenWorld, &[]);
    assert_eq!(family.goats[index].position, before.position);
    assert_eq!(family.goats[index].phase, 2);
    assert_eq!(family.goats[index].rng, next_rng(before.rng));

    let target = family
        .goats
        .iter()
        .find(|goat| goat.entity_index == 41)
        .unwrap()
        .position;
    family.goats[index].action = 14;
    family.goats[index].target_entity_index = Some(41);
    family.goats[index].position = target;
    family.goats[index].phase = 1;
    family.goats[index].rng = (0..=u16::MAX)
        .find(|seed| next_rng(*seed) % 43 == 0 && next_rng(*seed) % 77 != 0)
        .unwrap();
    let events = family.step_source_slot(40, &super::tests::OpenWorld, &[]);
    assert_eq!(
        family.goats[index].action, 14,
        "ordinary non-player targets remain resolvable"
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GoatStepEvent::Bleat {
            entity_index: 40,
            sound_id: 46,
            ..
        }
    )));
}
