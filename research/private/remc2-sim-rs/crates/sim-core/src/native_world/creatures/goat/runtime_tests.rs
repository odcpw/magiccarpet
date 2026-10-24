use super::{GoatStepEvent, next_rng};
use crate::{
    PlayerCommand,
    campaign::{CampaignSession, spec::CampaignSelection},
    native_world::Level2EconomyEvent,
};
use std::path::PathBuf;

#[test]
fn source_area_mailbox_preserves_zero_words_and_inject_validation() {
    let mut family = super::tests::level_two_family();
    let index = family
        .goats
        .iter()
        .position(|goat| goat.entity_index == 39)
        .unwrap();
    assert!(!family.submit_source_damage(39, 100, family.goats[index].source_id));
    assert!(family.submit_source_damage(39, 200, 0));
    assert_eq!(
        (
            family.goats[index].pending_damage,
            family.goats[index].pending_attacker
        ),
        (200, 0)
    );
    assert!(family.submit_source_damage(39, 0, 111));
    assert_eq!(
        (
            family.goats[index].pending_damage,
            family.goats[index].pending_attacker
        ),
        (0, 111)
    );
    family.step_source_slot(39, &super::tests::OpenWorld, &[]);
    assert_eq!(
        (
            family.goats[index].life,
            family.goats[index].action,
            family.goats[index].target_entity_index
        ),
        (600, 14, Some(111))
    );
    assert!(family.inject_damage(39, 0, 111).is_err());
    assert!(family.inject_damage(39, 100, 0).is_err());
}

#[test]
fn parent_clone_reads_preceding_player_visit_yaw_in_source_slot_order() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf15-level1-package-v3-current");
    let mut campaign = CampaignSession::open(package, CampaignSelection::LEVEL1).unwrap();
    let live = &mut campaign.level2_compatibility_mut().session.state.live;
    let player_slot = live.core.state.entity_index;
    let clone = live
        .goats
        .goats
        .iter_mut()
        .find(|goat| goat.entity_index == 50)
        .unwrap();
    clone.action = 15;
    clone.stage_var_1 = 0;
    clone.stage_var_2 = 12;
    clone.parent_entity_index = player_slot;
    campaign.step(
        PlayerCommand {
            roll: 96,
            flags: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    let live = &campaign.level2_compatibility().session.state.live;
    let parent = &live.economy.entities[usize::from(player_slot)];
    assert_ne!(live.core.state.yaw, 0);
    assert_eq!(parent.yaw, live.core.state.yaw);
    assert_eq!(live.shared_spells.player_entity.yaw, parent.yaw);
    let expected = (parent.position, parent.yaw, parent.source_roll());
    campaign.step(PlayerCommand::default(), 0);
    let goat = campaign
        .level2_compatibility()
        .goats()
        .iter()
        .find(|goat| goat.entity_index == 50)
        .unwrap();
    // Goat50 runs before player152 and copies the preceding visit, rather
    // than a future pose or the auxiliary input-roll accumulator.
    assert_eq!((goat.position, goat.yaw, goat.roll), expected);
}

#[test]
fn parent_clone_reclaimed_parent_resolves_to_source_null_instead_of_tombstone() {
    let mut family = super::tests::level_two_family();
    let parent = family
        .goats
        .iter_mut()
        .find(|goat| goat.entity_index == 40)
        .unwrap();
    parent.active = false;
    parent.yaw = 512;
    let index = family
        .goats
        .iter()
        .position(|goat| goat.entity_index == 39)
        .unwrap();
    family.goats[index].action = 15;
    family.goats[index].stage_var_2 = 12;
    family.goats[index].parent_entity_index = 40;
    family.step_source_slot(39, &super::tests::OpenWorld, &[]);
    assert_eq!(
        (
            family.goats[index].position,
            family.goats[index].yaw,
            family.goats[index].roll
        ),
        (crate::Position::default(), 0, 0)
    );
    assert_ne!(family.goats[index].control_flags & 0x400, 0);
    // On a later source pass, the same numeric slot can hold a new actor.
    // Runtime callbacks must resolve that occupant, not the Goat tombstone.
    family.frozen_model_slots = Some(Box::new(std::array::from_fn(|_| Vec::new())));
    let reused = family
        .source_entities
        .iter_mut()
        .find(|actor| actor.entity_index == 40)
        .unwrap();
    reused.active = true;
    reused.class = 3;
    reused.model = 0;
    reused.life = 10_000;
    reused.yaw = 700;
    reused.roll = 200;
    reused.position = crate::Position {
        x: 12_345,
        y: 23_456,
        z: 100,
    };
    let expected = (reused.position, reused.yaw, reused.roll);
    family.goats[index].control_flags &= !0x400;
    family.step_source_slot(39, &super::tests::OpenWorld, &[]);
    assert_eq!(
        (
            family.goats[index].position,
            family.goats[index].yaw,
            family.goats[index].roll
        ),
        expected
    );
    assert_eq!(family.goats[index].control_flags & 0x400, 0);
}

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
    // The ordinary class-9 Fireball mover consumes this target list before
    // its area pulse can reach the same canonical Goat damage mailbox.
    let arena = &campaign.level2_compatibility().session.state.live.economy;
    let collision_targets = arena.fireball_collision_targets(&arena.entities);
    for slot in &slots {
        let target = collision_targets
            .iter()
            .find(|target| target.source_slot == *slot)
            .expect("authored Goat participates in ordinary Fireball collision");
        let goat = campaign
            .level2_compatibility()
            .goats()
            .iter()
            .find(|goat| goat.entity_index == *slot)
            .unwrap();
        assert_eq!(target.position, goat.position);
        assert_eq!(
            (
                target.half_width,
                target.half_height,
                target.vertical_offset
            ),
            (
                goat.collision_half_width,
                goat.collision_half_height,
                goat.collision_vertical_offset
            )
        );
    }
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
