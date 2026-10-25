use super::{GoatStepEvent, next_rng};
use crate::{
    PlayerCommand,
    campaign::{CampaignSession, spec::CampaignSelection},
    native_world::Level2EconomyEvent,
};
use std::path::PathBuf;

fn ordinary_goat_package(level: u16) -> crate::campaign::spec::DecodedCampaignPackage {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b")
        },
        PathBuf::from,
    );
    crate::campaign::spec::DecodedCampaignPackage::decode(
        root.join(format!("level{level}")),
        CampaignSelection::ordinary(level).unwrap(),
    )
    .unwrap()
}

#[test]
fn ordinary_goat_resets_use_their_own_routes_instead_of_the_level_one_point() {
    use crate::native_world::{parse_entities, parse_presentation, parse_runtime};
    // These missions remain subject to independent family/objective admission
    // gates. Decode their actual owner records and use the production family
    // constructor without claiming that a whole mission is runnable.
    for (level, count) in [(5, 9), (13, 19), (22, 67)] {
        let package = ordinary_goat_package(level);
        let entities = parse_entities(package.member("entities.bin").unwrap()).unwrap();
        let runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
        let presentation = parse_presentation(package.member("presentation.bin").unwrap()).unwrap();
        let config =
            super::GoatBehaviorConfig::from_parameter_record(&presentation.entity_parameters[98])
                .unwrap();
        assert!(crate::native_world::decode_level_one_goat_stage_point(&runtime).is_err());
        let family =
            super::GoatFamily::from_campaign_entities(&entities, &runtime.stage_variables, config)
                .unwrap_or_else(|error| panic!("Goat mission {level}: {error}"));
        assert_eq!(family.goats().len(), count);
        for goat in family.goats() {
            assert_eq!(
                goat,
                &super::GoatState::from_snapshot(&entities[usize::from(goat.entity_index)])
                    .unwrap()
            );
            if level == 22 {
                assert_eq!(
                    (goat.action, goat.stage_var_1, goat.stage_var_2),
                    (15, 5, 5)
                );
            }
        }
    }
}

#[test]
fn real_point_routes_reject_missing_axes_before_binding_or_counter_mutation() {
    use crate::native_world::{RuntimeRelation, parse_entities, parse_runtime};
    let package = ordinary_goat_package(1);
    let entities = parse_entities(package.member("entities.bin").unwrap()).unwrap();
    let mut runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let mut family = super::GoatFamily::from_campaign_entities(
        &entities,
        &runtime.stage_variables,
        super::GoatBehaviorConfig::LEVEL_ONE,
    )
    .unwrap();
    runtime.stage_variables[3].relation = RuntimeRelation::LiveEntity(152);
    let before = family.clone();
    let runtime_before = runtime.clone();
    assert!(
        super::GoatFamily::from_campaign_entities(
            &entities,
            &runtime.stage_variables,
            super::GoatBehaviorConfig::LEVEL_ONE,
        )
        .unwrap_err()
        .contains("not an axis payload")
    );
    assert!(
        family
            .bind_authored_route(50, 3, &mut runtime)
            .unwrap_err()
            .contains("not an axis payload")
    );
    assert_eq!(family, before);
    assert_eq!(runtime, runtime_before);
}

#[test]
fn level_twenty_two_follow_route_runs_through_the_live_arena_adapter() {
    use crate::{
        campaign::source_arena::SourceArena,
        native_world::{
            Level2EconomyState, MapType, OwnerPackageProfile, OwnerPackageSchema, RuntimeRelation,
            SessionTerrain, parse_entities, parse_players, parse_presentation, parse_runtime,
            parse_terrain,
        },
    };
    let package = ordinary_goat_package(22);
    let entities = parse_entities(package.member("entities.bin").unwrap()).unwrap();
    let mut runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation = parse_presentation(package.member("presentation.bin").unwrap()).unwrap();
    let players = parse_players(
        package.member("players.bin").unwrap(),
        OwnerPackageProfile::Campaign,
        OwnerPackageSchema::V5,
    )
    .unwrap();
    let player = &players[usize::from(runtime.local_player_slot)];
    let actor = &entities[usize::from(player.entity_index)];
    let targets = [super::GoatPlayerTarget {
        entity_index: actor.index,
        position: actor.position,
        control_flags: actor.control_flags,
    }];
    let end = package.member("feature-end.bin").unwrap();
    let terrain = SessionTerrain::from_planes(
        MapType::Day,
        parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        u16::from_le_bytes([end[8], end[9]]),
    );
    let economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
        terrain.clone(),
        &entities,
        actor.mana,
        actor.model,
        0,
        player.enabled_spell_holders[2],
        runtime.random_state,
        Default::default(),
        runtime.constructor_counters(),
    )
    .unwrap();
    let mut family = super::GoatFamily::from_campaign_entities(
        &entities,
        &runtime.stage_variables,
        super::GoatBehaviorConfig::from_parameter_record(&presentation.entity_parameters[98])
            .unwrap(),
    )
    .unwrap();
    let before = family.goats().to_vec();
    assert_eq!(before.len(), 67);
    assert_eq!(
        runtime.stage_variables[5].relation,
        RuntimeRelation::LiveEntity(147)
    );
    let mut arena = SourceArena::new(entities.into_boxed_slice(), economy);
    for _ in 0..16 {
        let frozen = arena.rebuilt_class5_model_slots();
        family
            .prepare_runtime_pass(&mut arena, &mut runtime, &frozen)
            .unwrap();
        family.begin_source_turn(&targets);
        for slot in &frozen[1] {
            family
                .step_runtime_slot(
                    *slot,
                    &terrain,
                    &targets,
                    &mut arena,
                    &runtime,
                    &presentation.particles,
                )
                .unwrap();
        }
        family.finish_source_turn();
    }
    let moved = family
        .goats()
        .iter()
        .zip(&before)
        .filter(|(after, before)| after.position != before.position)
        .count();
    assert!(
        moved > 0,
        "real follow-route callbacks must move their source actors"
    );
    let target = arena.entities[147].position;
    let steering = family
        .goats()
        .iter()
        .filter(|goat| {
            let visited_phase = goat.phase.wrapping_sub(1);
            visited_phase & 7 == 0
                && visited_phase & 63 != 0
                && i32::from(goat.roll) == crate::heading_for_positions(goat.position, target)
        })
        .map(|goat| goat.entity_index)
        .collect::<Vec<_>>();
    assert!(
        !steering.is_empty(),
        "the actual follow route must steer toward its source target on cadence"
    );
    for goat in family.goats() {
        assert!(goat.active && goat.life == 600);
        assert_eq!(
            (goat.action, goat.stage_var_1, goat.stage_var_2),
            (15, 5, 5)
        );
        let source = &arena.entities[usize::from(goat.entity_index)];
        assert_eq!(source.position, goat.position);
        assert_eq!(source.record_bytes()[77], 5);
    }
    eprintln!(
        "Level22: {moved} real Goat roots moved across 16 arena-adapter passes; StageVar5 steers source slots {steering:?} toward source slot147"
    );
}

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
        .join("../../../generated/mc-bhq8-level1-collision-v5-night");
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
    use crate::{
        campaign::backend::load_mc2_campaign,
        game_definition::{
            ChangeSetBuilder, PublicationFault, RequestedTiming, WorkspaceStore, default_catalog,
        },
        level_definition::{CreatureKind, ObjectiveCondition},
    };

    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
        },
        PathBuf::from,
    );
    // Use the normal definition publication path to make the package's first
    // absence objective observe Goats. The same source objective and group-1
    // gate must consume the dynamic group-5 roots, rather than a hand-built
    // presence array or an isolated predicate callback.
    let catalog = default_catalog().unwrap();
    let base = load_mc2_campaign(&package, &catalog).unwrap();
    let workspace = "goat-dynamic-absence";
    let workspace_path = std::env::temp_dir().join(format!(
        "{workspace}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = WorkspaceStore::create(&workspace_path, workspace, base.clone()).unwrap();
    let changes = ChangeSetBuilder::new(
        workspace,
        &base,
        "observe-authored-goat-roots",
        RequestedTiming::SessionReset,
    )
    .put_objective_from(
        sim_definition::adapter::stable_objective_id("objective.clear.undead_horde").unwrap(),
        ObjectiveCondition::NoLivingCreatures(CreatureKind::Skeleton),
        ObjectiveCondition::NoLivingCreatures(CreatureKind::Goat),
    )
    .build()
    .unwrap();
    let validation = store.validate(&catalog, changes.clone()).unwrap();
    assert!(validation.accepted, "{:?}", validation.diagnostics);
    store
        .apply(
            &catalog,
            changes,
            &validation.receipt_hash,
            PublicationFault::None,
        )
        .unwrap();
    let published = store.active_definition().unwrap();
    let mut campaign = CampaignSession::open(&package, CampaignSelection::LEVEL3).unwrap();
    campaign
        .apply_semantic_level_variant(published.level_definition().unwrap())
        .unwrap();
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
    assert_eq!(
        campaign.level2_compatibility().runtime().objectives[0].stage_selectors[0],
        1,
        "a living dynamically constructed Goat blocks the published absence objective"
    );
    assert!(campaign.events().iter().all(|event| !matches!(
        event,
        crate::native_world::Level2DispatchEvent::AuthoredGroupActivated { group: 1, .. }
    )));

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
    assert_eq!(
        campaign.level2_compatibility().runtime().objectives[0].stage_selectors[0],
        1,
        "the killing pass still observes its frozen living-root census"
    );
    step(&mut campaign);
    assert_eq!(
        campaign.level2_compatibility().runtime().objectives[0].stage_selectors[0],
        2,
        "the following pass releases the actual Goat absence objective"
    );
    // Dispatch scratch is cleared each outer turn; retain the actual gate
    // publication when it is emitted, while the death animation continues.
    let mut group_one_published = false;
    for _ in 0..11 {
        step(&mut campaign);
        group_one_published |= campaign.events().iter().any(|event| {
            matches!(
                event,
                crate::native_world::Level2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 127,
                    group: 1,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == [141, 142]
            )
        });
    }
    assert!(
        group_one_published,
        "the source objective-complete producer must publish its real group"
    );
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
