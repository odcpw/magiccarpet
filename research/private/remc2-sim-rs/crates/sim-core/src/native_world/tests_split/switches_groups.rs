#[test]
fn baraghan_six_real_rival_banishments_produce_ordinary_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::SourceEntityHandle;
    use crate::game_session::{GameCompletionStatus, GameSession};
    use crate::level_definition::ObjectiveCondition;
    use crate::native_world::player_lifecycle_tests::{
        RivalTestGame, rival_test_contact as contact, rival_test_destroy_castle as destroy_castle,
        rival_test_held_spell_generations as held_spell_generations,
        rival_test_participation as participation, rival_test_sim as sim,
        rival_test_sim_mut as sim_mut, rival_test_step as step,
        rival_test_wait_unprotected as wait_unprotected,
    };
    use crate::native_world::Mc2DispatchEvent;

    #[derive(Default)]
    struct ExitReceipt {
        count: usize,
        trigger: Option<(SourceEntityHandle, Position)>,
        portal: Option<SourceEntityHandle>,
    }
    fn visit(game: &mut RivalTestGame, receipt: &mut ExitReceipt, gate: SourceEntityHandle) {
        step(game);
        let state = sim(game);
        for event in state.dispatch_events() {
            if let Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot, group: 12, authored_indices, ..
            } = event {
                assert_eq!(*source_slot, gate.source_slot);
                assert_eq!(authored_indices.as_ref(), [608, 632]);
                receipt.count += 1;
                let trigger = state.economy().authored_switch_producers().iter()
                    .find(|row| row.authored_index == 632 && (row.model, row.action) == (12, 12))
                    .expect("real Baraghan group12 exit632");
                let portal = state.economy().authored_class14().iter()
                    .find(|row| row.authored_index == 608 && row.model == 3)
                    .expect("real Baraghan group12 portal608");
                receipt.trigger = Some((state.source_arena().handle(trigger.source_entity_index).unwrap(), trigger.position));
                receipt.portal = Some(state.source_arena().handle(portal.source_entity_index).unwrap());
            }
        }
    }
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_LEVEL19_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("mc-bhq6-ordinary-20261025-zclu5fjj/level19"));
    let cache = std::env::var_os("MC_LEVEL19_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/level19"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    assert_eq!(definition.level_definition().unwrap().objectives().len(), 1);
    assert_eq!(definition.level_definition().unwrap().objectives()[0].condition(),
        ObjectiveCondition::AllOtherPlayersBanished);
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2219,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    let rivals = [412_u16, 435, 457, 479, 503, 527];
    assert_eq!(player, 411);
    assert_eq!(sim(&game).runtime().player_count, 7);
    assert_eq!(sim(&game).players().iter().take(7).map(|row| row.entity_index).collect::<Vec<_>>(),
        [411, 412, 435, 457, 479, 503, 527]);
    assert!(rivals.iter().all(|actor| participation(&game, *actor) == Some(1)));
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1);
    let handles = rivals.map(|actor| sim(&game).source_arena().handle(actor).unwrap());
    let gate = sim(&game).source_arena().handle(398).unwrap();
    let gate_row = &sim(&game).entities()[usize::from(gate.source_slot)];
    assert_eq!((gate_row.class, gate_row.model, gate_row.action, gate_row.source_id), (11, 4, 4, 12));
    let mut castles = Vec::new();
    let mut holders = Vec::new();
    let mut receipt = ExitReceipt::default();
    for (index, (&actor, &handle)) in rivals.iter().zip(&handles).enumerate() {
        assert_eq!(participation(&game, actor), Some(1));
        wait_unprotected(&mut game, actor);
        if sim(&game).economy().castle_for_owner(actor).is_some() {
            castles.push(destroy_castle(&mut game, actor, player));
        }
        let state = sim(&game);
        let row = &state.entities()[usize::from(actor)];
        let upkeep = state.session.state.live.nyphur_dispatcher.get(handle).unwrap().common_upkeep(row.phase);
        assert!(row.active && row.action == 1 && row.life > 0 && upkeep.word_0x159 == 0 && !upkeep.high_upkeep,
            "real actor must admit contact after its current Castle loss: {row:?}");
        assert!(state.economy().castle_for_owner(actor).is_none());
        let life = row.life;
        holders.extend(held_spell_generations(&game, actor));
        // Only the arrived contact is selected. Rival AI, protection,
        // Castle production, health, death, participation and cleanup run normally.
        contact(&mut game, actor, player, life);
        for _ in 0..768 {
            visit(&mut game, &mut receipt, gate);
            if participation(&game, actor) == Some(0) { break; }
        }
        assert_eq!(sim(&game).source_arena().handle(actor), Some(handle));
        assert_eq!(participation(&game, actor), Some(0));
        assert_eq!(sim(&game).entities()[usize::from(actor)].action, 3);
        assert_eq!(sim(&game).entities()[usize::from(actor)].word_24(), player);
        if index < rivals.len() - 1 {
            assert!(rivals[index + 1..].iter().any(|actor| participation(&game, *actor) == Some(1)));
            assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1,
                "a remaining participant blocks the actual all-rivals objective");
            assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 0);
            assert_eq!(receipt.count, 0);
            assert_eq!(sim(&game).source_arena().handle(gate.source_slot), Some(gate));
        }
    }
    for _ in 0..512 {
        if receipt.count == 1 && castles.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none())
            && holders.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
        visit(&mut game, &mut receipt, gate);
    }
    assert_eq!(receipt.count, 1);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    assert!(castles.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    assert!(holders.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()));
    let (trigger, position) = receipt.trigger.expect("actual exit632");
    let portal = receipt.portal.expect("actual portal608");
    let exit = sim(&game).level_exit_sequence();
    if !exit.active && !exit.complete {
        assert!(sim(&game).source_arena().resolve(trigger).is_some());
        sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
    }
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        visit(&mut game, &mut receipt, gate);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    let exit = sim(&game).level_exit_sequence();
    assert_eq!((exit.trigger_entity_index, exit.portal_entity_index), (trigger.source_slot, portal.source_slot));
    assert_eq!(sim(&game).source_exit_result_byte(), 0x0a);
}

#[test]
fn level_one_group7_switch_releases_all_villagers_once_without_touching_unmatched_group() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};

    let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/settlement-20261105-integration/owner-closed-v6/level1"),
        PathBuf::from,
    );
    let campaign = CampaignSession::open(package, CampaignSelection::LEVEL1)
        .expect("real Level 1 package");
    let mut session = campaign.simulation().clone();
    session.activate_authored_group_prerequisite_for_test(1)
        .expect("real group 1 materializes switch 104");
    let switch = *session.economy().authored_switch_producers().iter()
        .find(|row| row.authored_index == 104 && row.active)
        .expect("real group-7 switch");
    assert_eq!((switch.model, switch.stage_tag), (0, 7));
    let assert_waiting = |session: &Mc2Simulation| {
        let villagers = session.ordinary().villagers();
        assert_eq!(villagers.len(), 28);
        assert!(villagers.iter().all(|row| (row.action, row.stage_var_1, row.stage_var_2) == (111, 2, 7)));
    };
    assert_waiting(&session);
    let mut outside = session.clone();
    outside.activate_authored_group_prerequisite_for_test(255)
        .expect("empty nonmatching group broadcast");
    for _ in 0..12 {
        outside.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_waiting(&outside);
    assert_eq!(outside.runtime().stage_variables[2].flags as u8 & 0x18, 0);
    assert!(outside.economy().authored_switch_producers().iter()
        .any(|row| row.authored_index == 104 && row.active));

    // Test-only placement reaches the genuine authored producer. Public
    // flight evidence is captured separately; this does not claim gameplay.
    let player = session.state.entity_index;
    session.set_level2_entity_position(player, switch.position).unwrap();
    let mut broadcast = false;
    for _ in 0..12 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        broadcast |= session.runtime().stage_variables[2].flags as u8 & 0x18 != 0;
        if session.ordinary().villagers().iter().all(|row| (row.stage_var_1, row.stage_var_2) == (7, 1)) {
            break;
        }
    }
    assert!(broadcast, "real group-7 switch must publish its pulse");
    assert!(session.ordinary().villagers().iter().all(|row| (row.action, row.stage_var_1, row.stage_var_2) == (111, 7, 1)),
        "the shared pulse must reach every live villager: {:?}",
        session.ordinary().villagers().iter().map(|row| (row.entity_index, row.action, row.stage_var_1, row.stage_var_2)).collect::<Vec<_>>());
}

#[test]
fn level_one_group_schedules_unbound_fireflies_after_stagevar_count_clears() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};

    let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq8-level1-collision-v5-night")
        },
        PathBuf::from,
    );
    let campaign =
        CampaignSession::open(package, CampaignSelection::LEVEL1).expect("real Level 1 package");
    let mut session = campaign.simulation().clone();
    let retained = session.runtime().stage_variables.clone();
    assert_ne!(session.runtime().active_stage_variable_count(), 0);
    assert!(
        session
            .session
            .state
            .live
            .core
            .runtime
            .clear_active_stage_variable_count()
    );
    assert_eq!(session.runtime().stage_variables, retained);

    // Stage one real authored group scan, without asserting that count zero
    // occurs in vanilla Level 1 or forcing any objective to complete.
    session
        .activate_authored_group_prerequisite_for_test(6)
        .unwrap();
    assert!(session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 6, authored_indices, .. }
            if authored_indices.as_ref() == [108, 134, 135, 154, 189, 193]
    )));
    let mut slots = Vec::new();
    for authored in [108, 134, 135, 189, 193] {
        let source = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.authored_index == authored && source.active)
            .expect("group 6 Firefly source");
        let state = session
            .economy()
            .authored_fireflies()
            .iter()
            .find(|state| state.source_entity_index == source.source_entity_index)
            .expect("Firefly runtime state");
        assert_eq!(
            (
                source.model,
                source.action,
                state.stage_var1,
                state.stage_var2
            ),
            (19, 153, 0, 0)
        );
        slots.push((source.source_entity_index, source.source_byte_3e));
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    for (slot, phase) in slots {
        let source = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == slot && source.active)
            .expect("scheduled Firefly remains live");
        assert_eq!(source.source_byte_3e, phase.wrapping_add(1));
    }
    assert_eq!(session.runtime().active_stage_variable_count(), 0);
    assert_eq!(session.runtime().stage_variables, retained);
}

#[test]
fn level_nine_switch_publishes_ordered_group_and_binds_live_type_two_target() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};

    let package = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        PathBuf::from,
    );
    let campaign = CampaignSession::open(
        package.join("level9"),
        CampaignSelection::ordinary(9).unwrap(),
    )
    .expect("real Level 9 package");
    let mut session = campaign.simulation().clone();
    let target_stage = session
        .runtime()
        .stages
        .iter()
        .position(|stage| stage.kind == 2 && stage.relation == RuntimeRelation::AuthoredFeature(63))
        .expect("Level 9 type-2 objective names authored row 63");
    let switch = session.entities()[17].clone();
    assert_eq!(
        (switch.class, switch.model, switch.action, switch.source_id),
        (11, 1, 1, 1)
    );
    let player = session.state.entity_index;

    // Source model 1 passes inverse overlap to 6F850. An overlapping player
    // blocks it; leaving the region publishes the real group.
    let mut inside = session.clone();
    let mut distant = switch.position;
    distant.x = distant.x.wrapping_add(0x4000);
    inside.set_level2_entity_position(player, switch.position).unwrap();
    for _ in 0..8 {
        inside.step_outer_turn(PlayerCommand::default(), 0);
        assert!(!inside.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot: 17,
                group: 1,
                ..
            }
        )));
    }
    assert_eq!(
        inside.runtime().stages[target_stage].relation,
        RuntimeRelation::AuthoredFeature(63)
    );

    session
        .set_level2_entity_position(player, distant)
        .unwrap();
    let mut published = false;
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        published |= session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 17, group: 1, authored_indices, .. }
                if authored_indices.as_ref() == [63, 170]
        ));
        if published {
            break;
        }
    }
    assert!(
        published,
        "real switch must publish both source rows in order"
    );
    let target = session
        .economy()
        .authored_house_source_slot(63)
        .expect("authored target house");
    assert_eq!(
        session.runtime().stages[target_stage].relation,
        RuntimeRelation::LiveEntity(target)
    );
    assert_ne!(session.runtime().stages[target_stage].flags & 1, 0);
    let owner = usize::from(session.players()[0].player_color_index());
    let assert_target_projection = |session: &Mc2Simulation, slot: u16| {
        let stage = &session.runtime().stages[target_stage];
        let offset = 192 + target_stage * 14;
        let raw = &session.runtime().bytes()[offset..offset + 14];
        assert_eq!(stage.relation, RuntimeRelation::LiveEntity(slot));
        assert_eq!(raw[1] & 1, 1);
        assert_eq!(raw[6], 2);
        assert_eq!(
            u32::from_le_bytes(raw[10..14].try_into().unwrap()),
            u32::from(slot) + 1
        );
    };
    assert_target_projection(&session, target);
    assert_eq!(
        session.runtime().objectives[owner].stage_selectors[target_stage],
        1,
        "a live target cannot satisfy the death objective"
    );

    // Real BLDGPRM 23 -> 24 -> 28 conversion: submit damage through the
    // House mailbox, then let its ordinary source action construct and bind
    // each successor. No objective selector or target is forced here.
    let mut current = target;
    for (building, successor) in [(23, Some(24)), (24, Some(28)), (28, None)] {
        for _ in 0..32 {
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
        let house = session
            .session
            .state
            .live
            .ordinary
            .houses()
            .iter()
            .find(|house| house.active && house.entity_index == current)
            .expect("live bound House target");
        assert_eq!((house.building_index, house.action), (building, 52));
        assert_eq!(
            session.runtime().objectives[owner].stage_selectors[target_stage],
            1
        );
        assert!(
            session
                .session
                .state
                .live
                .ordinary
                .houses
                .submit_source_damage(current, u16::MAX, player)
        );
        for _ in 0..4 {
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
        if let Some(successor) = successor {
            let next = session
                .session
                .state
                .live
                .ordinary
                .houses()
                .iter()
                .find(|house| house.active && house.building_index == successor)
                .expect("source House conversion created its successor");
            current = next.entity_index;
            assert_target_projection(&session, current);
            assert_eq!(
                session.runtime().objectives[owner].stage_selectors[target_stage],
                1
            );
        } else {
            assert_eq!(
                session.runtime().objectives[owner].stage_selectors[target_stage],
                2,
                "the final bound target's death advances its real type-2 objective"
            );
            for index in [0, 1, 2, 4] {
                assert_eq!(
                    session.runtime().objectives[owner].stage_selectors[index],
                    1,
                    "other Level 9 objectives remain open"
                );
            }
        }
    }
}

#[test]
fn level_two_group27_model17_companion_activates_from_objective_production_route() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-27 model-17 session");
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-27 feature")
            .world_position(0)
    };
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");

    // The real row-154 model-45 source selects group 27 during the
    // same source pass that materializes group 8. The group-27 spell row
    // remains the first event payload item; row 157 is its model-17
    // companion and must be allocated transactionally beside it.
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                group,
                authored_indices,
                ..
            } if *group == 27 && authored_indices.as_ref() == [156, 157]
        )
    }));
    let meteor = session
        .economy()
        .authored_model17()
        .iter()
        .find(|source| source.authored_index == 157)
        .copied()
        .expect("group-27 model-17 companion");
    assert_eq!(
        (
            meteor.source_class,
            meteor.model,
            meteor.action,
            meteor.subspell_index,
            meteor.max_life,
        ),
        (10, 17, 17, 3000, 10)
    );
    let meteor_position = feature_position(157);
    assert_eq!(
        (meteor.position.x, meteor.position.y),
        (meteor_position.x, meteor_position.y)
    );
    assert_eq!(
        meteor.position.z,
        session.canonical_terrain().ground_height(meteor_position)
    );
    assert_eq!(meteor.life, 9);
    assert_eq!(session.economy().model17_spawned_children().len(), 3);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel17,
            source_slot,
            ..
        } if *source_slot == meteor.source_entity_index
    )));

    session.reset();
    assert!(session.economy().authored_model17().is_empty());
    assert!(session.economy().model17_spawned_children().is_empty());
    assert!(session.economy().spell_pickups().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_model32_switch_stays_inert_until_objective_state_two() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective switch session");

    session
        .set_level2_selected_objective_stage_state(0)
        .expect("source objective incomplete state");
    assert_eq!(session.runtime().objectives[0].stage_selectors[3], 0);
    assert_eq!(session.runtime().objectives[0].done, 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().authored_model45().is_empty());
    assert!(!session.level2_objective_done());
    assert!(!session.economy.entities()[32].drawing_disabled());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
    )));

    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.level2_objective_done());
    assert!(session.economy.entities()[32].drawing_disabled());
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 8,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [153, 154]
    )));
}

#[test]
fn level_two_model32_switch_ignores_non_local_objective_owner_offline() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective switch session");
    let remote_color = session
        .players
        .iter()
        .find(|player| player.slot != 0 && player.active_player_index >= 0)
        .map(PlayerSnapshot::player_color_index)
        .expect("fixture has a non-local participating player");
    assert_eq!(session.level2_selected_objective_stage_state(), 1);
    session.runtime.objectives[usize::from(remote_color)].stage_selectors
        [LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX] = 2;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(!session.level2_objective_done());
    assert_eq!(
        session.runtime.objectives[usize::from(remote_color)].done,
        0
    );
    assert_eq!(session.runtime.objectives[0].done, 0);
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 8,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [153, 154]
    )));
}

#[test]
fn level_two_model32_switch_ignores_remote_objective_outside_player_count() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective switch session");
    let remote_color = session
        .players
        .iter()
        .find(|player| player.slot != 0 && player.active_player_index >= 0)
        .map(PlayerSnapshot::player_color_index)
        .expect("fixture has a non-local participating player");
    session.runtime.player_count = 1;
    session.runtime.objectives[usize::from(remote_color)].stage_selectors
        [LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX] = 2;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(!session.level2_objective_done());
    assert_eq!(
        session.runtime.objectives[usize::from(remote_color)].done,
        0
    );
    assert!(session.economy().spell_pickups().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
    )));
}

#[test]
fn level_two_type5_objective_advances_from_player_point_before_group8() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective point session");
    let stage = session
        .runtime()
        .stages
        .get(LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX)
        .cloned()
        .expect("selected type-5 objective stage");
    assert_eq!(stage.kind, 5);
    assert_eq!(
        session.level2_selected_objective_stage_state(),
        1,
        "frame zero starts with the source objective incomplete"
    );
    session
        .runtime
        .set_level2_objective_text_selector(LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX);

    // A point just beyond the source tolerance must not advance the
    // stage, even though the same outer-turn route is otherwise live.
    session.state.position = Position {
        x: (stage.axis_x as u16).wrapping_add(LEVEL2_OBJECTIVE_POINT_RADIUS as u16 + 1),
        y: stage.axis_y as u16,
        z: 0,
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.level2_selected_objective_stage_state(), 1);
    assert!(session.economy().spell_pickups().is_empty());

    // Entering the authored point writes state 2 after the entity pass;
    // the model-32 switch consumes it on the next source turn.
    session.state.position = Position {
        x: stage.axis_x as u16,
        y: stage.axis_y as u16,
        z: 0,
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.level2_selected_objective_stage_state(), 2);
    assert!(session.economy().spell_pickups().is_empty());

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.level2_objective_done());
    assert_eq!(session.economy().spell_pickups().len(), 2);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 8,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [153, 154]
    )));

    session.reset();
    assert_eq!(session.level2_selected_objective_stage_state(), 1);
    assert!(!session.level2_objective_done());
    assert!(session.economy().spell_pickups().is_empty());
}

#[test]
fn level_two_objective_type5_stage_zero_uses_source_gate_and_advances_text() {
    let mut session = level_two_campaign_session();
    let stage = session.runtime().stages[0].clone();
    assert_eq!(stage.kind, 5);
    assert_eq!(session.runtime().objectives[0].text_selector, 0);

    // The source type-5 check uses the active ObjectiveText stage and a
    // signed XY radius. One step beyond that radius is a discriminating
    // negative case on the real outer-turn route.
    session.state.position = Position {
        x: (stage.axis_x as u16).wrapping_add(LEVEL2_OBJECTIVE_POINT_RADIUS as u16 + 1),
        y: stage.axis_y as u16,
        z: 0,
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.runtime().objectives[0].stage_selectors[0], 1);
    assert_eq!(session.runtime().objectives[0].text_selector, 0);

    session.state.position = Position {
        x: stage.axis_x as u16,
        y: stage.axis_y as u16,
        z: 0,
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.runtime().objectives[0].stage_selectors[0], 2);
    // Source progression selects the first remaining stage, rather than
    // leaving the completed type-5 row as the text selector.
    assert_eq!(session.runtime().objectives[0].text_selector, 1);
}

#[test]
fn level_two_objective_type3_ignores_text_selector_and_uses_bound_player_state() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective type-three session");
    let stage = session.runtime().stages[1].clone();
    assert_eq!(stage.kind, 3);
    assert_eq!(stage.flags & 1, 1);
    let RuntimeRelation::LiveEntity(target_index) = stage.relation else {
        panic!("type-3 objective must retain its bound player relation");
    };
    let target_player = session
        .players
        .iter()
        .find(|player| player.entity_index == target_index)
        .expect("type-3 objective target player row");
    assert_ne!(target_player.objective_player_state(), Some(0));

    // Stage 0 is already complete, but ObjectiveText remains 0. The
    // source type-3 predicate has no text-selector gate, so this active
    // target must still leave stage 1 incomplete.
    session.runtime.set_level2_objective_stage_selector(0, 2);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.runtime().objectives[0].stage_selectors[1], 1);

    // A dead/hidden entity alone is not the source terminal state: the
    // referenced player slot remains participating, so kind 3 stays put.
    session.economy.entities[usize::from(target_index)].life = -1;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.runtime().objectives[0].stage_selectors[1], 1);

    // The compact ActPlayerIndex raw byte range is not the type-3
    // carrier. A mutation there must leave the source predicate false.
    session
        .players
        .iter_mut()
        .find(|player| player.entity_index == target_index)
        .expect("type-3 objective target player row")
        .record[6] = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.runtime().objectives[0].stage_selectors[1], 1);

    // The exported Type_str_0x2BDE carrier is authoritative. The source
    // banishment transition writes zero there, then this ordinary
    // objective pass advances and selects the next row.
    session
        .players
        .iter_mut()
        .find(|player| player.entity_index == target_index)
        .expect("type-3 objective target player row")
        .banishment_state_0x006 = Some(0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.runtime().objectives[0].stage_selectors[1], 2);
    assert_eq!(session.runtime().objectives[0].text_selector, 2);
}

#[test]
fn level_two_objective_progression_sets_level_end_after_final_stage() {
    let mut session = level_two_campaign_session();
    let stage = session.runtime().stages[0].clone();
    for stage_index in 1..usize::from(session.runtime().stage_index) {
        session
            .runtime
            .set_level2_objective_stage_selector(stage_index, 0);
    }
    session.runtime.set_level2_objective_text_selector(0);
    session.state.position = Position {
        x: stage.axis_x as u16,
        y: stage.axis_y as u16,
        z: 0,
    };

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert_eq!(session.runtime().objectives[0].stage_selectors[0], 2);
    assert_eq!(session.runtime().objectives[0].is_level_end, 1);
    assert_eq!(session.runtime().bytes()[304], 1);
}

#[test]
fn level_two_model_zero_one_switches_use_strict_xy_predicates() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 switch session");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 switch session");
    let row_65 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 65)
        .copied()
        .expect("model-zero switch row");
    let row_83 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 83)
        .copied()
        .expect("model-one switch row");

    let target_half_depth = i16::from_le_bytes(
        session.economy.entities[111].record_bytes()[90..92]
            .try_into()
            .expect("live model-zero render half-depth"),
    );
    let row_65_half_depth = row_65.word_10.wrapping_shl(8).cast_signed();
    let row_83_half_depth = row_83.word_10.wrapping_shl(8).cast_signed();
    let row_65_strict_limit =
        u16::try_from(i32::from(row_65_half_depth) + i32::from(target_half_depth))
            .expect("positive row-65 strict overlap limit");
    let row_83_strict_limit =
        u16::try_from(i32::from(row_83_half_depth) + i32::from(target_half_depth))
            .expect("positive row-83 strict overlap limit");

    // Model 0 fires strictly inside the source-derived switch + player
    // render extents, but not at the exact boundary or one unit outside.
    session
        .set_level2_entity_position(
            111,
            Position {
                x: row_65.world_position(0).x,
                y: row_65
                    .world_position(0)
                    .y
                    .wrapping_add(row_65_strict_limit - 1),
                z: 0,
            },
        )
        .expect("move model-zero target one unit inside row-65 limit");
    assert!(
        session
            .level2_authored_switch_matches(65)
            .expect("model-zero strict inside predicate")
    );
    session
        .set_level2_entity_position(
            111,
            Position {
                x: row_65.world_position(0).x,
                y: row_65.world_position(0).y.wrapping_add(row_65_strict_limit),
                z: 0,
            },
        )
        .expect("move model-zero target to exact row-65 limit");
    assert!(
        !session
            .level2_authored_switch_matches(65)
            .expect("model-zero strict boundary predicate")
    );
    session
        .set_level2_entity_position(
            111,
            Position {
                x: row_65.world_position(0).x,
                y: row_65
                    .world_position(0)
                    .y
                    .wrapping_add(row_65_strict_limit + 1),
                z: 0,
            },
        )
        .expect("move model-zero target one unit outside row-65 limit");
    assert!(
        !session
            .level2_authored_switch_matches(65)
            .expect("model-zero strict outside predicate")
    );

    // Model 1 is the inverse predicate over the same source-derived extents:
    // false one unit inside, true at the exact boundary and outside it.
    session
        .set_level2_entity_position(
            111,
            Position {
                x: row_83.world_position(0).x,
                y: row_83
                    .world_position(0)
                    .y
                    .wrapping_add(row_83_strict_limit - 1),
                z: 0,
            },
        )
        .expect("move model-zero target one unit inside row-83 limit");
    assert!(
        !session
            .level2_authored_switch_matches(83)
            .expect("model-one strict inside predicate")
    );
    session
        .set_level2_entity_position(
            111,
            Position {
                x: row_83.world_position(0).x,
                y: row_83.world_position(0).y.wrapping_add(row_83_strict_limit),
                z: 0,
            },
        )
        .expect("move model-zero target to exact row-83 limit");
    assert!(
        session
            .level2_authored_switch_matches(83)
            .expect("model-one strict boundary predicate")
    );
    session
        .set_level2_entity_position(
            111,
            Position {
                x: row_83.world_position(0).x,
                y: row_83
                    .world_position(0)
                    .y
                    .wrapping_add(row_83_strict_limit + 1),
                z: 0,
            },
        )
        .expect("move model-zero target one unit outside row-83 limit");
    assert!(
        session
            .level2_authored_switch_matches(83)
            .expect("model-one strict outside predicate")
    );

    session
        .set_level2_entity_position(111, row_65.world_position(0))
        .expect("restore overlap before dead-player check");
    assert!(
        session
            .level2_authored_switch_matches(65)
            .expect("restored model-zero overlap predicate")
    );

    // A dead class-3/model-0 row is absent from remc2's linked player
    // chain, so it cannot satisfy an overlap switch even when its position
    // remains exactly at the authored center.
    session.economy.entities[111].life = -1;
    assert!(
        !session
            .level2_authored_switch_matches(65)
            .expect("dead model-zero row is excluded from overlap scan")
    );
}

#[test]
fn level_two_root_switch_skips_nonzero_authored_row_zero() {
    let mut session = level_two_campaign_session();
    let root = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 109 && root.target_group == 9)
        .copied()
        .expect("real row-109 group-9 switch");
    let wind = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 110)
        .copied()
        .expect("real group-9 Wind row");
    // Source-language negative control only: a matching nonzero sentinel
    // must be ignored. The real row109 -> group9 -> row110 route is unchanged.
    let mut sentinel = wind;
    sentinel.index = 0;
    session.session.state.live.mission.authored_features_mut()[0] = sentinel;
    session
        .set_level2_entity_position(session.state.entity_index, root.position)
        .expect("place the real player at the switch");

    let mut published = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "source dispatch failed: {:?}",
            session.dispatch_events()
        );
        published |= session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 9, authored_indices, .. }
                if *source_slot == root.source_slot && authored_indices.as_ref() == [110]
        ));
        if published {
            break;
        }
    }
    assert!(
        published,
        "the real switch publishes only row110, never row0"
    );
    assert!(
        session
            .economy()
            .authored_model22()
            .iter()
            .any(|source| { source.authored_index == 110 && source.active && source.action == 22 })
    );
    assert_eq!(session.economy().model22_spawned_children().len(), 11);
    let producer = &session.entities()[usize::from(root.source_slot)];
    assert!(producer.active && producer.drawing_disabled());
    let producer_handle = session
        .source_arena()
        .handle(root.source_slot)
        .expect("hidden source retains its original generation");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session.source_arena().resolve(producer_handle).is_none(),
        "the hidden source is reclaimed at the following pass, even if its slot is reused"
    );
}

#[test]
fn level_two_root_switch_materializes_authored_wind_on_production_turn() {
    let mut session = level_two_campaign_session();
    let root = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 109 && root.target_group == 9)
        .copied()
        .expect("authored row 109 group-9 root switch");
    let wind_feature = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 110)
        .copied()
        .expect("authored row 110 wind feature");
    assert_eq!((wind_feature.kind, wind_feature.subtype), (10, 22));
    session
        .set_level2_entity_position(session.state.entity_index, root.position)
        .expect("fixture player position updates the authoritative row and map link");

    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session
            .economy()
            .authored_model22()
            .iter()
            .any(|source| source.authored_index == 110)
        {
            break;
        }
    }

    let wind = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.authored_index == 110)
        .copied()
        .expect("rooted group-9 switch materializes row 110 wind");
    assert_eq!(session.economy().model22_spawned_children().len(), 11);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 9,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [110]
    )));
    assert!(
        !session
            .level2_imported_switch_roots
            .iter()
            .find(|candidate| candidate.source_slot == root.source_slot)
            .expect("rooted row-109 switch state")
            .drawing_enabled
    );
    assert!(session.economy.entities()[usize::from(root.source_slot)].drawing_disabled());
    assert_eq!(wind.model, 22);
    assert_eq!(wind.action, 22);

    // The source switch consumes row 110 once. Expire its temporary wind
    // parent, let the root's ten-tick cooldown clear while out of range,
    // then overlap the root again; no second authored row may appear.
    let wind_slot = wind.source_entity_index;
    session
        .economy_mut()
        .fields
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == wind_slot)
        .expect("live row-110 wind parent")
        .life = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden_wind = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.authored_index == 110)
        .expect("expired wind parent remains allocated through its callback pass");
    assert_ne!(hidden_wind.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert!(session.economy().map_chains().is_linked(wind_slot));
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model22()
            .iter()
            .all(|source| source.authored_index != 110)
    );
    assert!(!session.economy().map_chains().is_linked(wind_slot));
    session
        .set_level2_entity_position(
            session.state.entity_index,
            Position {
                x: root.position.x.wrapping_add(4_096),
                y: root.position.y,
                z: root.position.z,
            },
        )
        .expect("fixture player position updates the authoritative row and map link");
    for _ in 0..12 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    session
        .set_level2_entity_position(session.state.entity_index, root.position)
        .expect("fixture player position updates the authoritative row and map link");
    let mut second_activation = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 9,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == [110]
            )
        }) {
            second_activation = true;
        }
    }
    assert!(!second_activation);
    assert!(
        session
            .economy()
            .authored_model22()
            .iter()
            .all(|source| source.authored_index != 110)
    );

    let mut boundary = level_two_campaign_session();
    let switch_half_depth = i16::from_le_bytes(
        boundary.economy.entities()[usize::from(root.source_slot)].record_bytes()[90..92]
            .try_into()
            .expect("source switch half-depth"),
    );
    let player_half_depth = i16::from_le_bytes(
        boundary.economy.entities()[usize::from(boundary.state.entity_index)].record_bytes()
            [90..92]
            .try_into()
            .expect("source player half-depth"),
    );
    let boundary_distance =
        u16::try_from(i32::from(switch_half_depth) + i32::from(player_half_depth))
            .expect("qualified positive overlap boundary");
    assert!(boundary_distance > 0 && boundary_distance < 0x8000);
    boundary
        .set_level2_entity_position(
            boundary.state.entity_index,
            Position {
                x: root.position.x,
                y: root.position.y.wrapping_add(boundary_distance),
                z: root.position.z,
            },
        )
        .expect("fixture player position updates the authoritative row and map link");
    for _ in 0..16 {
        boundary.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(boundary.economy().authored_model22().is_empty());
    assert!(
        boundary
            .level2_imported_switch_roots
            .iter()
            .find(|candidate| candidate.source_slot == root.source_slot)
            .expect("boundary row-109 switch state")
            .drawing_enabled
    );

    let mut dead = level_two_campaign_session();
    dead.set_level2_entity_position(dead.state.entity_index, root.position)
        .expect("fixture player position updates the authoritative row and map link");
    dead.economy.entities[111].life = -1;
    for _ in 0..16 {
        dead.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(dead.economy().authored_model22().is_empty());
    assert!(
        dead.level2_imported_switch_roots
            .iter()
            .find(|candidate| candidate.source_slot == root.source_slot)
            .expect("dead-player row-109 switch state")
            .drawing_enabled
    );
}

#[test]
fn level_two_root_switches_materialize_all_authored_wind_groups() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    for (root_index, target_group, wind_index) in [
        (109, 9, 110),
        (113, 11, 114),
        (117, 13, 118),
        (121, 15, 122),
    ] {
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 rooted wind-switch session");
        let root = session
            .level2_imported_switch_roots
            .iter()
            .find(|root| root.authored_index == root_index && root.target_group == target_group)
            .copied()
            .expect("authored rooted wind switch");
        session.state.position = root.position;
        for _ in 0..16 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            if session
                .economy()
                .authored_model22()
                .iter()
                .any(|source| source.authored_index == wind_index)
            {
                break;
            }
        }
        assert!(
            session
                .economy()
                .authored_model22()
                .iter()
                .any(|source| source.authored_index == wind_index)
        );
        assert_eq!(session.economy().model22_spawned_children().len(), 11);
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::AuthoredGroupActivated {
                group,
                authored_indices,
                ..
            } if *group == target_group && authored_indices.as_ref() == [wind_index]
        )));
        assert!(
            !session
                .level2_imported_switch_roots
                .iter()
                .find(|candidate| candidate.source_slot == root.source_slot)
                .expect("rooted wind switch state")
                .drawing_enabled
        );
    }
}

#[test]
fn level_two_group15_wind_is_scheduled_in_source_order_and_reclaimed_next_pass() {
    let mut inactive = level_two_campaign_session();
    let inactive_root = inactive
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 121 && root.target_group == 15)
        .copied()
        .expect("authored row 121 group-15 producer");
    let inactive_player = inactive.state.entity_index;
    inactive
        .set_level2_entity_position(
            inactive_player,
            Position {
                x: inactive_root.position.x.wrapping_add(0x8000),
                y: inactive_root.position.y,
                z: inactive_root.position.z,
            },
        )
        .expect("public player-position input remains outside the group-15 producer");
    let mut activated_while_away = false;
    for _ in 0..16 {
        inactive.step_outer_turn(PlayerCommand::default(), 0);
        activated_while_away |= inactive.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 15, .. }
            )
        });
    }
    assert!(!activated_while_away);
    assert!(
        inactive
            .economy()
            .authored_model22()
            .iter()
            .all(|source| source.authored_index != 122)
    );
    assert!(
        inactive
            .level2_imported_switch_roots
            .iter()
            .find(|root| root.source_slot == inactive_root.source_slot)
            .expect("inactive group-15 producer remains live")
            .drawing_enabled
    );

    let mut session = level_two_campaign_session();
    let root = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 121 && root.target_group == 15)
        .copied()
        .expect("authored row 121 group-15 producer");
    let authored_wind = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 122)
        .copied()
        .expect("authored group-15 wind row 122");
    assert_eq!(
        (
            authored_wind.kind,
            authored_wind.subtype,
            authored_wind.activation_group,
        ),
        (10, 22, 15)
    );
    session
        .set_level2_entity_position(session.state.entity_index, root.position)
        .expect("public player-position input reaches the group-15 producer");

    let mut activated = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let events = session.dispatch_events();
        let activation_at = events.iter().position(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot,
                    group: 15,
                    authored_indices,
                    ..
                } if *source_slot == root.source_slot && authored_indices.as_ref() == [122]
            )
        });
        if let Some(activation_at) = activation_at {
            let wind = session
                .economy()
                .authored_model22()
                .iter()
                .find(|source| source.authored_index == 122)
                .copied()
                .expect("the group-15 constructor materializes row 122");
            assert!(
                wind.source_entity_index > root.source_slot,
                "NewEvent places row 122 after the producer's source cursor"
            );
            let consumer_at = events
                .iter()
                .position(|event| {
                    matches!(
                        event,
                        Mc2DispatchEvent::DynamicArena {
                            kind: Level2DynamicArenaKind::AuthoredModel22,
                            source_slot,
                            ..
                        } if *source_slot == wind.source_entity_index
                    )
                })
                .expect("remaining source cursor dispatches row 122 in the producer pass");
            assert!(
                activation_at < consumer_at,
                "the group event precedes row 122's scheduled callback"
            );
            let authored_position = authored_wind.world_position(0);
            assert_eq!((wind.model, wind.action), (22, 22));
            assert_eq!(
                (wind.position.x, wind.position.y, wind.position.z),
                (
                    authored_position.x,
                    authored_position.y,
                    session.canonical_terrain().ground_height(authored_position),
                )
            );
            assert_eq!(session.economy().model22_spawned_children().len(), 11);
            activated = true;
            break;
        }
    }
    assert!(activated, "the real group-15 producer must reach row 122");

    let wind_slot = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.authored_index == 122)
        .expect("row 122 remains live after its scheduled callback")
        .source_entity_index;
    session
        .economy_mut()
        .fields
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == wind_slot)
        .expect("row-122 wind source for lifetime boundary")
        .life = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden_wind = session
        .economy()
        .authored_model22()
        .iter()
        .find(|source| source.source_entity_index == wind_slot)
        .expect("row 122 stays allocated through its callback epilogue");
    assert_ne!(hidden_wind.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert!(session.economy().map_chains().is_linked(wind_slot));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel22,
            source_slot,
            ..
        } if *source_slot == wind_slot
    )));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model22()
            .iter()
            .all(|source| source.source_entity_index != wind_slot)
    );
    assert!(!session.economy().map_chains().is_linked(wind_slot));
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 15, .. }
    )));
}

#[test]
fn level_two_reset_villagers_use_imported_family_source_dispatch() {
    let source_entities = super::parse_entities(
        &std::fs::read(level_two_campaign_fixture_path().join("entities.bin"))
            .expect("qualified immutable Level 2 entity export"),
    )
    .expect("qualified immutable Level 2 source rows");
    let mut session = level_two_campaign_session();
    let mut authored_villagers = session
        .mission()
        .authored_features()
        .iter()
        .copied()
        .filter(|feature| {
            feature.activation_group == 0 && feature.kind == 5 && feature.subtype == 13
        })
        .collect::<Vec<_>>();
    authored_villagers.sort_unstable_by_key(|feature| feature.index);
    let expected_indices = authored_villagers
        .iter()
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    let expected_group0_indices = (284_u16..=290)
        .chain(292..=294)
        .chain(323..=341)
        .collect::<Vec<_>>();
    assert_eq!(expected_indices, expected_group0_indices);
    assert_eq!(
        expected_indices.len(),
        crate::villager::LEVEL_TWO_VILLAGER_INDICES.len()
    );
    // The pinned Level 2 reset runs sub_4A1E0(0,1) before exporting frame
    // zero, so these authored group-0 rows are already imported carriers.
    let imported_villagers = session.ordinary().villagers();
    let imported_slots = imported_villagers
        .iter()
        .map(|villager| villager.entity_index)
        .collect::<Vec<_>>();
    assert_eq!(
        imported_slots,
        crate::villager::LEVEL_TWO_VILLAGER_INDICES.to_vec()
    );
    // The pinned reset already ran the authored group-0 constructors and
    // their source callbacks before exporting frame zero. Use the exported
    // raw entity rows as the position authority; the later source jitter and
    // movement make authored tile centers an invalid expectation.
    let mut expected_positions = imported_slots
        .iter()
        .map(|slot| {
            let source = &source_entities[usize::from(*slot)];
            (source.position.x, source.position.y, source.position.z)
        })
        .collect::<Vec<_>>();
    expected_positions.sort_unstable();

    let mut imported_positions = imported_villagers
        .iter()
        .map(|villager| {
            (
                villager.position.x,
                villager.position.y,
                villager.position.z,
            )
        })
        .collect::<Vec<_>>();
    imported_positions.sort_unstable();
    assert_eq!(imported_positions, expected_positions);
    assert!(imported_villagers.iter().all(|villager| {
        villager.active
            && villager.model == 13
            && villager.action == 105
            && villager.max_life == 1_000
            && villager.life == 1_000
    }));

    let baseline_resident_slots = session
        .economy()
        .house_residents()
        .iter()
        .map(|resident| resident.source_entity_index)
        .collect::<BTreeSet<_>>();
    let imported_slot_set = imported_slots.iter().copied().collect::<BTreeSet<_>>();

    session.step_outer_turn(PlayerCommand::default(), 0);
    let events = session.dispatch_events();
    let scheduled = events
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::Villager {
                pass, source_slot, ..
            } if imported_slot_set.contains(source_slot) => Some((*pass, *source_slot)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scheduled
            .iter()
            .map(|(_, source_slot)| *source_slot)
            .collect::<Vec<_>>(),
        imported_slots
    );
    assert!(scheduled.iter().all(|(pass, _)| *pass == 0));
    assert!(!events.iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 0, .. }
    )));

    let duplicate_group0_residents = session
        .economy()
        .house_residents()
        .iter()
        .filter(|resident| {
            !baseline_resident_slots.contains(&resident.source_entity_index)
                && resident.model == 13
                && expected_positions
                    .binary_search(&(
                        resident.position.x,
                        resident.position.y,
                        resident.position.z,
                    ))
                    .is_ok()
        })
        .map(|resident| resident.source_entity_index)
        .collect::<BTreeSet<_>>();
    assert!(
        duplicate_group0_residents.is_empty(),
        "the frame-zero group-0 Villagers must not be republished as HouseResidents"
    );
}

#[test]
fn level_two_model4_group31_stays_latent_before_player_level_end() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-4 exit-switch session");

    assert!(session.economy().authored_class14().is_empty());
    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(session.economy().authored_class14().is_empty());
    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().authored_trees().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [244]
    )));
    let switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 4
                && entity.action == 4
                && entity.source_id == 31
        })
        .expect("live model-4 group-31 switch");
    assert!(!switch.drawing_disabled());
}

#[test]
fn level_two_model4_group31_activates_for_level_end_local_model_zero_player() {
    let mut session = level_two_campaign_session();
    assert_eq!(
        session.economy.entities()[usize::from(session.state.entity_index)].model,
        0
    );
    let (root_slot, root_phase) = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 4
                && entity.action == 4
                && entity.source_id == 31
        })
        .map(|entity| (entity.index, entity.phase))
        .expect("live model-4 group-31 switch before source dispatch");

    session.runtime.set_level2_is_level_end(1);
    session.step_outer_turn(PlayerCommand::default(), 0);

    let switch28 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 274)
        .copied()
        .expect("row 274 switch producer must materialize with group 31");
    assert_eq!(
        (switch28.model, switch28.action, switch28.stage_tag),
        (0, 0, 28)
    );
    assert_eq!(
        switch28.phase,
        (switch28.source_entity_index as u8).wrapping_add(1)
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == LEVEL2_GROUP31_AUTHORED_INDICES
    )));
    assert!(
        session
            .economy()
            .authored_class14()
            .iter()
            .any(|source| source.authored_index == 244),
        "group 31 must materialize its single class-14/model-3 row"
    );
    let tree_indices = session
        .economy()
        .authored_trees()
        .iter()
        .map(|source| source.authored_index)
        .collect::<Vec<_>>();
    assert_eq!(tree_indices, LEVEL2_GROUP31_TREE_INDICES);
    assert!(session.economy().authored_trees().iter().all(|source| {
        (source.source_class, source.model, source.action) == (2, 0, 0)
            && source.active
            && source.drawing_enabled
    }));
    let switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 4
                && entity.action == 4
                && entity.source_id == 31
        })
        .expect("live model-4 group-31 switch");
    assert_eq!(switch.index, root_slot);
    assert_eq!(switch.phase, root_phase.wrapping_add(1));
    assert!(switch.drawing_disabled());
}

#[test]
fn level_two_model12_group31_materializes_row209_without_empty_group32_work() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    session.runtime.set_level2_is_level_end(1);
    session.step_outer_turn(PlayerCommand::default(), 0);

    let source = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 209)
        .copied()
        .expect("group 31 must materialize authored row 209");
    assert_eq!(
        (
            source.authored_index,
            source.model,
            source.action,
            source.stage_tag,
            source.word_10,
            source.dword_0x10,
        ),
        (209, 12, 12, 32, 4, 0)
    );
    let row209 = session
        .mission()
        .authored_features()
        .iter()
        .copied()
        .find(|feature| feature.index == 209)
        .expect("authored row 209");
    assert_eq!(
        (source.position.x, source.position.y, source.position.z),
        (
            row209.world_position(0).x,
            row209.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(row209.world_position(0))
        )
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == LEVEL2_GROUP31_AUTHORED_INDICES
    )));
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 32, .. }
    )));

    session.reset();
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .all(|source| source.authored_index != 209)
    );
    assert!(session.economy().authored_trees().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_model12_group31_rolls_back_when_row244_cannot_allocate() {
    let mut session = level_two_campaign_session();
    // Let unrelated reset-time producers consume their ordinary first-pass
    // work before constraining the arena. Group 31 is still latent because no
    // player has reached the level-end objective yet.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let saved_free_slots = session.economy().free_slots.clone();
    session.economy_mut().free_slots.truncate(1);
    session.runtime.set_level2_is_level_end(1);
    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .all(|source| source.authored_index != 209)
    );
    assert!(
        session
            .economy()
            .authored_class14()
            .iter()
            .all(|source| source.authored_index != 244)
    );
    assert!(session.economy().authored_trees().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 31, .. }
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Error { message, .. } if message.contains("row 244")
    )));

    session.economy_mut().free_slots = saved_free_slots;
    session.reset();
}

#[test]
fn level_two_model4_group31_requires_a_model_zero_local_player() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-4 exit-switch session");
    session.runtime.set_level2_is_level_end(1);
    let local_index = usize::from(session.state.entity_index);
    session.economy.entities[local_index].model = 1;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(session.economy().authored_class14().is_empty());
    assert!(session.economy().authored_trees().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 31, .. }
    )));
    let switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 4
                && entity.action == 4
                && entity.source_id == 31
        })
        .expect("live model-4 group-31 switch");
    assert!(!switch.drawing_disabled());
}

#[test]
fn level_two_model4_group31_scans_each_live_player_objective() {
    let mut session = level_two_campaign_session();
    let local_index = usize::from(session.state.entity_index);
    session.economy.entities[local_index].model = 1;

    let remote_player = session
        .players
        .iter()
        .find(|player| player.slot != 0 && player.active_player_index >= 0)
        .map(|player| (player.entity_index, player.player_color_index()))
        .expect("fixture has a non-local live player row");
    let remote_index = usize::from(remote_player.0);
    let remote = session
        .economy
        .entities
        .get_mut(remote_index)
        .expect("remote player entity is in the entity arena");
    assert!(remote.active && remote.class == 3 && remote.life >= 0);
    remote.model = 0;
    session.runtime.objectives[usize::from(remote_player.1)].is_level_end = 1;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == LEVEL2_GROUP31_AUTHORED_INDICES
    )));
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 274)
    );
    assert_eq!(
        session
            .economy()
            .authored_trees()
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        LEVEL2_GROUP31_TREE_INDICES
    );
}

#[test]
fn level_two_model4_group31_ignores_dead_player_objective() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-4 exit-switch session");
    session.runtime.set_level2_is_level_end(1);
    let local_index = usize::from(session.state.entity_index);
    let local = session
        .economy
        .entities
        .get_mut(local_index)
        .expect("local player entity is in the entity arena");
    assert!(local.active && local.class == 3 && local.model == 0);
    local.life = -1;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(session.economy().authored_class14().is_empty());
    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().authored_trees().is_empty());
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 31, .. }
    )));
    let switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 4
                && entity.action == 4
                && entity.source_id == 31
        })
        .expect("live model-4 group-31 switch");
    assert!(!switch.drawing_disabled());
}

#[test]
fn level_two_group31_switch274_activates_group28_on_strict_overlap() {
    let mut session = level_two_campaign_session();
    let row_274 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 274)
        .copied()
        .expect("group-31 row-274 switch");
    let expected_group28 = session
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| {
            feature.activation_group == 28 && feature.kind == 14 && feature.subtype == 5
        })
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    session.runtime.set_level2_is_level_end(1);
    session
        .set_level2_entity_position(111, row_274.world_position(0))
        .expect("move model-zero overlap target");

    session.step_outer_turn(PlayerCommand::default(), 0);

    let runtime_switch_slot = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 274)
        .map(|source| source.source_entity_index)
        .expect("row 274 runtime producer remains live after its source callback");
    let switch_phase = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.source_entity_index == runtime_switch_slot)
        .map(|source| source.phase)
        .expect("row 274 phase remains live");
    for _ in 0..level_two_switch_turns_until_cadence(switch_phase) {
        session
            .set_level2_entity_position(111, row_274.world_position(0))
            .expect("keep the public player input on row 274");
        session.step_outer_turn(PlayerCommand::default(), 0);
    }

    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 28,
            authored_indices,
            source_slot,
            ..
        } if *source_slot == runtime_switch_slot
            && authored_indices.as_ref() == expected_group28.as_slice()
    )));
    // Pickup observability is pass-local (RngPrepass clears it). Retain
    // actual collected occurrences before advancing the lower-slot birth.
    let birth_generations = session
        .economy()
        .authored_class14()
        .iter()
        .filter(|source| expected_group28.contains(&source.authored_index))
        .map(|source| {
            (
                source.source_entity_index,
                session
                    .economy()
                    .slot_generation(source.source_entity_index),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(birth_generations.len(), expected_group28.len());
    let mut collected_scroll_slots = session
        .shared_spells()
        .pickup_events()
        .iter()
        .filter_map(|event| match event {
            Level2SharedSpellEvent::ExperienceScrollCollected { source_slot, .. } => {
                Some(*source_slot)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert!(
        collected_scroll_slots
            .iter()
            .all(|slot| birth_generations.contains_key(slot))
    );
    let deferred_scrolls = session
        .economy()
        .authored_class14()
        .iter()
        .filter(|source| {
            source.active
                && source.drawing_enabled
                && expected_group28.contains(&source.authored_index)
        })
        .map(|source| {
            (
                source.source_entity_index,
                session
                    .economy()
                    .slot_generation(source.source_entity_index),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        deferred_scrolls
            .iter()
            .all(|(slot, _)| *slot < runtime_switch_slot),
        "only actual births below the advancing source cursor may await collection: {deferred_scrolls:?}"
    );
    if !deferred_scrolls.is_empty() {
        session
            .set_level2_entity_position(111, row_274.world_position(0))
            .expect("keep the public player input on the source scroll overlap");
        session.step_outer_turn(PlayerCommand::default(), 0);
        for event in session.shared_spells().pickup_events() {
            if let Level2SharedSpellEvent::ExperienceScrollCollected { source_slot, .. } = event {
                assert_eq!(
                    Some(&session.economy().slot_generation(*source_slot)),
                    birth_generations.get(source_slot)
                );
                assert!(
                    collected_scroll_slots.insert(*source_slot),
                    "one authored scroll occurrence cannot be collected twice"
                );
            }
        }
        for (slot, generation) in &deferred_scrolls {
            assert_eq!(session.economy().slot_generation(*slot), *generation);
            assert!(
                session.shared_spells().pickup_events().iter().any(|event| {
                    matches!(event, Level2SharedSpellEvent::ExperienceScrollCollected {
                    source_slot, ..
                } if source_slot == slot)
                }),
                "the following ordinary visit must collect the generation-qualified lower-slot birth"
            );
        }
    }
    assert_eq!(collected_scroll_slots.len(), expected_group28.len());
    let hidden_scroll = session
        .economy()
        .authored_class14()
        .iter()
        .find(|row| row.authored_index == 275)
        .expect("collected group-28 row remains through the callback pass");
    assert!(!hidden_scroll.drawing_enabled);
    assert!(
        session
            .economy()
            .map_chains()
            .is_linked(hidden_scroll.source_entity_index)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .economy()
            .authored_class14()
            .iter()
            .any(|row| row.authored_index == 275)
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
    )));
}

#[test]
fn level_two_group31_switch274_rejects_the_strict_boundary() {
    let mut session = level_two_campaign_session();
    let row_274 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 274)
        .copied()
        .expect("group-31 row-274 switch");
    let switch_half_depth = row_274.word_10.wrapping_shl(8) as i16;
    let player_half_depth = i16::from_le_bytes(
        session.economy.entities()[usize::from(session.state.entity_index)].record_bytes()[90..92]
            .try_into()
            .expect("source player half-depth"),
    );
    let boundary_distance =
        u16::try_from(i32::from(switch_half_depth) + i32::from(player_half_depth))
            .expect("qualified positive overlap boundary");
    assert!(boundary_distance > 0 && boundary_distance < 0x8000);
    let strict_boundary = Position {
        x: row_274.world_position(0).x,
        y: row_274.world_position(0).y.wrapping_add(boundary_distance),
        z: 0,
    };
    session.runtime.set_level2_is_level_end(1);
    session
        .set_level2_entity_position(111, strict_boundary)
        .expect("move model-zero target to strict boundary");

    session.step_outer_turn(PlayerCommand::default(), 0);
    let switch_phase = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 274)
        .map(|source| source.phase)
        .expect("row 274 runtime producer remains live at strict boundary");
    for _ in 0..level_two_switch_turns_until_cadence(switch_phase) {
        session
            .set_level2_entity_position(111, strict_boundary)
            .expect("hold public player input at strict boundary");
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session.dispatch_events().iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
            )),
            "strict boundary must not activate group 28 on any cadence turn"
        );
        assert!(
            (275_u16..=321)
                .all(|authored_index| !session.level2_class14_activated.contains(&authored_index)),
            "strict boundary must not persist group-28 activation rows"
        );
    }

    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
    )));
    assert!(
        session
            .economy()
            .authored_class14()
            .iter()
            .all(|row| row.authored_index < 275 || row.authored_index > 321),
        "strict boundary must leave group 28 latent"
    );
}

#[test]
fn level_two_group31_switch274_and_group28_reset_to_baseline() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let row_274 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 274)
        .copied()
        .expect("group-31 row-274 switch");
    session.runtime.set_level2_is_level_end(1);
    session
        .set_level2_entity_position(111, row_274.world_position(0))
        .expect("move model-zero overlap target");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let (runtime_switch_slot, switch_phase) = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 274)
        .map(|source| (source.source_entity_index, source.phase))
        .expect("row 274 runtime producer remains live before reset");
    for _ in 0..level_two_switch_turns_until_cadence(switch_phase) {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 28,
            source_slot,
            ..
        } if *source_slot == runtime_switch_slot
    )));

    session.reset();

    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().authored_class14().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group21_day_bees_inherit_l2_06_from_ordinary_production() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    assert_eq!(session.runtime().stage_variables[6].word_2, 0);

    // Row 228 is the ordinary model-32 producer. Its real group-21
    // callback constructs the three Day Bees after the package objective
    // reaches state 2; no test mutates their action or StageVar fields.
    fire_level_two_model32_group(&mut session, 21);
    let group21 = [229_u16, 230, 231]
        .into_iter()
        .map(|authored_index| {
            session
                .economy()
                .bee_creatures()
                .iter()
                .copied()
                .find(|source| source.authored_index() == Some(authored_index))
                .expect("ordinary group-21 Day Bee")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        group21
            .iter()
            .map(|source| {
                (
                    source.authored_index().expect("authored group-21 Bee"),
                    source.action,
                    source.stage_var1,
                    source.stage_var2,
                    source.stage_target,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                229,
                23,
                6,
                2,
                Some(Position {
                    x: 0x6500,
                    y: 0x1000,
                    z: 0
                })
            ),
            (
                230,
                23,
                6,
                2,
                Some(Position {
                    x: 0x6500,
                    y: 0x1000,
                    z: 0
                })
            ),
            (
                231,
                23,
                6,
                2,
                Some(Position {
                    x: 0x6500,
                    y: 0x1000,
                    z: 0
                })
            ),
        ]
    );
    // The source-owned L2-06 high-byte cadence advances once for each of the
    // three successful group-21 constructors. Merely live producers from
    // other groups do not activate their authored rows.
    assert_eq!(session.runtime().stage_variables[6].word_2, 0x0300);
    assert_eq!(
        &session.runtime().bytes()[392 + 6 * 12 + 2..392 + 6 * 12 + 4],
        &0x0300_u16.to_le_bytes()
    );

    session.reset();
    assert_eq!(session.runtime().stage_variables[6].word_2, 0);
    assert!(session.economy().bee_creatures().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group21_bee_extinction_advances_type7_objective_on_production_path() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    assert_eq!(
        session.runtime().stages[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX].kind,
        7
    );
    assert_eq!(
        session.runtime().stages[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX].relation,
        RuntimeRelation::Scalar(2)
    );
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        1
    );
    assert_eq!(session.runtime().objectives[0].text_selector, 0);

    // Use the real reset producer route and select only the terminal
    // action on the resulting live authored row. The source objective
    // predicate is gated by the currently selected objective stage, so
    // select stage 2 explicitly without pre-marking its completion bit.
    session
        .runtime
        .set_level2_objective_text_selector(LEVEL2_BEE_OBJECTIVE_STAGE_INDEX);
    assert_eq!(session.runtime().objectives[0].text_selector, 2);
    fire_level_two_model32_group(&mut session, 21);
    let row229_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX))
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee");
    session
        .economy_mut()
        .bee_creatures
        .iter_mut()
        .find(|bee| bee.source_entity_index == row229_slot)
        .expect("production row 229 source")
        .action = 20;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .expect("action-21 row remains before kill callback")
            .action,
        21
    );
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        1
    );

    let mut phase_zero_callback_seen = false;
    for _ in 0..10 {
        let before = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied()
            .expect("phase-gated action-21 row remains until its callback");
        session.step_outer_turn(PlayerCommand::default(), 0);
        let after = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied()
            .expect("phase-gated action-21 row remains until the following boundary");
        if before.source_byte_3e & 7 == 0 {
            phase_zero_callback_seen = true;
            assert!(
                after.active,
                "drawing disable retains the allocated source row"
            );
            assert_eq!(after.life, before.life);
            assert!(session.economy().map_chains.is_linked(row229_slot));
            assert!(!after.drawing_enabled);
        } else {
            assert!(after.active);
            assert!(after.drawing_enabled);
        }
        if phase_zero_callback_seen {
            break;
        }
    }
    assert!(phase_zero_callback_seen);
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        1,
        "the final Bee remains in the pass-start model-2 census on its hide turn"
    );
    assert_eq!(
        session.runtime().bytes()[304 + LEVEL2_BEE_OBJECTIVE_STAGE_INDEX + 3],
        1
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .all(|bee| bee.authored_index() != Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX))
    );
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        1,
        "row 229 alone cannot satisfy the model-2 absence objective"
    );
    assert_eq!(
        session.runtime().bytes()[304 + LEVEL2_BEE_OBJECTIVE_STAGE_INDEX + 3],
        1
    );
    assert_eq!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .filter_map(BeeCreatureRuntime::authored_index)
            .collect::<Vec<_>>(),
        vec![230, 231],
        "the other two package Bees still occupy the model-2 census"
    );

    for bee in session
        .economy_mut()
        .bee_creatures
        .iter_mut()
        .filter(|bee| matches!(bee.authored_index(), Some(230 | 231)))
    {
        bee.action = 20;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .filter(|bee| matches!(bee.authored_index(), Some(230 | 231)))
            .all(|bee| bee.action == 21 && bee.active && bee.drawing_enabled)
    );

    let mut final_hide_pass_seen = false;
    for _ in 0..10 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let remaining_drawn = session
            .economy()
            .bee_creatures()
            .iter()
            .filter(|bee| {
                matches!(bee.authored_index(), Some(230 | 231)) && bee.active && bee.drawing_enabled
            })
            .count();
        if remaining_drawn == 0 {
            final_hide_pass_seen = true;
            assert_eq!(
                session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
                1,
                "the final hide pass still uses its frozen model-2 census"
            );
            break;
        }
    }
    assert!(
        final_hide_pass_seen,
        "all group-21 Bees reach phase-gated hide"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .all(|bee| { !matches!(bee.authored_index(), Some(229 | 230 | 231)) })
    );
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        2,
        "the following pass rebuilds an empty model-2 census before objective service"
    );
    assert_eq!(
        session.runtime().bytes()[304 + LEVEL2_BEE_OBJECTIVE_STAGE_INDEX + 3],
        2
    );

    // Reset restores the selector and the production activation ledger.
    // Killing the exact authored target while another objective stage is
    // selected must not advance the type-7 slot.
    session.reset();
    assert_eq!(session.runtime().objectives[0].text_selector, 0);
    fire_level_two_model32_group(&mut session, 21);
    let row229_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX))
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee for selector-negative path");
    session
        .economy_mut()
        .bee_creatures
        .iter_mut()
        .find(|bee| bee.source_entity_index == row229_slot)
        .expect("production row 229 source for selector-negative path")
        .action = 20;
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        1
    );
    session.reset();
    assert_eq!(
        session.runtime().objectives[0].stage_selectors[LEVEL2_BEE_OBJECTIVE_STAGE_INDEX],
        1
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group2_row42_materializes_from_row41_production_route() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let local_player = session.state.entity_index;
    let authored_features = session.mission().authored_features().to_vec();
    let feature = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-2 feature")
    };

    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .all(|source| source.authored_index != 42)
    );
    level_two_reach_row42(
        &mut session,
        local_player,
        feature(11).world_position(0),
        feature(42).world_position(0),
    );

    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 2,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [42]
    )));
    let row42 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 42)
        .copied()
        .expect("group 2 must materialize row 42");
    assert_eq!(
        (
            row42.source_class,
            row42.model,
            row42.action,
            row42.stage_tag,
            row42.word_10,
            row42.dword_0x10,
        ),
        (11, 0, 0, 3, 17, 0)
    );
    let authored_row42 = feature(42);
    assert_eq!(
        (row42.position.x, row42.position.y, row42.position.z),
        (
            authored_row42.world_position(0).x,
            authored_row42.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(authored_row42.world_position(0))
        )
    );
    assert!(row42.active);
    assert!(row42.drawing_enabled);
    assert!(!row42.fired);

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_switch_band_a_is_complete_and_tamper_sensitive() {
    let mut session = level_two_campaign_session();
    // This is the same campaign-entry seam used by sim-capi before the
    // first generation is exposed; it is not a test-only runtime edit.
    session.select_level2_campaign_entry_stage();
    let baseline_hash = session.campaign_state_hash();
    let local_player = session.state.entity_index;
    let authored = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("owned activation feature")
            .world_position(0)
    };
    let objective_position = |session: &Mc2Simulation| {
        let stage = session
            .runtime()
            .stages
            .get(LEVEL2_SELECTED_OBJECTIVE_STAGE_INDEX)
            .expect("selected objective stage");
        Position {
            x: stage.axis_x as u16,
            y: stage.axis_y as u16,
            z: 0,
        }
    };
    let mut activation_trace = Vec::<(u16, u16)>::new();
    let step_at = |session: &mut Mc2Simulation,
                   position: Position,
                   turns: usize,
                   trace: &mut Vec<(u16, u16)>| {
        session
            .set_level2_entity_position(local_player, position)
            .expect("public player-position input");
        assert_eq!(
            session.state.position,
            session.economy.entities()[usize::from(local_player)].position,
            "public local-player position input keeps state/entity projections coherent"
        );
        for _ in 0..turns {
            session
                .set_level2_entity_position(local_player, position)
                .expect("public player-position input remains coherent");
            session.step_outer_turn(PlayerCommand::default(), 0);
            for event in session.dispatch_events() {
                if let Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } = event
                {
                    if (1..=8).contains(group) {
                        trace.extend(
                            authored_indices
                                .iter()
                                .copied()
                                .map(|authored_index| (*group, authored_index)),
                        );
                    }
                }
            }
        }
    };
    let wait_for_row42 =
        |session: &mut Mc2Simulation, _row11_position: Position, trace: &mut Vec<(u16, u16)>| {
            if !session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|source| source.authored_index == 42)
            {
                let row41 = session
                    .economy()
                    .authored_switch_producers()
                    .iter()
                    .find(|source| source.authored_index == 41)
                    .copied()
                    .expect("row 41 producer remains live before its cadence");
                assert_eq!(row41.model, 1);
                let mut inverse_position = row41.position;
                inverse_position.x = inverse_position.x.wrapping_add(0x8000);
                step_at(
                    session,
                    inverse_position,
                    level_two_switch_turns_until_cadence(row41.phase),
                    trace,
                );
            }
        };

    let step_source =
        |session: &mut Mc2Simulation, authored_index: u16, trace: &mut Vec<(u16, u16)>| {
            let source = session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.authored_index == authored_index)
                .copied()
                .expect("next authored producer is live before its source cadence");
            let mut position = source.position;
            if source.model == 1 {
                position.x = position.x.wrapping_add(0x8000);
            }
            step_at(
                session,
                position,
                level_two_switch_turns_until_cadence(source.phase),
                trace,
            );
        };

    // Ordinary public input follows the rooted row-11 -> row-41 -> row-42
    // chain, then the row-65 switch and the source objective point. No
    // group activation or authored constructor is called by the test.
    step_at(&mut session, feature_position(11), 1, &mut activation_trace);
    wait_for_row42(&mut session, feature_position(11), &mut activation_trace);
    step_at(
        &mut session,
        feature_position(42),
        16,
        &mut activation_trace,
    );
    step_source(&mut session, 64, &mut activation_trace);
    step_at(
        &mut session,
        feature_position(65),
        16,
        &mut activation_trace,
    );
    step_source(&mut session, 83, &mut activation_trace);
    step_source(&mut session, 84, &mut activation_trace);
    let objective_input_position = objective_position(&session);
    step_at(
        &mut session,
        objective_input_position,
        3,
        &mut activation_trace,
    );
    let plan = session
        .mission()
        .level2_activation_plan()
        .expect("artifact-derived owned activation plan");
    assert_eq!(plan.owned_rows().len(), 151);
    assert_eq!(plan.edges().len(), 18);
    let mut expected = (1_u16..=8)
        .flat_map(|group| {
            plan.group_rows(group)
                .expect("band-A artifact group")
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = activation_trace
        .iter()
        .map(|(_, authored_index)| *authored_index)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "every owned row must activate exactly once"
    );
    let mut counts = BTreeMap::<u16, usize>::new();
    for (_, authored_index) in &activation_trace {
        *counts.entry(*authored_index).or_default() += 1;
    }
    assert!(counts.values().all(|count| *count == 1));
    assert!(activation_trace.contains(&(3, 108)));
    assert!(
        (125_u16..=151).all(|authored_index| { activation_trace.contains(&(3, authored_index)) })
    );
    let traders = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| (125..=151).contains(&source.authored_index))
        .collect::<Vec<_>>();
    assert_eq!(traders.len(), 27);
    for source in &traders {
        assert!(
            source.model == 14
                && source.action == 113
                && source.minimum_speed == 54
                && source.maximum_speed == 18
                && source.actual_speed == if source.word_0x96_150 == 0 { 18 } else { 30 }
                && source.max_life == 1_000
                && source.life == 1_000
                && source.mana == 0
                && source.subspell_index == 500
                && source.source_byte_39 == 64
                && source.phase > 0
                && source.active,
            "ordinary group-3 Trader diverged from its source packet: {source:?}"
        );
    }
    assert!(
        traders
            .iter()
            .any(|source| source.word_0x96_150 != 0 && source.actual_speed == 30),
        "ordinary outer turns must reach Trader action 113 target acquisition"
    );
    let trader_runtime = session.economy().authored_class5_runtime();
    assert_eq!(trader_runtime.len(), 27);
    assert!(trader_runtime.iter().all(|state| {
        state.byte_0x38 == 1
            && state.xtype == 3
            && state.descriptor_index == 100
            && state.pending_damage == 0
            && state.linked_head == 0
    }));

    let first_trace = activation_trace.clone();
    session.reset();
    session.select_level2_campaign_entry_stage();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert!(session.economy().authored_class5().is_empty());
    assert!(session.economy().authored_class5_runtime().is_empty());
    activation_trace.clear();
    step_at(&mut session, feature_position(11), 1, &mut activation_trace);
    wait_for_row42(&mut session, feature_position(11), &mut activation_trace);
    step_at(
        &mut session,
        feature_position(42),
        16,
        &mut activation_trace,
    );
    step_source(&mut session, 64, &mut activation_trace);
    step_at(
        &mut session,
        feature_position(65),
        16,
        &mut activation_trace,
    );
    step_source(&mut session, 83, &mut activation_trace);
    step_source(&mut session, 84, &mut activation_trace);
    let objective_input_position = objective_position(&session);
    step_at(
        &mut session,
        objective_input_position,
        3,
        &mut activation_trace,
    );
    assert_eq!(
        activation_trace, first_trace,
        "reset/replay must preserve the route"
    );

    let mut tampered_group = session.mission().authored_features().to_vec();
    tampered_group
        .iter_mut()
        .find(|feature| feature.index == 41)
        .expect("producer-owned row 41")
        .activation_group = 8;
    assert!(checked_in_level2_activation_plan(&tampered_group).is_err());
    let mut tampered_edge = session.mission().authored_features().to_vec();
    tampered_edge
        .iter_mut()
        .find(|feature| feature.index == 11)
        .expect("root producer row 11")
        .stage_tag = 8;
    assert!(checked_in_level2_activation_plan(&tampered_edge).is_err());

    let mut allocation_failure = level_two_campaign_session();
    allocation_failure.select_level2_campaign_entry_stage();
    let mut allocation_trace = Vec::new();
    step_at(
        &mut allocation_failure,
        feature_position(11),
        1,
        &mut allocation_trace,
    );
    wait_for_row42(
        &mut allocation_failure,
        feature_position(11),
        &mut allocation_trace,
    );
    step_at(
        &mut allocation_failure,
        feature_position(42),
        0,
        &mut allocation_trace,
    );

    // Leave exactly thirty real arena slots for the next public source
    // pass: the established group-3 batch consumes 22 main rows plus
    // model-54 row 63, and action 58 consumes three real model-0 child
    // slots (993..=995), leaving four slots for a partial trader batch.
    // This is the narrowly authorized stale-band accounting correction in
    // mc-switch-regression-dzgvb comment 574; the pinned source remains
    // low-first and all transactional/RNG/reset negatives stay intact.
    let expected_group3_main_indices = (43_u16..=62).chain([64, 439]).collect::<Vec<_>>();
    let action58_child_count = 3;
    let established_allocation_count =
        expected_group3_main_indices.len() + 1 + action58_child_count;
    assert_eq!(established_allocation_count, 26);
    allocation_failure.economy.free_slots.truncate(30);
    let trader_capacity =
        allocation_failure.economy.free_slots.len() - established_allocation_count;
    assert_eq!(trader_capacity, 4);
    let pre_trader_free_slots = allocation_failure.economy.free_slots[..trader_capacity].to_vec();
    let pre_trader_global_rng = allocation_failure.economy.global_rng;
    let snapshot_free_slot_state = |economy: &Level2EconomyState, slots: &[u16]| {
        slots
            .iter()
            .map(|slot| {
                (
                    *slot,
                    economy.slot_generations.get(slot).copied(),
                    economy.slot_rng.get(slot).copied(),
                    economy.source_rng.get(slot).copied(),
                )
            })
            .collect::<Vec<_>>()
    };
    let pre_trader_slot_state =
        snapshot_free_slot_state(&allocation_failure.economy, &pre_trader_free_slots);
    let pre_trader_model14_phase = allocation_failure.economy.class5_model14_constructor_phase;

    let row42_phase = allocation_failure
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 42)
        .map(|source| source.phase)
        .expect("row 42 producer remains live before its failure-route cadence");
    let row42_turns = level_two_switch_turns_until_cadence(row42_phase);
    step_at(
        &mut allocation_failure,
        feature_position(42),
        row42_turns,
        &mut allocation_trace,
    );
    let group3_main_batches = allocation_failure
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated {
                group: 3,
                authored_indices,
                ..
            } => Some(authored_indices),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_group3_consumed_indices = allocation_failure
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| feature.activation_group == 3)
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    let established_group3_batch = group3_main_batches
        .iter()
        .find(|batch| batch.as_ref() == expected_group3_consumed_indices.as_slice())
        .expect("the source group scan publishes its complete consumed-row receipt");
    assert_eq!(
        established_group3_batch
            .iter()
            .filter(|index| expected_group3_main_indices.contains(index))
            .count(),
        22,
        "the full group3 receipt still contains every established main row"
    );
    assert_eq!(
        allocation_failure
            .economy
            .mana_attraction_fields()
            .iter()
            .filter(|field| field.authored_index == 63)
            .count(),
        1
    );
    assert!(
        allocation_failure
            .dispatch_events()
            .iter()
            .any(|event| matches!(
                event,
                Mc2DispatchEvent::Error { message, .. }
                    if message.contains("model-14 row 132")
            ))
    );
    assert!(
        allocation_failure
            .economy
            .authored_class5()
            .iter()
            .all(|source| !(125..=151).contains(&source.authored_index))
    );
    assert!(
        allocation_failure
            .economy
            .authored_class5_runtime()
            .is_empty(),
        "transactional Trader failure must not leave sidecar rows"
    );
    assert!(
        (125_u16..=151)
            .all(|index| { !allocation_failure.level2_class5_activated.contains(&index) })
    );
    assert_eq!(allocation_failure.economy.free_slots, pre_trader_free_slots);
    assert_eq!(
        allocation_failure.economy.global_rng,
        (0..row42_turns).fold(pre_trader_global_rng, |rng, _| level2_next_rng32(rng))
    );
    assert_eq!(
        allocation_failure.economy.class5_model14_constructor_phase,
        pre_trader_model14_phase
    );
    assert_eq!(
        snapshot_free_slot_state(&allocation_failure.economy, &pre_trader_free_slots),
        pre_trader_slot_state
    );
    allocation_failure.reset();
    assert_eq!(allocation_failure.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_switch_band_b_is_complete_and_tamper_sensitive() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let local_player = session.state.entity_index;

    let drive_band = |session: &mut Mc2Simulation| {
        let mut trace = Vec::<(u16, u16)>::new();
        let mut source_trace = Vec::new();
        let mut model32_history = BTreeMap::<u16, (Level2AuthoredModel32Source, u32)>::new();
        let mut completed_model32_history = BTreeMap::new();
        let mut step_at = |session: &mut Mc2Simulation,
                           position: Position,
                           turns: usize,
                           trace: &mut Vec<(u16, u16)>| {
            for _ in 0..turns {
                session
                    .set_level2_entity_position(local_player, position)
                    .expect("public local-player position input");
                session.step_outer_turn(PlayerCommand::default(), 0);
                source_trace.extend_from_slice(session.dispatch_events());
                // Row208 can construct, consume group18, hide and be
                // reclaimed before this outer-turn observation. Retain its
                // actual scheduled source occurrence, not an invented live copy.
                let schedule = session.cf10_production_schedule_receipts();
                for event in session.dispatch_events() {
                    if let Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 18,
                        source_slot,
                        authored_indices,
                        ..
                    } = event
                    {
                        assert!(authored_indices.contains(&213));
                        let selector = schedule
                            .iter()
                            .find_map(|receipt| match receipt {
                                crate::campaign::schedule::ScheduleReceipt::CallbackCompleted {
                                    slot,
                                    selector,
                                    ..
                                } if slot == source_slot => Some(selector),
                                _ => None,
                            })
                            .expect("group18 is produced by the actual row208 callback");
                        let cursor = selector
                            .observed_dynamic_cursor_row()
                            .expect("row208 is a real dynamic source occurrence");
                        assert_eq!(cursor.source_slot, *source_slot);
                        assert_eq!(cursor.kind, Level2DynamicArenaKind::AuthoredModel32);
                        assert!(cursor.generation > 0);
                        assert_eq!(
                            selector.registry_key().selector,
                            crate::campaign::spec::BehaviorSelector::ClassAction {
                                class: 11,
                                action: 32
                            }
                        );
                        assert!(
                            schedule.iter().any(|receipt| matches!(receipt,
                            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                                slot, original, ..
                            } if slot == source_slot
                                && original.observed_dynamic_cursor_row() == Some(cursor))),
                            "the actual completed row208 generation receives its source epilogue"
                        );
                        assert!(
                            completed_model32_history
                                .insert(208_u16, (cursor, selector.registry_key().clone()))
                                .is_none(),
                            "the one-shot source occurrence consumes group18 exactly once"
                        );
                    }
                }
                for source in session
                    .economy()
                    .authored_model32()
                    .iter()
                    .filter(|source| source.active && matches!(source.authored_index, 208 | 213))
                {
                    let generation = session
                        .economy()
                        .slot_generation(source.source_entity_index);
                    if let Some((previous, previous_generation)) =
                        model32_history.get(&source.authored_index)
                    {
                        assert_eq!(previous.source_entity_index, source.source_entity_index);
                        assert_eq!(
                            *previous_generation, generation,
                            "one-shot model32 history remains on its actual source occurrence"
                        );
                    }
                    model32_history.insert(source.authored_index, (*source, generation));
                }
                for event in session.dispatch_events() {
                    if let Mc2DispatchEvent::AuthoredGroupActivated {
                        group,
                        authored_indices,
                        ..
                    } = event
                        && (9..=18).contains(group)
                    {
                        trace.extend(
                            authored_indices
                                .iter()
                                .copied()
                                .map(|authored_index| (*group, authored_index)),
                        );
                    }
                }
            }
        };

        // Each reset model-0 source row owns its own overlap callback. A live
        // producer is not activation: drive every package row whose group is
        // part of this band, including the even model-76 groups.
        for target_group in [9_u16, 11, 12, 13, 14, 15, 16] {
            let root = session
                .level2_imported_switch_roots
                .iter()
                .find(|root| root.target_group == target_group)
                .copied()
                .expect("band-B imported switch root");
            step_at(
                session,
                root.position,
                level_two_switch_turns_until_cadence(root.phase),
                &mut trace,
            );
        }

        // Slot 25 is remc2's dedicated group-10 model-76 switch. It uses
        // the same public player-position input but retains its distinct
        // source phase and transactional allocator.
        let switch25 = session
            .level2_group10_switch25
            .expect("live group-10 switch slot 25");
        step_at(
            session,
            switch25.position,
            level_two_model76_switch25_activation_turns(switch25.phase),
            &mut trace,
        );

        // Row 11's real group-1 callback materializes row 206. The two
        // source objective completion sentinels then let the dynamic
        // source cursor consume row 206 -> 208 -> 213 in order; no group
        // activator or authored constructor is called by this proof.
        session
            .runtime
            .set_level2_objective_stage_selector_for(0, 1, 2);
        session
            .runtime
            .set_level2_objective_stage_selector_for(0, 2, 2);
        let row11 = session
            .level2_imported_switch_roots
            .iter()
            .find(|root| root.authored_index == 11)
            .copied()
            .expect("row-11 group-1 production root");
        step_at(
            session,
            row11.position,
            level_two_switch_turns_until_cadence(row11.phase),
            &mut trace,
        );
        // Earlier free-slot reclamation can place the newly constructed
        // row-208 producer below the current numeric cursor. One following
        // source pass is then required before it can consume group 18.
        step_at(session, row11.position, 1, &mut trace);
        (
            trace,
            model32_history,
            completed_model32_history,
            source_trace,
        )
    };

    let (first_trace, first_model32_history, first_completed_history, first_source_trace) =
        drive_band(&mut session);
    let first_final_hash = session.campaign_state_hash();
    let plan = session
        .mission()
        .level2_activation_plan()
        .expect("artifact-derived activation plan");
    assert_eq!((plan.owned_rows().len(), plan.edges().len()), (151, 18));
    let mut expected = (9_u16..=18)
        .flat_map(|group| {
            plan.group_rows(group)
                .expect("band-B artifact group")
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = first_trace
        .iter()
        .map(|(_, authored_index)| *authored_index)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, expected, "all 13 band-B rows activate once");
    let counts = first_trace.iter().fold(
        BTreeMap::<u16, usize>::new(),
        |mut counts, (_, authored_index)| {
            *counts.entry(*authored_index).or_default() += 1;
            counts
        },
    );
    assert_eq!(counts.len(), 13);
    assert!(counts.values().all(|count| *count == 1));
    assert_eq!(
        session
            .economy()
            .authored_model22()
            .iter()
            .map(|source| source.authored_index)
            .filter(|index| matches!(index, 110 | 114 | 118 | 122))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([110, 114, 118, 122])
    );
    assert_eq!(
        session
            .economy()
            .authored_model76()
            .iter()
            .map(|source| source.authored_index)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([112, 116, 120, 124])
    );
    assert_eq!(
        first_model32_history
            .keys()
            .copied()
            .chain(first_completed_history.keys().copied())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([208, 213])
    );
    assert_eq!(
        first_completed_history
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([208])
    );
    for (source, generation) in first_model32_history.values() {
        assert_eq!(
            (source.source_class, source.model, source.action),
            (11, 32, 32)
        );
        assert!(*generation > 0);
    }
    let (completed208, _) = first_completed_history
        .get(&208)
        .expect("actual row208 generation-qualified source callback was observed");
    assert!(
        session.level2_model32_activated.contains(&208),
        "the real successful callback records its one-shot completion"
    );
    assert!(
        first_source_trace.iter().any(|event| matches!(event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 18, source_slot, authored_indices, .. }
            if *source_slot == completed208.source_slot && authored_indices.contains(&213))),
        "the observed row208 occurrence must consume its actual group18/row213 child"
    );
    // Its successful one-shot callback hides row208; the following source
    // prologue reclaims it while the not-yet-completed row213 stays live.
    assert_eq!(
        session
            .economy()
            .authored_model32()
            .iter()
            .map(|source| source.authored_index)
            .filter(|index| matches!(index, 208 | 213))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([213])
    );
    assert_eq!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .filter_map(BeeCreatureRuntime::authored_index)
            .filter(|index| matches!(index, 236 | 237 | 257))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([236, 237, 257])
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    let (replay_trace, replay_model32_history, replay_completed_history, replay_source_trace) =
        drive_band(&mut session);
    assert_eq!(replay_trace, first_trace, "reset preserves source order");
    assert_eq!(
        replay_model32_history, first_model32_history,
        "reset preserves actual construction, generation and hide history"
    );
    assert_eq!(
        replay_completed_history, first_completed_history,
        "reset preserves the completed source occurrence, generation and selector"
    );
    assert_eq!(
        replay_source_trace, first_source_trace,
        "reset preserves every actual source dispatch receipt"
    );
    assert_eq!(session.campaign_state_hash(), first_final_hash);

    let mut tampered_member = session.mission().authored_features().to_vec();
    tampered_member
        .iter_mut()
        .find(|feature| feature.index == 110)
        .expect("group-9 member row 110")
        .activation_group = 8;
    assert!(checked_in_level2_activation_plan(&tampered_member).is_err());
    let mut tampered_edge = session.mission().authored_features().to_vec();
    tampered_edge
        .iter_mut()
        .find(|feature| feature.index == 109)
        .expect("group-9 producer row 109")
        .stage_tag = 8;
    assert!(checked_in_level2_activation_plan(&tampered_edge).is_err());
}

#[test]
fn level_two_group1_row41_materializes_latent_from_row11_production_route() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-1 feature")
    };

    let local_player = session.state.entity_index;
    let row11_position = feature(11).world_position(0);
    session
        .set_level2_entity_position(local_player, row11_position)
        .expect("public player-position input for row 11");
    session.step_outer_turn(PlayerCommand::default(), 0);

    let expected_group1_indices = (12_u16..=41)
        .chain([167, 168, 169, 206])
        .collect::<Vec<_>>();
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 1,
            authored_indices,
            ..
        } if authored_indices.as_ref() == expected_group1_indices.as_slice()
    )));

    let row41 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 41)
        .copied()
        .expect("group 1 must materialize row 41");
    let row16_source_slot = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 16)
        .map(|field| field.source_entity_index)
        .expect("group 1 must materialize row 16");
    let row16_dynamic_seen = session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaAttraction,
                ..
            } if *source_slot == row16_source_slot
        )
    });
    if !row41.fired {
        let mut row41_exit_position = row41.position;
        row41_exit_position.x = row41_exit_position.x.wrapping_add(0x8000);
        for _ in 0..level_two_switch_turns_until_cadence(row41.phase) {
            session
                .set_level2_entity_position(local_player, row41_exit_position)
                .expect("public player-position input leaves the row-41 inverse-overlap extent");
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
    }
    let row41 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 41)
        .copied()
        .expect("group 1 row 41 remains live");
    assert_eq!(
        (
            row41.source_class,
            row41.model,
            row41.action,
            row41.stage_tag,
            row41.word_10,
            row41.dword_0x10,
        ),
        (11, 1, 1, 2, 34, 0)
    );
    let authored_row41 = feature(41);
    assert_eq!(
        (row41.position.x, row41.position.y, row41.position.z),
        (
            authored_row41.world_position(0).x,
            authored_row41.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(authored_row41.world_position(0))
        )
    );
    assert!(row41.active);
    // Model-1 uses the inverse-overlap callback. Row 11 is only one tile
    // from row 41, inside its authored 34-cell extent; leave that extent
    // while the live row-41 producer reaches its source cadence.
    assert!(row41.fired);
    assert_eq!(row41.cooldown, 0);
    assert!(!row41.drawing_enabled);

    // Row 16 is the same source group but a class-10/model-54 field. The
    // source `sub_4A1E0` scan consumes every matching authored row in one
    // numeric-order batch, irrespective of which constructor owns it.
    let row16 = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 16)
        .copied()
        .expect("group 1 must reach authored row 16 model-54 field");
    assert_eq!(row16.action, MANA_ATTRACTION_ACTION);
    assert_eq!(row16.radius_squared, (33_u32 * 256).pow(2));
    assert_eq!(row16.max_life, 280);
    let authored_row16 = feature(16);
    assert_eq!(
        (row16.position.x, row16.position.y, row16.position.z),
        (
            authored_row16.world_position(0).x,
            authored_row16.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(authored_row16.world_position(0))
        )
    );
    assert_eq!(row16.source_entity_index, row16_source_slot);
    assert!(row16_dynamic_seen);

    let class5_rows = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| matches!(source.authored_index, 167 | 168 | 169))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        class5_rows
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        vec![167, 168, 169]
    );
    assert!(class5_rows.iter().all(|source| {
        source.model == 3
            && source.action == 25
            && source.minimum_speed == 64
            && source.maximum_speed == 16
            && source.actual_speed == 30
            && source.max_life == 9_000
            && source.life == 9_000
            && source.mana == 2_250
            && source.max_mana == 4_500
            && source.drawing_enabled
            && source.active
    }));
    for source in &class5_rows {
        let authored = feature(source.authored_index);
        // Model-3 turns after each accepted source move, so replaying
        // every phase with the final yaw is not a valid oracle. The
        // dedicated action-family test covers exact turn/RNG vectors;
        // this production route proves source displacement.
        assert!(source.phase > 0);
        assert_ne!(
            (source.position.x, source.position.y),
            (authored.world_position(0).x, authored.world_position(0).y)
        );
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredClass5,
                ..
            } if *source_slot == source.source_entity_index
        )));
        assert_eq!(
            session
                .economy()
                .source_rng
                .get(&source.source_entity_index),
            Some(&source.entity_rng)
        );
    }

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().mana_attraction_fields().is_empty());
    assert!(session.economy().authored_class5().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group6_row84_materializes_from_row83_production_route() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-6 feature")
    };

    // Follow the real source walk into row 83: row 11 -> row 41 -> row
    // 42 -> row 64 -> row 65 -> row 83. Enter each model-0 source and
    // leave each model-1 source's authored inverse-overlap extent.
    let local_player = session.state.entity_index;
    level_two_reach_row42(
        &mut session,
        local_player,
        feature(11).world_position(0),
        feature(42).world_position(0),
    );
    let row42_phase = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 42)
        .map(|source| source.phase)
        .expect("row 42 producer remains live before its cadence");
    for _ in 0..level_two_switch_turns_until_cadence(row42_phase) {
        session
            .set_level2_entity_position(local_player, feature(42).world_position(0))
            .expect("public player-position input remains on row 42");
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let mut group6_activated = false;
    for source_index in [64_u16, 65, 83] {
        let source = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == source_index)
            .copied()
            .expect("group-6 producer remains live before its cadence");
        let mut input_position = source.position;
        if source.model == 1 {
            input_position.x = input_position.x.wrapping_add(0x8000);
        }
        for _ in 0..level_two_switch_turns_until_cadence(source.phase) {
            session
                .set_level2_entity_position(local_player, input_position)
                .expect(
                    "public player-position input satisfies the current source overlap predicate",
                );
            session.step_outer_turn(PlayerCommand::default(), 0);
            group6_activated |= session.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 6,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == [84]
                )
            });
        }
    }
    assert!(
        group6_activated,
        "row 83 must activate group 6 in production"
    );
    let row83 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 83)
        .copied()
        .expect("group 5 must materialize row 83");
    assert!(row83.fired);
    assert!(!row83.drawing_enabled);

    let row84 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 84)
        .copied()
        .expect("group 6 must materialize row 84");
    // Leaving row 83's wide inverse-overlap extent puts the player outside
    // row 84. Enter its actual overlap before asserting downstream hide.
    let mut group7_activated = false;
    for _ in 0..level_two_switch_turns_until_cadence(row84.phase) {
        session
            .set_level2_entity_position(local_player, row84.position)
            .expect("public player-position input overlaps the new row-84 producer");
        session.step_outer_turn(PlayerCommand::default(), 0);
        group7_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 7, .. }
            )
        });
    }
    assert!(
        group7_activated,
        "row 84 must consume group 7 on its source cadence"
    );
    let row84 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 84)
        .copied()
        .expect("row 84 remains addressable through its callback pass");
    assert_eq!(
        (
            row84.source_class,
            row84.model,
            row84.action,
            row84.stage_tag,
            row84.word_10,
            row84.dword_0x10,
        ),
        (11, 0, 0, 7, 16, 0)
    );
    let authored_row84 = feature(84);
    assert_eq!(
        (row84.position.x, row84.position.y, row84.position.z),
        (
            authored_row84.world_position(0).x,
            authored_row84.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(authored_row84.world_position(0))
        )
    );
    assert!(row84.active);
    // The ordinary overlap visit consumes the downstream group once and
    // hides the source drawing bit after the synchronous scan.
    assert!(!row84.drawing_enabled);
    assert!(row84.fired);

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group3_model39_rows_materialize_from_row42_production_route() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let baseline_stages = session.runtime().stages.to_vec();
    let baseline_sphere_count = session.economy().spheres().len();
    let local_player = session.state.entity_index;
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-3 feature")
            .world_position(0)
    };
    let expected_group3_indices = (43_u16..=64)
        .chain([108])
        .chain(125..=151)
        .chain([439])
        .collect::<Vec<_>>();

    // The real row-11 root first creates row 42; putting the local player
    // on that model-0 switch then selects group 3 through ordinary source
    // turns. No direct allocator or narrative helper is used.
    level_two_reach_row42(
        &mut session,
        local_player,
        feature_position(11),
        feature_position(42),
    );
    let mut group3_activated = false;
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        group3_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 3,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == expected_group3_indices.as_slice()
            )
        });
        if group3_activated {
            break;
        }
    }
    assert!(
        group3_activated,
        "row 42 must activate group 3 in production"
    );

    // Row 63 is the class-10/model-54 temporary field in the same
    // authored group.  The source `sub_4A310` scan dispatches it through
    // the shared model-54 constructor rather than the model-39 sphere
    // list, so prove that the secondary consumer is materialized by the
    // same production route and carries the authored stage radius/life.
    let row63 = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 63)
        .copied()
        .expect("group 3 must materialize authored model-54 row 63");
    assert_eq!(row63.action, MANA_ATTRACTION_ACTION);
    assert_eq!(row63.radius_squared, (45_u32 * 256).pow(2));
    assert_eq!(row63.max_life, 376);
    assert!(row63.active);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaAttraction,
                ..
            } if *source_slot == row63.source_entity_index
        )
    }));

    let expected_positions = (43_u16..=62)
        .map(feature_position)
        .map(|position| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    let mut actual_position_counts = BTreeMap::new();
    for sphere in session.economy().spheres().iter().filter(|sphere| {
        sphere.active
            && sphere.mana == 512
            && expected_positions.contains(&(sphere.position.x, sphere.position.y))
    }) {
        *actual_position_counts
            .entry((sphere.position.x, sphere.position.y))
            .or_insert(0usize) += 1;
    }
    let expected_position_counts = expected_positions
        .iter()
        .copied()
        .map(|position| (position, 1usize))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_position_counts, expected_position_counts);

    let row64 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 64)
        .copied()
        .expect("group 3 must materialize row 64");
    assert_eq!(
        (row64.model, row64.action, row64.stage_tag, row64.word_10),
        (1, 1, 4, 47)
    );
    assert!(!row64.fired);
    assert!(row64.drawing_enabled);

    let row439 = session
        .economy()
        .authored_model53()
        .iter()
        .find(|source| source.authored_index == 439)
        .copied()
        .expect("group 3 must materialize authored subtype-58 row 439");
    assert_eq!(
        (
            row439.source_class,
            row439.model,
            row439.action,
            row439.max_life,
            row439.life,
            row439.subspell_index,
            row439.control_flags,
        ),
        (10, 53, 58, 6, 5, 3000, 0x0001_0002)
    );
    assert_eq!(
        (
            row439.actual_speed,
            row439.source_byte_39,
            row439.byte_0x43,
            row439.byte_0x44,
            row439.byte_0x46,
            row439.sprite_index,
            row439.last_sound_id,
            row439.water_spawn_requested,
        ),
        (16, -6, 10, 0, 0, 0, 30, false)
    );
    assert_eq!(
        (row439.dword_0x10, row439.shift_rot_x, row439.shift_rot_y),
        (2, 0, 512)
    );
    let authored_row439 = feature_position(439);
    assert_eq!(
        (row439.position.x, row439.position.y, row439.position.z),
        (
            authored_row439.x,
            authored_row439.y,
            session.canonical_terrain().ground_height(authored_row439)
        )
    );
    assert!(row439.active);
    assert!(row439.drawing_enabled);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredModel53,
                ..
            } if *source_slot == row439.source_entity_index
        )
    }));
    for (before, after) in baseline_stages.iter().zip(session.runtime().stages.iter()) {
        if matches!(before.kind, 1 | 2 | 4 | 6)
            && before.relation == RuntimeRelation::AuthoredFeature(439)
        {
            let expected_relation = if before.kind == 6 {
                RuntimeRelation::LiveEntityIndex(row439.source_entity_index)
            } else {
                RuntimeRelation::LiveEntity(row439.source_entity_index)
            };
            assert_eq!(after.relation, expected_relation);
            assert_eq!(after.flags, before.flags | 1);
        } else {
            assert_eq!(after, before);
        }
    }

    session.reset();
    assert_eq!(session.economy().spheres().len(), baseline_sphere_count);
    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().authored_model53().is_empty());
    assert_eq!(
        session.runtime().stages.as_ref(),
        baseline_stages.as_slice()
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn model53_sub58da0_stage_binding_preserves_source_relation_encodings() {
    let bytes = fs::read(level_two_fixture_path().join("runtime.bin"))
        .expect("qualified Level 2 runtime snapshot");
    let mut runtime = parse_runtime(&bytes).expect("qualified runtime snapshot");
    for (stage_index, kind) in [1_i8, 2, 4, 6].into_iter().enumerate() {
        let stage = &mut runtime.stages[stage_index];
        stage.kind = kind;
        stage.flags = 0;
        stage.relation = RuntimeRelation::AuthoredFeature(439);
    }

    assert_eq!(runtime.bind_authored_stage_entity(439, 77), 4);
    for (stage_index, kind) in [1_i8, 2, 4, 6].into_iter().enumerate() {
        let stage = &runtime.stages[stage_index];
        let expected_relation = if kind == 6 {
            RuntimeRelation::LiveEntityIndex(77)
        } else {
            RuntimeRelation::LiveEntity(77)
        };
        assert_eq!(stage.relation, expected_relation);
        assert_eq!(stage.flags, 1);
        let offset = 192 + stage_index * 14;
        assert_eq!(runtime.bytes()[offset + 1], 1);
        assert_eq!(runtime.bytes()[offset + 6], if kind == 6 { 3 } else { 2 });
        let encoded_value = u32::from_le_bytes(
            runtime.bytes()[offset + 10..offset + 14]
                .try_into()
                .expect("runtime relation width"),
        );
        assert_eq!(encoded_value, if kind == 6 { 77 } else { 78 });
    }
}

#[test]
fn level_two_authored_switch_chain_reaches_group5_consumers_from_production_turns() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let baseline_sphere_count = session.economy().spheres().len();
    let baseline_owner_mana_global = session.economy().owner_mana_global();
    let expected_group1_model39_indices = (12..=15).chain(17..=40).collect::<Vec<_>>();
    let expected_group7_event_indices = (85..=105).chain([165]).collect::<Vec<_>>();
    let baseline_root_phase = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 11)
        .expect("imported row-11 root baseline")
        .phase;
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored switch row")
            .world_position(0)
    };
    let local_player = session.state.entity_index;
    let row11_position = feature_position(11);

    assert!(session.economy().authored_switch_producers().is_empty());

    // Frame zero already imports row 11 as source slot 23. Put the local
    // player on that switch so its normal callback fires, then let its
    // non-overlap successor row 41 fire through the same runagain walk.
    session
        .set_level2_entity_position(local_player, row11_position)
        .expect("public player-position input for row 11");
    let owner_mana_global_before_group1 = session.economy().owner_mana_global();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let expected_group1_indices = (12..=41).chain([167, 168, 169, 206]).collect::<Vec<_>>();
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            source_slot,
            group,
            authored_indices,
            ..
        } if *source_slot == 23
            && *group == 1
            && authored_indices.as_ref() == expected_group1_indices.as_slice()
    )));
    assert_eq!(
        session.economy().owner_mana_global(),
        owner_mana_global_before_group1,
        "group 1 spheres cannot publish sub_60F00's global counter in their allocation pass"
    );
    let expected_group1_positions = expected_group1_model39_indices
        .iter()
        .copied()
        .map(feature_position)
        .collect::<Vec<_>>();
    let expected_group1_position_set = expected_group1_positions
        .iter()
        .map(|position| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    let mut actual_group1_position_counts = BTreeMap::new();
    for sphere in session.economy().spheres().iter().filter(|sphere| {
        expected_group1_position_set.contains(&(sphere.position.x, sphere.position.y))
    }) {
        *actual_group1_position_counts
            .entry((sphere.position.x, sphere.position.y))
            .or_insert(0usize) += 1;
        assert!(sphere.active);
        assert_eq!(sphere.mana, 512);
        assert_eq!(
            (sphere.owner_entity_index, sphere.player_entity_index),
            (0, 0)
        );
    }
    let expected_group1_position_counts = expected_group1_position_set
        .iter()
        .copied()
        .map(|position| (position, 1usize))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual_group1_position_counts, expected_group1_position_counts,
        "group 1 materializes one sphere for every model-39 row 12..15,17..40 position"
    );
    let collectable_group1_mana = session
        .economy()
        .spheres()
        .iter()
        .filter(|sphere| {
            sphere.active
                && !sphere.pending_delete
                && sphere.control_flags & 0x2000 == 0
                && expected_group1_position_set.contains(&(sphere.position.x, sphere.position.y))
        })
        .map(|sphere| sphere.mana)
        .sum::<i32>();
    assert_eq!(
        collectable_group1_mana,
        28 * 512,
        "the package's exact neutral group-1 population supplies 14,336 collectable mana, exceeding Beyond Sight tier one's 10,000-Castle-mana predicate"
    );
    let imported_root = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 11)
        .expect("imported row-11 root");
    assert_eq!(imported_root.source_slot, 23);
    assert_eq!(imported_root.phase, baseline_root_phase.wrapping_add(1));
    assert_eq!(
        imported_root.cooldown, 0,
        "the compatibility projection must not invent a post-fire cooldown"
    );
    let imported_entity = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == 23)
        .expect("imported row-11 entity");
    assert_eq!(imported_entity.phase, baseline_root_phase.wrapping_add(1));
    assert_eq!(imported_entity.source_word_10(), 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session.economy().owner_mana_global() >= owner_mana_global_before_group1 + 28 * 512,
        "group 1 spheres publish at the next source boundary"
    );
    if !session
        .economy()
        .authored_switch_producers()
        .iter()
        .any(|source| source.authored_index == 42)
    {
        let row41_phase = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 41)
            .map(|source| source.phase)
            .expect("row 41 producer remains live before its cadence");
        for _ in 0..level_two_switch_turns_until_cadence(row41_phase) {
            let mut row41_exit_position = feature_position(42);
            row41_exit_position.x = row41_exit_position.x.wrapping_add(0x8000);
            session
                .set_level2_entity_position(local_player, row41_exit_position)
                .expect("public player-position input leaves the row-41 inverse-overlap switch");
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
    }
    let switch_indices = session
        .economy()
        .authored_switch_producers()
        .iter()
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert!(!switch_indices.contains(&11));
    assert!(!switch_indices.contains(&111));
    assert!(!switch_indices.contains(&342));
    assert!(switch_indices.contains(&41));
    let model32 = session
        .economy()
        .authored_model32()
        .iter()
        .find(|source| source.authored_index == 206)
        .copied()
        .expect("group-1 authored model-32 source");
    assert_eq!(
        (
            model32.source_class,
            model32.model,
            model32.action,
            model32.stage_tag,
            model32.objective_stage_index,
            model32.word_10,
            model32.max_life,
            model32.life,
            model32.control_flags,
        ),
        (11, 32, 32, 17, 1, 0, 300, 300, 1)
    );
    assert!(!model32.fired);
    assert!(model32.drawing_enabled);

    // Drive the ordinary row 42 -> row 64 -> row 65 chain. Drawing-disabled
    // one-shot producers are reclaimed at the following pass boundary, so
    // row 65—not the retired intermediates—is its durable continuation.
    let row65_position = feature_position(65);
    let mut row64_exit_position = feature_position(64);
    row64_exit_position.x = row64_exit_position.x.wrapping_add(0x8000);
    session
        .set_level2_entity_position(local_player, row65_position)
        .expect("public player-position input for row 65");
    let expected_group5_indices = (66..=83)
        .chain([164, 248, 249, 250, 251])
        .collect::<Vec<_>>();
    let mut group5_activated = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .any(|field| field.authored_index == 82);
    let mut group7_activated = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .any(|field| field.authored_index == 105);
    let mut mana_attraction_source_slots = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .map(|field| field.source_entity_index)
        .collect::<BTreeSet<_>>();
    let mut class5_source_slots = session
        .economy()
        .authored_class5()
        .iter()
        .map(|source| source.source_entity_index)
        .collect::<BTreeSet<_>>();
    let mut group5_sphere_spawn_ids = BTreeSet::new();
    let mut row105_group_activated = group7_activated;
    let classify_chain_events = |events: &[Mc2DispatchEvent]| {
        let mut group5_this_turn = false;
        let mut group7_this_turn = false;
        let mut row105_this_turn = false;
        let mut mana_attraction_slots = BTreeSet::new();
        let mut class5_slots = BTreeSet::new();
        let mut group5_spawn_ids = BTreeSet::new();
        let group5_key = events.iter().find_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated {
                pass,
                source_slot,
                group: 5,
                authored_indices,
            } if authored_indices.as_ref() == expected_group5_indices.as_slice() => {
                Some((*pass, *source_slot))
            }
            _ => None,
        });
        for event in events {
            match event {
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 5
                    && authored_indices.as_ref() == expected_group5_indices.as_slice() =>
                {
                    group5_this_turn = true;
                }
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 7
                    && authored_indices.as_ref() == expected_group7_event_indices.as_slice() =>
                {
                    group7_this_turn = true;
                    row105_this_turn = true;
                }
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::ManaAttraction,
                    ..
                } => {
                    mana_attraction_slots.insert(*source_slot);
                }
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::AuthoredClass5,
                    ..
                } => {
                    class5_slots.insert(*source_slot);
                }
                Mc2DispatchEvent::Economy {
                    pass,
                    source_slot,
                    event:
                        Level2EconomyEvent::ManaSphereSpawned {
                            sphere_id,
                            source_entity_index,
                            mana,
                        },
                } if group5_key == Some((*pass, *source_slot))
                    && *source_entity_index == *source_slot
                    && *mana == 512 =>
                {
                    group5_spawn_ids.insert(*sphere_id);
                }
                _ => {}
            }
        }
        (
            group5_this_turn,
            group7_this_turn,
            row105_this_turn,
            mana_attraction_slots,
            class5_slots,
            group5_spawn_ids,
        )
    };
    let record_chain_turn =
        |session: &Mc2Simulation,
         group5_activated: &mut bool,
         group7_activated: &mut bool,
         row105_group_activated: &mut bool,
         mana_attraction_source_slots: &mut BTreeSet<u16>,
         class5_source_slots: &mut BTreeSet<u16>,
         group5_sphere_spawn_ids: &mut BTreeSet<u32>| {
            let (
                group5_this_turn,
                group7_this_turn,
                row105_this_turn,
                mana_attraction_slots,
                class5_slots,
                group5_spawn_ids,
            ) = classify_chain_events(session.dispatch_events());
            *group5_activated |= group5_this_turn;
            *group7_activated |= group7_this_turn;
            *row105_group_activated |= row105_this_turn;
            mana_attraction_source_slots.extend(mana_attraction_slots);
            class5_source_slots.extend(class5_slots);
            group5_sphere_spawn_ids.extend(group5_spawn_ids);
        };
    for source_index in [42_u16, 64, 65, 83, 84] {
        if (source_index == 65 && group5_activated)
            || (matches!(source_index, 83 | 84) && group7_activated)
        {
            continue;
        }
        let Some(source_phase) = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == source_index)
            .map(|source| source.phase)
        else {
            // A higher-slot one-shot producer may already have fired and
            // been reclaimed while the same package chain was advancing.
            continue;
        };
        for _ in 0..level_two_switch_turns_until_cadence(source_phase) {
            session
                .set_level2_entity_position(
                    local_player,
                    match source_index {
                        42 => feature_position(42),
                        64 => row64_exit_position,
                        83 => {
                            let mut row83_exit_position = feature_position(83);
                            row83_exit_position.x = row83_exit_position.x.wrapping_add(0x8000);
                            row83_exit_position
                        }
                        84 => feature_position(84),
                        _ => row65_position,
                    },
                )
                .expect("public player-position input advances the authored switch chain");
            session.step_outer_turn(PlayerCommand::default(), 0);
            record_chain_turn(
                &session,
                &mut group5_activated,
                &mut group7_activated,
                &mut row105_group_activated,
                &mut mana_attraction_source_slots,
                &mut class5_source_slots,
                &mut group5_sphere_spawn_ids,
            );
        }
    }
    // Firing row 65 selects authored group 5.  The bounded downstream
    // consumers are already production-owned: row 83 is the next switch,
    // row 82 is model-54, and rows 248..251 are class-14/model-5.
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .all(|source| source.authored_index != 65),
        "the one-shot row-65 producer is reclaimed on the following source boundary"
    );
    let row82 = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 82)
        .copied()
        .expect("group 5 must materialize authored model-54 row 82");
    assert_eq!(
        group5_sphere_spawn_ids.len(),
        16,
        "the group-5 producer publishes one same-pass economy receipt for every package model-39 row"
    );
    assert_eq!(row82.action, MANA_ATTRACTION_ACTION);
    assert_eq!(row82.radius_squared, (64_u32 * 256).pow(2));
    assert_eq!(row82.max_life, 528);
    assert!(row82.active);
    assert!(mana_attraction_source_slots.contains(&row82.source_entity_index));
    let class14_indices = session
        .economy()
        .authored_class14()
        .iter()
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert!(class14_indices.is_superset(&BTreeSet::from([248, 249, 250, 251])));
    let group5_class5 = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.authored_index == 164)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        group5_class5
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        vec![164]
    );
    for source in &group5_class5 {
        assert_eq!(source.model, 3);
        assert_eq!(source.action, 25);
        assert!(source.phase > 0);
        // NewEvent byte 0xc bit 8 makes the first action-25 visit a
        // source-prescribed no-move; the explicit second-visit
        // move-or-blocked assertion below covers the actual movement.
        assert!(source.active && source.drawing_enabled);
        assert!(class5_source_slots.contains(&source.source_entity_index));
        assert_eq!(
            session
                .economy()
                .source_rng
                .get(&source.source_entity_index),
            Some(&source.entity_rng)
        );
    }
    let group7_class5 = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.authored_index == 165)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        group7_class5
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        vec![165]
    );
    for source in &group7_class5 {
        assert_eq!(source.model, 3);
        assert_eq!(source.action, 25);
        assert!(source.phase > 0);
        assert!(source.active && source.drawing_enabled);
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredClass5,
                ..
            } if *source_slot == source.source_entity_index
        )));
    }

    // NewEvent's byte 0xc bit 8 suppresses the first action-25 movement
    // visit. The next source visit must therefore either move the model-3
    // root or expose the exact blocked-candidate terminal outcome.
    let group5_positions_before = group5_class5
        .iter()
        .map(|source| {
            (
                source.authored_index,
                source.source_entity_index,
                (source.position.x, source.position.y),
            )
        })
        .collect::<Vec<_>>();
    let group7_positions_before = group7_class5
        .iter()
        .map(|source| {
            (
                source.authored_index,
                source.source_entity_index,
                (source.position.x, source.position.y),
            )
        })
        .collect::<Vec<_>>();
    for source in group5_class5.iter().chain(group7_class5.iter()) {
        let state = session
            .economy()
            .authored_class5_model3()
            .iter()
            .find(|state| state.source_entity_index == source.source_entity_index)
            .expect("model-3 movement sidecar");
        assert_eq!(
            state.control_flags & 8,
            0,
            "the first action-25 visit must clear NewEvent byte 0xc bit 8"
        );
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    for (group_name, positions) in [
        ("group5", group5_positions_before.as_slice()),
        ("group7", group7_positions_before.as_slice()),
    ] {
        for (authored_index, source_slot, (before_x, before_y)) in positions {
            let source = session
                .economy()
                .authored_class5()
                .iter()
                .find(|source| source.source_entity_index == *source_slot)
                .copied()
                .expect("model-3 source after second action-25 visit");
            let state = session
                .economy()
                .authored_class5_model3()
                .iter()
                .find(|state| state.source_entity_index == *source_slot)
                .expect("model-3 sidecar after second action-25 visit");
            let xy_changed = (source.position.x, source.position.y) != (*before_x, *before_y);
            let candidate_blocked =
                !xy_changed && source.life < 0 && state.control_flags & 0x0004_0000 != 0;
            assert!(
                xy_changed || candidate_blocked,
                "{group_name} model-3 source {authored_index} must move or report the source blocked-candidate outcome: before=({}, {}), after=({}, {}), life={}, flags=0x{:08x}",
                before_x,
                before_y,
                source.position.x,
                source.position.y,
                source.life,
                state.control_flags
            );
        }
    }

    // The source runagain cursor reaches row 83 -> row 84 -> group 7 in
    // this same production chain, so no direct allocator call is needed.
    assert!(
        group7_activated,
        "row 84 must activate group 7 in production"
    );
    // The same rooted row-84 stage tag also reaches the authored
    // class-10/model-54 field at row 105 through the source
    // `sub_4A1E0` scan. Keep it as a separate activation event: it is
    // not one of the model-39 sphere rows in the group-7 constructor
    // batch. This proves the source-backed field is live on the real
    // row-11 -> row-65 -> row-84 route rather than only through the
    // direct allocation helper.
    let row105 = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 105)
        .copied()
        .expect("group 7 must reach authored row 105 model-54 field");
    let authored_row105 = feature_position(105);
    let row105_feature = authored_features
        .iter()
        .copied()
        .find(|feature| feature.index == 105)
        .expect("authored row 105 feature");
    assert_eq!(
        (
            row105_feature.kind,
            row105_feature.subtype,
            row105_feature.activation_group,
            row105_feature.stage_tag,
        ),
        (10, 54, 7, 31)
    );
    assert_eq!(row105.action, MANA_ATTRACTION_ACTION);
    assert_eq!(row105.radius_squared, (31_u32 * 256).pow(2));
    assert_eq!(row105.max_life, 264);
    assert_eq!(
        (row105.position.x, row105.position.y, row105.position.z),
        (
            authored_row105.x,
            authored_row105.y,
            session.canonical_terrain().ground_height(authored_row105)
        )
    );
    assert!(row105.active);
    assert!(row105_group_activated);
    assert!(mana_attraction_source_slots.contains(&row105.source_entity_index));
    assert!(
        group5_activated,
        "row 65 must activate group 5 in production"
    );
    assert!(
        session.economy().owner_mana_global() > baseline_owner_mana_global,
        "the ordinary group chain must publish its live mana consumers"
    );

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().bee_creatures().is_empty());
    assert!(session.economy().mana_attraction_fields().is_empty());
    assert!(session.economy().authored_class5().is_empty());
    assert!(session.economy().authored_class14().is_empty());
    assert_eq!(session.economy().spheres().len(), baseline_sphere_count);
    assert_eq!(
        session.economy().owner_mana_global(),
        baseline_owner_mana_global
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_model32_switch_reaches_group17_bees_from_production_turns() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored model-32 chain row")
            .world_position(0)
    };

    // Row 206 is materialized by the real imported row-11 overlap path.
    // Set only its source objective selector to the remc2 completion
    // sentinel; the model-32 callback must then scan that selector and
    // consume group 17, rather than relying on the dedicated group-8
    // root or a direct allocator call.
    session.state.position = feature_position(11);
    session.runtime.objectives[0].stage_selectors[1] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);

    let row206 = session
        .economy()
        .authored_model32()
        .iter()
        .find(|source| source.authored_index == 206)
        .copied()
        .expect("row-206 model-32 source");
    assert!(row206.fired);
    assert!(!row206.drawing_enabled);
    assert_eq!(session.runtime.objectives[0].done, 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            source_slot,
            group: 17,
            authored_indices,
            ..
        } if *source_slot == row206.source_entity_index
            && authored_indices.as_ref() == [208, 236, 237, 257]
    )));

    let row208 = session
        .economy()
        .authored_model32()
        .iter()
        .find(|source| source.authored_index == 208)
        .copied()
        .expect("downstream row-208 model-32 source");
    assert_eq!((row208.stage_tag, row208.objective_stage_index), (18, 2));
    assert!(!row208.fired);
    let bee_indices = session
        .economy()
        .bee_creatures()
        .iter()
        .filter(|source| matches!(source.authored_index(), Some(236 | 237 | 257)))
        .filter_map(BeeCreatureRuntime::authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(bee_indices, BTreeSet::from([236, 237, 257]));

    session.reset();
    assert!(session.economy().authored_model32().is_empty());
    assert!(
        session
            .economy()
            .bee_creatures()
            .iter()
            .all(|source| !matches!(source.authored_index(), Some(236 | 237 | 257)))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group18_row213_materializes_from_model32_production_route() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-18 feature")
            .world_position(0)
    };

    // Select only the first two objective stages. The real source walk
    // then consumes row 206 -> group 17 -> row 208 -> group 18, while
    // leaving row 213 latent for the next objective stage.
    session.state.position = feature_position(11);
    session.runtime.objectives[0].stage_selectors[1] = 2;
    session.runtime.objectives[0].stage_selectors[2] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);

    let row213 = session
        .economy()
        .authored_model32()
        .iter()
        .find(|source| source.authored_index == 213)
        .copied()
        .expect("group 18 must materialize row 213");
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 18,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [213]
    )));
    assert_eq!(
        (
            row213.source_class,
            row213.model,
            row213.action,
            row213.stage_tag,
            row213.objective_stage_index,
            row213.word_10,
            row213.max_life,
            row213.life,
            row213.control_flags,
        ),
        (11, 32, 32, 19, 3, 0, 300, 300, 1)
    );
    let authored_row213 = authored_features
        .iter()
        .copied()
        .find(|feature| feature.index == 213)
        .expect("authored row 213");
    assert_eq!(
        (row213.position.x, row213.position.y, row213.position.z),
        (
            authored_row213.world_position(0).x,
            authored_row213.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(authored_row213.world_position(0))
        )
    );
    assert!(!row213.fired);
    assert!(row213.drawing_enabled);
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .all(|source| source.authored_index != 214)
    );

    session.reset();
    assert!(
        session
            .economy()
            .authored_model32()
            .iter()
            .all(|source| source.authored_index != 213)
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_model32_chain_continues_through_group18_and_group19() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored model-32 continuation row")
            .world_position(0)
    };

    // A single real outer turn can visit newly allocated rows through the
    // dynamic run-again cursor. Selectors 1 and 2 therefore exercise the
    // source row-206 -> group-17 -> row-208 -> group-18 edge without
    // directly invoking an allocator or narrative helper.
    session.state.position = feature_position(11);
    session.runtime.objectives[0].stage_selectors[1] = 2;
    session.runtime.objectives[0].stage_selectors[2] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);

    let row208 = session
        .economy()
        .authored_model32()
        .iter()
        .find(|source| source.authored_index == 208)
        .copied()
        .expect("group-17 row-208 model-32 source");
    assert!(row208.fired);
    assert!(!row208.drawing_enabled);
    let row213 = session
        .economy()
        .authored_model32()
        .iter()
        .find(|source| source.authored_index == 213)
        .copied()
        .expect("group-18 row-213 model-32 source");
    assert_eq!((row213.stage_tag, row213.objective_stage_index), (19, 3));
    assert!(!row213.fired);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 18,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [213]
    )));

    // The next source objective edge is independent: selecting stage 3
    // on the following turn consumes row 213 and materializes the authored
    // group-19 model-0 switch, while its overlap callback remains latent.
    session.runtime.objectives[0].stage_selectors[3] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            group: 19,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [214, 215, 224, 225, 226, 227]
    )));
    let row214 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 214)
        .copied()
        .expect("group-19 row-214 switch source");
    assert_eq!((row214.model, row214.stage_tag), (0, 20));
    assert!(row214.drawing_enabled);
    let stone_position = feature_position(215);
    let stone = session
        .economy
        .entities()
        .iter()
        .find(|entity| {
            entity.active
                && (entity.class, entity.model, entity.action) == (2, 1, 3)
                && (entity.position.x, entity.position.y) == (stone_position.x, stone_position.y)
        })
        .expect("group-19 row-215 stone source");
    assert_eq!(
        stone.position.z,
        session.canonical_terrain().ground_height(stone_position)
    );
    assert_eq!((stone.max_life, stone.life), (300, 300));
    assert_eq!(stone.actual_speed, 16);
    assert_eq!(stone.particle_parameter_index, Some(79));
    assert_eq!(stone.parameter_relation, Some(59));
    assert_eq!(
        stone.special_relation,
        EntitySpecialRelation::GlobalSpecialState
    );
    assert_eq!(stone.source_word_10(), stone.index % 11);
    // The newly allocated higher slot is visited later in this same numeric
    // pass: action 3 sets source byte-2 bit 1, then the scheduler advances the
    // common phase exactly once.
    assert_eq!(stone.control_flags, 0x0002_0004);
    assert_eq!(stone.phase, (stone.index as u8).wrapping_add(1));
    assert!(session.economy().map_chains().is_linked(stone.index));
    let checkpoint_indices = [224_u16, 225, 226, 227];
    assert!(checkpoint_indices.iter().all(|index| {
        session
            .economy()
            .authored_model59()
            .iter()
            .any(|source| source.authored_index == *index && source.active)
    }));
    for index in checkpoint_indices {
        let checkpoint = session
            .economy()
            .authored_model59()
            .iter()
            .find(|source| source.authored_index == index)
            .copied()
            .expect("group-19 checkpoint source");
        let position = feature_position(index);
        assert_eq!((checkpoint.model, checkpoint.action), (59, 64));
        assert_eq!(
            (checkpoint.position.x, checkpoint.position.y),
            (position.x, position.y)
        );
        assert_eq!(
            checkpoint.position.z,
            session.canonical_terrain().ground_height(position)
        );
        // The live source cursor revisits newly allocated model-59 rows
        // in the same runagain sweep, consuming the first action-64 life
        // tick before this production assertion observes them.
        assert_eq!(checkpoint.life, checkpoint.max_life - 1);
        assert!((800..=899).contains(&checkpoint.max_life));
        assert!((0..=16).contains(&checkpoint.actual_speed));
        assert_eq!(checkpoint.control_flags, 1);
    }
    assert!(
        !session
            .economy()
            .authored_model32()
            .iter()
            .find(|source| source.authored_index == 213)
            .expect("row-213 remains in the model-32 arena")
            .drawing_enabled
    );
    session.reset();
    assert!(session.economy().authored_model59().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group20_materializes_model1_and_archer_rows_from_row214() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2");
    // The production campaign entry supplies the semantic creature bindings
    // required by these authored births; the source payloads remain identical.
    for member in [
        "entities.bin",
        "authored-level.bin",
        "runtime.bin",
        "presentation.bin",
        "players.bin",
        "terrain.bin",
        "feature-end.bin",
        "collision-masks.bin",
    ] {
        assert!(
            std::fs::read(level_two_fixture_path().join(member)).unwrap()
                == std::fs::read(package.join(member)).unwrap(),
            "source member changed: {member}"
        );
    }
    let mut session = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("Level 2 semantic campaign opens")
    .simulation()
    .clone();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-20 feature")
            .world_position(0)
    };

    // Reuse the real objective chain: row 206 -> 208 -> 213 -> 214.
    // No allocator or narrative helper is called directly.
    session.state.position = feature_position(11);
    session.runtime.objectives[0].stage_selectors[1] = 2;
    session.runtime.objectives[0].stage_selectors[2] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let first_dispatch = session.dispatch_events().to_vec();
    session.runtime.objectives[0].stage_selectors[3] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 214),
        "row 214 missing: first={first_dispatch:?}; second={:?}; pose={:?}; objective={:?}",
        session.dispatch_events(),
        session.state.position,
        session.runtime.objectives[0]
    );

    // Row 214 is model-0, so the source callback requires the local
    // model-0 player to overlap its authored position.  The eight-turn
    // bound covers its phase/cooldown cadence without repeating a long
    // suite; we stop on the first exact activation event.
    session.state.position = feature_position(214);
    let expected = [
        216_u16, 217, 218, 219, 220, 221, 222, 223, 232, 233, 234, 235,
    ];
    let mut activated = false;
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 20,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == expected
            )
        }) {
            activated = true;
            break;
        }
    }
    assert!(activated, "row 214 must activate group 20 in production");

    let model1 = session
        .economy()
        .authored_model1()
        .iter()
        .find(|source| source.authored_index == 216)
        .copied()
        .expect("group-20 model-1 source");
    assert_eq!(
        (
            model1.model,
            model1.action,
            model1.subspell_index,
            model1.max_life,
            model1.particle_index,
            model1.control_flags,
        ),
        (1, 1, 400, 1, 41, 0x0002_0006)
    );
    // UpdateEntities_57730 scans live slots, including births ahead of its
    // cursor. AddQuickfair0A_01_30F60 has therefore already decremented life
    // and latched sound 3 at this end-of-turn observation (not constructor time).
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredModel1,
            ..
        } if *source_slot == model1.source_entity_index
    )));
    assert_eq!(model1.life, 0);
    assert_eq!(
        model1.phase,
        (model1.source_entity_index as u8).wrapping_add(1)
    );
    assert_eq!(model1.last_sound_id, 3);
    assert!(model1.active);
    let constructor_phase = model1.phase;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let model1 = session
        .economy()
        .authored_model1()
        .iter()
        .find(|source| source.authored_index == 216)
        .copied()
        .expect("group-20 model-1 source after its first callback");
    assert_eq!(model1.phase, constructor_phase.wrapping_add(1));
    assert_eq!(model1.control_flags, 0x0002_0006);
    assert_eq!(model1.last_sound_id, 3);
    let quickfair_children = session
        .economy()
        .model17_spawned_children()
        .iter()
        .filter(|child| child.parent_entity_index == model1.source_entity_index)
        .collect::<Vec<_>>();
    assert!(!quickfair_children.is_empty());
    // These children also lie ahead of the live cursor. sub_30D50 latches
    // their impact bit on the first visit; the following turn is visit two.
    assert!(quickfair_children.iter().all(|child| {
        (
            child.parent_entity_id,
            child.control_flags,
            child.shift_rot_x,
            child.shift_rot_y,
        ) == (model1.source_entity_index, 0x0002_0082, 128, 128)
            && child.life == 6
    }));

    let archer = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("group-20 archer source");
    assert_eq!((archer.model, archer.action), (4, 34));
    assert_eq!(
        (
            archer.minimum_speed,
            archer.maximum_speed,
            archer.actual_speed,
            archer.max_life,
            archer.mana,
            archer.subspell_index,
        ),
        (30, 0, 30, 1_000, 500, 500)
    );
    // AddArchers_4BA10 uses its constructor RNG for body angles;
    // SetEntityIndexAndRot(0)'s 125/66 collision dimensions are not yaw/pitch.
    // The live callbacks have already advanced RNG and may steer desired roll,
    // but retain the equal constructor body yaw/pitch on this short route.
    assert!((-1..=0x07fe).contains(&archer.yaw));
    assert_eq!(archer.yaw, archer.pitch);
    assert_eq!(
        (
            archer.dword_0x10,
            archer.particle_index,
            archer.fov,
            archer.shift_rot_x,
            archer.shift_rot_y,
        ),
        (0, 0, 0, 128, 256)
    );
    assert_ne!((archer.yaw, archer.pitch), (125, 66));
    assert_eq!(
        session
            .economy()
            .source_rng
            .get(&archer.source_entity_index),
        Some(&archer.entity_rng)
    );
    assert!(archer.active && archer.drawing_enabled);

    // The root uses remc2's `life-- < 0` callback guard: after the
    // activation turn's first visit and the second visit above, one more turn exhausts life
    // and hide drawing while retaining the allocated row.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let model1 = session
        .economy()
        .authored_model1()
        .iter()
        .find(|source| source.authored_index == 216)
        .copied()
        .expect("model-1 root remains source-active after drawing disable");
    assert_eq!(model1.life, -2);
    assert!(!model1.drawing_enabled);
    assert!(model1.active);

    // The constructor proof above is not enough: the live source cursor
    // must visit the authored class-5/model-4 row after group activation.
    // The ordinary outer turn is the production route; no direct arena
    // step is used here. Action 34 owns pitch-aware movement and terrain
    // projection. This turn is deliberately after the model-1 cadence
    // assertion so it cannot weaken that independent proof.
    let archer_before_cursor = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("group-20 archer remains after model-1 cadence");
    let archer_phase = archer_before_cursor.phase;
    let archer_position = archer_before_cursor.position;
    let archer_source_slot = archer_before_cursor.source_entity_index;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let stepped_archer = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("group-20 archer remains on the live source cursor");
    assert_eq!(stepped_archer.phase, archer_phase.wrapping_add(1));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredClass5,
                ..
            } if *source_slot == archer_source_slot
        )
    }));
    let mut expected_archer_position = archer_position;
    crate::move_entity(
        &mut expected_archer_position,
        archer_before_cursor.yaw,
        archer_before_cursor.pitch,
        archer_before_cursor.actual_speed,
    );
    expected_archer_position.z = session
        .canonical_terrain()
        .ground_height(expected_archer_position);
    assert_eq!(
        (
            stepped_archer.position.x,
            stepped_archer.position.y,
            stepped_archer.position.z,
        ),
        (
            expected_archer_position.x,
            expected_archer_position.y,
            expected_archer_position.z,
        )
    );
    assert!(stepped_archer.active && stepped_archer.drawing_enabled);

    // The Quickfair terminal guard is before its first branch draw. A
    // negative pre-decrement therefore hides the root without changing
    // its source RNG or allocating another model-0 child.
    let model1_before_terminal = session
        .economy()
        .authored_model1()
        .iter()
        .find(|source| source.authored_index == 216)
        .copied()
        .expect("model-1 root for terminal negative branch");
    let child_count_before_terminal = session.economy().model17_spawned_children().len();
    session
        .economy_mut()
        .authored_model1
        .iter_mut()
        .find(|source| source.source_entity_index == model1_before_terminal.source_entity_index)
        .expect("model-1 root for terminal mutation")
        .life = -1;
    assert!(
        session
            .economy_mut()
            .step_authored_model1_slot(model1_before_terminal.source_entity_index)
    );
    let model1_after_terminal = session
        .economy()
        .authored_model1()
        .iter()
        .find(|source| source.source_entity_index == model1_before_terminal.source_entity_index)
        .copied()
        .expect("model-1 root after terminal negative branch");
    assert_eq!(model1_after_terminal.life, -2);
    assert_eq!(
        model1_after_terminal.entity_rng,
        model1_before_terminal.entity_rng
    );
    assert_eq!(
        model1_after_terminal.phase,
        model1_before_terminal.phase.wrapping_add(1)
    );
    assert!(!model1_after_terminal.drawing_enabled);
    assert_eq!(
        session.economy().model17_spawned_children().len(),
        child_count_before_terminal
    );

    session.reset();
    assert!(session.economy().authored_model1().is_empty());
    assert!(
        session
            .economy()
            .authored_class5()
            .iter()
            .all(|source| !matches!(source.authored_index, 232 | 233 | 234 | 235))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_reset_model32_producer_reaches_group21_bees_from_production_turns() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    // The qualified reset contains a live class-11/model-32 producer at
    // source slot 76 with source id/group 21. The ordinary source pass
    // must consume that real producer and materialize the three authored
    // group-21 Bees without a direct allocator or fixture-only route.
    let producer_slot = fire_level_two_model32_group(&mut session, 21);

    let expected_bees = BTreeSet::from([229_u16, 230, 231]);
    let actual_bees = session
        .economy()
        .bee_creatures()
        .iter()
        .filter(|source| {
            source.active
                && source
                    .authored_index()
                    .is_some_and(|index| expected_bees.contains(&index))
        })
        .filter_map(BeeCreatureRuntime::authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_bees, expected_bees);
    let activated_from_producer = session
        .dispatch_events()
        .iter()
        .find_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 21,
                authored_indices,
                ..
            } if *source_slot == producer_slot => Some(authored_indices.to_vec()),
            _ => None,
        })
        .expect("group-21 activation receipt from the real producer");
    assert_eq!(activated_from_producer, vec![229, 230, 231]);

    session.reset();
    assert!(session.economy().bee_creatures().iter().all(|source| {
        source
            .authored_index()
            .is_none_or(|index| !expected_bees.contains(&index))
    }));
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_reset_model32_producer_reaches_group29_bees_from_production_turns() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    let producer_slot = fire_level_two_model32_group(&mut session, 29);

    let expected_bees = BTreeSet::from([307_u16, 308, 312]);
    let actual_bees = session
        .economy()
        .bee_creatures()
        .iter()
        .filter(|source| {
            source.active
                && source
                    .authored_index()
                    .is_some_and(|index| expected_bees.contains(&index))
        })
        .filter_map(BeeCreatureRuntime::authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_bees, expected_bees);
    let activated_from_producer = session
        .dispatch_events()
        .iter()
        .find_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 29,
                authored_indices,
                ..
            } if *source_slot == producer_slot => Some(authored_indices.to_vec()),
            _ => None,
        })
        .expect("group-29 activation receipt from the real producer");
    assert_eq!(activated_from_producer, vec![307, 308, 312]);

    session.reset();
    assert!(session.economy().bee_creatures().iter().all(|source| {
        source
            .authored_index()
            .is_none_or(|index| !expected_bees.contains(&index))
    }));
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group23_model59_materializes_from_rooted_production_turns() {
    // Nested group24 creates real GroundFire rows361/362 in the same pass.
    // Gameplay needs their immutable descriptor bindings; the raw export-only
    // inspection fixture intentionally does not supply those definitions.
    let mut session = level_two_campaign_session();
    for authored_record in [361, 362] {
        let definition = &session.session.definition;
        let id = definition.mechanic_entity_for_authored_record(authored_record)
            .expect("the production descriptor binds the actual GroundFire record");
        let effect = definition.semantic_level().unwrap().entity(id).unwrap().effect()
            .expect("the immutable authored record owns effect properties");
        assert_eq!(effect.kind(), crate::level_definition::EffectKind::GroundFire);
    }
    let baseline_hash = session.campaign_state_hash();
    let baseline_model59_count = session.economy().authored_model59().len();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-23 feature")
            .world_position(0)
    };
    let expected_group23_indices = [343_u16, 344];
    let root = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 342)
        .expect("imported row-342 group-23 root");
    assert_eq!(root.target_group, 23);
    let root_source_slot = root.source_slot;

    // This is the real imported root overlap path. No allocator or
    // authored-row helper is called directly; the source pass owns the
    // group activation and the same-pass dynamic cursor visit.
    let player_slot = session.state.entity_index;
    session.set_level2_entity_position(player_slot, feature_position(342)).unwrap();
    let mut activated = false;
    let mut activation_trace = Vec::new();
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let root = &session.entities()[usize::from(root_source_slot)];
        let player = &session.entities()[usize::from(player_slot)];
        let dx = i32::from((player.position.x as i16)
            .wrapping_sub(root.position.x as i16)).abs();
        let dy = i32::from((player.position.y as i16)
            .wrapping_sub(root.position.y as i16)).abs();
        let overlap = dx < i32::from(root.array_pitch()) + i32::from(player.array_pitch())
            && dy < i32::from(root.array_roll()) + i32::from(player.array_roll());
        activation_trace.push((
            (root.active, root.class, root.model, root.action, root.phase,
                root.position, root.control_flags, root.source_dword_10()),
            (player.active, player.class, player.model, player.life,
                player.position, session.state.position),
            (overlap, root.phase & 7),
        ));
        assert!(!session.dispatch_events().iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "root342 ordinary activation errors: {:?}; source trace: {activation_trace:?}",
            session.dispatch_events());
        activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot,
                    group,
                    authored_indices,
                    ..
                } if *source_slot == root_source_slot
                    && *group == 23
                    && authored_indices.as_ref() == expected_group23_indices.as_slice()
            )
        });
        if activated {
            break;
        }
    }
    assert!(activated, "row 342 must activate group 23 in production; source trace: {activation_trace:?}; events: {:?}", session.dispatch_events());

    let checkpoint = session
        .economy()
        .authored_model59()
        .iter()
        .find(|source| source.authored_index == 343)
        .copied()
        .expect("group 23 model-59 checkpoint source");
    assert_eq!(
        (checkpoint.source_class, checkpoint.model, checkpoint.action),
        (10, 59, 64)
    );
    let checkpoint_position = feature_position(343);
    assert_eq!(
        (checkpoint.position.x, checkpoint.position.y),
        (checkpoint_position.x, checkpoint_position.y)
    );
    assert_eq!(
        checkpoint.position.z,
        session
            .canonical_terrain()
            .ground_height(checkpoint_position)
    );
    // The newly allocated row is visited by the same source runagain
    // sweep, so its action-64 callback consumes one life tick.
    assert_eq!(checkpoint.life, checkpoint.max_life - 1);
    assert!((800..=899).contains(&checkpoint.max_life));
    assert!((0..=16).contains(&checkpoint.actual_speed));
    assert_eq!(checkpoint.control_flags, 1);
    assert!(
        !session
            .economy()
            .map_chains()
            .is_linked(checkpoint.source_entity_index),
        "ArriveCheckpoint writes a direct position and does not link the parent"
    );
    // The newly allocated row is visited by the same source runagain
    // sweep once, so the constructor seed has advanced by one callback.
    assert_eq!(
        checkpoint.source_byte_3e,
        (checkpoint.source_entity_index as u8).wrapping_add(1)
    );
    assert!(checkpoint.drawing_enabled);
    assert!(checkpoint.active);
    let smoke_child = session
        .economy()
        .model59_smoke_children()
        .first()
        .copied()
        .expect("checkpoint callback must allocate a model-13 smoke child");
    assert_eq!(
        (
            smoke_child.source_class,
            smoke_child.model,
            smoke_child.action,
            smoke_child.max_life,
            smoke_child.life,
            smoke_child.particle_index,
            smoke_child.control_flags,
        ),
        (10, 13, 13, 32, 31, 67, 0x0002_0004)
    );
    assert_eq!(
        smoke_child.phase,
        (smoke_child.source_entity_index as u8).wrapping_add(1)
    );
    assert!(
        session
            .economy()
            .map_chains()
            .is_linked(smoke_child.source_entity_index)
    );
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::Model59SmokeChild,
                ..
            } if *source_slot == smoke_child.source_entity_index
        )
    }));
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 344)
    );
    assert_eq!(session.economy().authored_model59().len(), 1);
    assert_eq!(session.economy().model59_smoke_children().len(), 1);

    // Retain the real group-produced state for the complete unshortened
    // lifecycle. Only the observer's position is controlled; no source life,
    // allocation list, activation gate or callback is forced.
    let mut lifetime = session.clone();
    let slot = checkpoint.source_entity_index;
    let handle = lifetime.source_arena().handle(slot).unwrap();
    let child_handle = lifetime.source_arena().handle(smoke_child.source_entity_index).unwrap();
    let mut saw_child_hidden = false;
    let mut saw_child_reclaimed = false;
    let mut saw_smoke_presentation = false;
    let mut terminal = None;
    for _ in 0..usize::try_from(checkpoint.max_life).unwrap() + 2 {
        assert!(!lifetime.dispatch_events().iter().any(|event|
            matches!(event, Mc2DispatchEvent::Error { .. })),
            "the unfiltered checkpoint source pass must complete: {:?}", lifetime.dispatch_events());
        let current = lifetime.economy().authored_model59().iter()
            .find(|source| source.source_entity_index == slot).copied()
            .expect("the real checkpoint remains owned until its following reclaim");
        assert!(lifetime.source_arena().resolve(handle).is_some());
        let packet = source_target::status_parent(lifetime.source_arena(), source_target::Context::default(), slot)
            .unwrap().unwrap();
        assert_eq!((packet.class, packet.model, packet.action, packet.life, packet.entity_rng,
            packet.actual_speed, packet.phase, packet.position, packet.source_id, packet.control_flags),
            (10, 59, 64, i32::from(current.life), current.entity_rng,
                current.actual_speed, current.phase, current.position, current.entity_id, current.control_flags));
        assert_eq!(current.source_byte_3e, current.phase);
        assert_eq!(current.phase, (slot as u8)
            .wrapping_add((i32::from(current.max_life) - i32::from(current.life)) as u8));
        let player = lifetime.state.entity_index;
        lifetime.set_level2_entity_position(player, Position {
            z: checkpoint.position.z.wrapping_add(768), ..checkpoint.position
        }).unwrap();
        let presentations = lifetime.campaign_entity_presentations(0).unwrap();
        assert!(!presentations.iter()
            .any(|view| view.entity_index == slot),
            "source flag1 keeps the checkpoint itself hidden");
        if lifetime.source_arena().resolve(child_handle).is_some() {
            let child = lifetime.economy().model59_smoke_children().iter()
                .find(|child| child.source_entity_index == child_handle.source_slot).unwrap();
            if child.control_flags & 0x400 != 0 {
                assert_eq!(child.life, -2);
                assert!(child.active && !child.drawing_enabled);
                assert!(!lifetime.economy().free_source_slots().contains(&child_handle.source_slot));
                saw_child_hidden = true;
            }
        } else {
            assert!(saw_child_hidden, "the child hides before its source incarnation is reclaimed");
            saw_child_reclaimed = true;
        }
        for child in lifetime.economy().model59_smoke_children().iter()
            .filter(|child| child.active && child.drawing_enabled)
        {
            if let Some(view) = presentations.iter()
                .find(|view| view.entity_index == child.source_entity_index)
            {
                assert_eq!(view.source_id, child.source_entity_index,
                    "32420 preserves the Smoke13 child's own NewEvent identity");
                assert_eq!(view.render_parameter_index, child.particle_index);
                assert!((67..=74).contains(&view.render_parameter_index));
                saw_smoke_presentation = true;
            }
        }
        if !current.drawing_enabled {
            assert_eq!((current.life, current.control_flags), (-2, 0x401));
            assert!(current.active);
            assert!(!lifetime.economy().free_source_slots().contains(&slot));
            terminal = Some(current);
            break;
        }
        lifetime.step_level2_source_pass(0);
        let after = lifetime.economy().authored_model59().iter()
            .find(|source| source.source_entity_index == slot).unwrap();
        assert_eq!(after.life, current.life - 1);
        if current.life < 0 {
            assert_eq!(after.entity_rng, current.entity_rng,
                "the terminal postdecrement creates no child and consumes no parent RNG");
        }
    }
    let terminal = terminal.expect("the original 800..899-life checkpoint naturally hides");
    assert!(saw_smoke_presentation && saw_child_hidden && saw_child_reclaimed);
    // Inspect the actual owning49F90 boundary before another ordinary producer
    // can spend this newly free address in the following scheduled pass.
    let mut boundary = lifetime.clone();
    {
        let live = &mut boundary.session.state.live;
        live.economy.rebuild_source_arena_with_terrain(&mut live.terrain, &[]).unwrap();
    }
    assert!(boundary.source_arena().resolve(handle).is_none());
    assert!(boundary.economy().free_source_slots().contains(&slot));
    let retained = source_target::status_parent(boundary.source_arena(), source_target::Context::default(), slot)
        .unwrap().unwrap();
    assert_eq!((retained.class, retained.model, retained.life, retained.entity_rng,
        retained.actual_speed, retained.phase, retained.position, retained.source_id, retained.control_flags),
        (0, 59, -2, terminal.entity_rng, terminal.actual_speed, terminal.phase,
            terminal.position, terminal.entity_id, 0x401));
    lifetime.step_level2_source_pass(0);
    assert!(lifetime.source_arena().resolve(handle).is_none(),
        "the following scheduled source pass reclaims the checkpoint incarnation");
    // The remaining naturally emitted children have at most32 life. No parent
    // survives to replenish them, and their own source reclaim ends each one.
    let remaining = lifetime.economy().model59_smoke_children().iter()
        .map(|child| lifetime.source_arena().handle(child.source_entity_index).unwrap())
        .collect::<Vec<_>>();
    for _ in 0..35 {
        lifetime.step_level2_source_pass(0);
    }
    assert!(remaining.into_iter().all(|handle| lifetime.source_arena().resolve(handle).is_none()));
    assert!(!lifetime.economy().authored_model59().iter()
        .any(|source| source.authored_index == 343));

    session.reset();
    assert_eq!(
        session.economy().authored_model59().len(),
        baseline_model59_count
    );
    assert!(session.economy().model59_smoke_children().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group26_model39_materializes_from_production_switch_chain() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let baseline_sphere_count = session.economy().spheres().len();
    let baseline_owner_mana_global = session.economy().owner_mana_global();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-26 feature")
            .world_position(0)
    };
    let expected_group25_indices = [356_u16];
    let expected_group26_sphere_indices = [357_u16, 358, 359, 360];
    let expected_group26_indices = authored_features
        .iter()
        .filter(|feature| feature.activation_group == 26)
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    assert_eq!(
        expected_group26_indices,
        [357_u16, 358, 359, 360, 365, 366, 367, 368],
        "the source group includes exactly four spheres and four model-13 rows"
    );

    // Rebind one real imported model-0 root to the authored group-25
    // source position. The callback, arena allocation, and downstream
    // group-26 switch still execute through ordinary outer turns.
    let root_position = feature_position(355);
    let root_slot = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 11 && root.model == 0)
        .map(|root| root.source_slot)
        .expect("imported model-0 root for production chain");
    session
        .set_level2_entity_position(root_slot, root_position)
        .expect("fixture root position updates the authoritative row and map link");
    // This component fixture rebinds the source root's group. The raw
    // class-11 row owns dispatch; its imported-root view is an epilogue projection.
    let root = &mut session.economy.entities[usize::from(root_slot)];
    root.source_id = 25;
    root.record[30..32].copy_from_slice(&25_u16.to_le_bytes());
    root.set_source_dword_10(0);
    session
        .set_level2_entity_position(session.state.entity_index, root_position)
        .expect("fixture player position updates the authoritative row and map link");

    let model13_indices = BTreeSet::from([365_u16, 366, 367, 368]);
    let mut model13_activated = BTreeSet::new();
    let mut group26_activated = false;
    let mut group26_owner_mana_global_at_boundary = None;
    let mut group26_owner_mana_global_after = None;
    let mut group25_activated = false;
    for _ in 0..16 {
        let (_, owner_mana_global_at_boundary) =
            level2_owner_mana_projection(&session.economy().owner_mana_rows);
        session.step_outer_turn(PlayerCommand::default(), 0);
        // Observe all source births now: a newly constructed later-slot
        // row 356 can consume group 26 in this same ordinary source pass.
        if let Some(authored_indices) =
            session
                .dispatch_events()
                .iter()
                .find_map(|event| match event {
                    Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 26,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref() == expected_group26_indices.as_slice() => {
                        Some(authored_indices.as_ref())
                    }
                    _ => None,
                })
        {
            group26_activated = true;
            group26_owner_mana_global_at_boundary = Some(owner_mana_global_at_boundary);
            group26_owner_mana_global_after = Some(session.economy().owner_mana_global());
            model13_activated.extend(
                authored_indices
                    .iter()
                    .copied()
                    .filter(|index| model13_indices.contains(index)),
            );
        }
        group25_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 25 && authored_indices.as_ref() == expected_group25_indices
            )
        });
        if session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 356)
        {
            break;
        }
    }
    assert!(group25_activated, "group 25 must allocate row 356");

    let row356_position = feature_position(356);
    session
        .set_level2_entity_position(session.state.entity_index, row356_position)
        .expect("fixture player position updates the authoritative row and map link");
    for _ in 0..16 {
        if group26_activated {
            break;
        }
        let (_, owner_mana_global_at_boundary) =
            level2_owner_mana_projection(&session.economy().owner_mana_rows);
        session.step_outer_turn(PlayerCommand::default(), 0);
        let activated_this_turn = session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 26 && authored_indices.as_ref() == expected_group26_indices
            )
        });
        if activated_this_turn {
            group26_owner_mana_global_at_boundary = Some(owner_mana_global_at_boundary);
            group26_owner_mana_global_after = Some(session.economy().owner_mana_global());
            for event in session.dispatch_events() {
                if let Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 26,
                    authored_indices,
                    ..
                } = event
                {
                    model13_activated.extend(
                        authored_indices
                            .iter()
                            .copied()
                            .filter(|index| model13_indices.contains(index)),
                    );
                }
            }
        }
        group26_activated |= activated_this_turn;
        if expected_group26_sphere_indices.iter().all(|index| {
            session.economy().spheres().iter().any(|sphere| {
                sphere.active && sphere.mana == 512 && {
                    let position = feature_position(*index);
                    (sphere.position.x, sphere.position.y) == (position.x, position.y)
                }
            })
        }) {
            break;
        }
    }
    assert!(
        group26_activated,
        "row 356 must activate group 26 through production turns"
    );
    let expected_positions = expected_group26_sphere_indices
        .iter()
        .map(|index| {
            let position = feature_position(*index);
            (position.x, position.y)
        })
        .collect::<BTreeSet<_>>();
    let mut actual_position_counts = BTreeMap::new();
    for sphere in session
        .economy()
        .spheres()
        .iter()
        .filter(|sphere| expected_positions.contains(&(sphere.position.x, sphere.position.y)))
    {
        *actual_position_counts
            .entry((sphere.position.x, sphere.position.y))
            .or_insert(0usize) += 1;
        assert!(sphere.active);
        assert_eq!(sphere.mana, 512);
        assert_eq!(
            (sphere.owner_entity_index, sphere.player_entity_index),
            (0, 0)
        );
    }
    let expected_position_counts = expected_positions
        .iter()
        .copied()
        .map(|position| (position, 1usize))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_position_counts, expected_position_counts);
    let owner_mana_global_at_boundary = group26_owner_mana_global_at_boundary
        .expect("group 26 owner-mana boundary projection captured");
    let owner_mana_global_after =
        group26_owner_mana_global_after.expect("group 26 owner-mana projection captured");
    assert_eq!(
        owner_mana_global_after, owner_mana_global_at_boundary,
        "group 26 spheres cannot publish sub_60F00's global counter in their allocation pass"
    );

    // sub_4A1E0 consumes all eight authored rows synchronously, including
    // the four class-10/model-13 constructors. Retain their actual source
    // birth receipts while advancing the ordinary denominator publication
    // outside the direct model-39 sphere allocation assertions above.
    for _ in 0..4 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        for event in session.dispatch_events() {
            if let Mc2DispatchEvent::AuthoredGroupActivated {
                group: 26,
                authored_indices,
                ..
            } = event
            {
                model13_activated.extend(
                    authored_indices
                        .iter()
                        .copied()
                        .filter(|index| model13_indices.contains(index)),
                );
            }
        }
        if model13_activated == model13_indices {
            break;
        }
    }
    assert_eq!(model13_activated, model13_indices);
    assert!(
        session.economy().owner_mana_global() >= owner_mana_global_at_boundary + 4 * 512,
        "group 26 spheres publish on a later source boundary"
    );
    for index in model13_indices {
        let source = session
            .economy()
            .authored_model13()
            .iter()
            .find(|source| source.authored_index == index)
            .copied()
            .expect("group 26 must materialize model-13 rows");
        assert_eq!(
            (
                source.source_class,
                source.model,
                source.action,
                source.subspell_index,
                source.max_life,
                source.dword_0x10,
                source.particle_index,
                source.control_flags,
                source.shift_rot_x,
                source.shift_rot_y,
            ),
            (10, 19, 19, 200, 240, 0, 228, 0x0002_0001, 512, 512)
        );
        assert!((0..=source.max_life).contains(&source.life));
        let position = feature_position(index);
        assert_eq!(
            (source.position.x, source.position.y, source.position.z),
            (
                position.x,
                position.y,
                session.canonical_terrain().ground_height(position)
            )
        );
        assert!(source.active && source.drawing_enabled);
    }

    session.reset();
    assert_eq!(session.economy().spheres().len(), baseline_sphere_count);
    assert_eq!(
        session.economy().owner_mana_global(),
        baseline_owner_mana_global
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group24_model39_and_model6_materialize_from_row344_production_chain() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-24 feature")
            .world_position(0)
    };
    let expected_group23_indices = [343_u16, 344];
    let expected_group24_indices = authored_features
        .iter()
        .filter(|feature| feature.activation_group == 24)
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    assert_eq!(
        expected_group24_indices,
        vec![
            345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 361, 362, 363, 364
        ]
    );
    let expected_model39_positions = (345_u16..=354)
        .map(|index| {
            let position = feature_position(index);
            (position.x, position.y)
        })
        .collect::<BTreeSet<_>>();

    // First take the real imported row-342 -> group-23 path. This creates
    // row 344 as the authored model-0 switch whose stage tag is group 24.
    let root_position = feature_position(342);
    let root = session
        .level2_imported_switch_roots
        .iter_mut()
        .find(|root| root.authored_index == 342)
        .expect("imported row-342 group-23 root");
    root.position = root_position;
    root.target_group = 23;
    root.cooldown = 0;
    session
        .set_level2_entity_position(session.state.entity_index, root_position)
        .expect("fixture player position updates the authoritative row and map link");
    // NewEvent can place row 344 later in the constructing source pass.
    // Retain its actual group-24 receipt before any following pass replaces
    // dispatch_events; the constructors and consumers remain ordinary.
    let mut group24_receipts = Vec::new();
    let mut group23_activated = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        group24_receipts.extend(
            session
                .dispatch_events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated { group: 24, .. }
                    )
                })
                .cloned(),
        );
        group23_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 23 && authored_indices.as_ref() == expected_group23_indices
            )
        });
        if session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 344)
        {
            break;
        }
    }
    assert!(group23_activated, "row 342 must allocate group 23");

    // Position the local player over the newly-created row 344 and let
    // the normal source cursor consume its every-eighth-pass overlap.
    session
        .set_level2_entity_position(session.state.entity_index, feature_position(344))
        .expect("fixture player position updates the authoritative row and map link");
    let mut group24_activated = group24_receipts.iter().any(|event| {
        matches!(event, Mc2DispatchEvent::AuthoredGroupActivated {
            group: 24, authored_indices, ..
        } if authored_indices.as_ref() == expected_group24_indices)
    });
    for _ in 0..16 {
        if group24_activated {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        group24_receipts.extend(
            session
                .dispatch_events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated { group: 24, .. }
                    )
                })
                .cloned(),
        );
        group24_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 24 && authored_indices.as_ref() == expected_group24_indices
            )
        });
        if expected_model39_positions.iter().all(|position| {
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.active && (sphere.position.x, sphere.position.y) == *position)
        }) && [355_u16].iter().all(|index| {
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|source| source.active && source.authored_index == *index)
        }) && (361_u16..=364).all(|index| {
            session
                .economy()
                .authored_model6()
                .iter()
                .any(|source| source.active && source.authored_index == index)
        }) {
            break;
        }
    }
    assert!(group24_activated, "row 344 must allocate group 24");
    assert_eq!(
        group24_receipts.len(),
        1,
        "the source group is consumed exactly once"
    );
    assert!(
        expected_group24_indices
            .iter()
            .all(|index| group24_receipts.iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 24,
                    authored_indices,
                    ..
                } if authored_indices.as_ref().contains(index)
            )))
    );
    assert_eq!(
        session
            .economy()
            .authored_model6()
            .iter()
            .filter(|source| source.active)
            .map(|source| source.authored_index)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([361, 362, 363, 364])
    );
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.active && source.authored_index == 355)
    );
    assert!(
        session
            .economy()
            .spheres()
            .iter()
            .filter(|sphere| {
                sphere.active
                    && expected_model39_positions.contains(&(sphere.position.x, sphere.position.y))
            })
            .count()
            >= 10
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert!(session.economy().authored_model6().is_empty());
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .all(|source| source.authored_index != 355)
    );
}

#[test]
fn level_two_group25_row356_materializes_from_row355_production_route() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-25 feature")
    };

    // Reuse the real imported row-342 -> group-23 -> row-344 route. The
    // existing qualified root is the source authority; only its authored
    // position/target are rebound to the production route under test.
    let local_player = session.state.entity_index;
    let root_position = feature(342).world_position(0);
    let root = session
        .level2_imported_switch_roots
        .iter_mut()
        .find(|root| root.authored_index == 342)
        .expect("imported row-342 group-23 root");
    root.position = root_position;
    root.target_group = 23;
    root.cooldown = 0;
    let root_phase = root.phase;
    session
        .set_level2_entity_position(local_player, root_position)
        .expect("public player-position input for row 342");
    for _ in 0..level_two_switch_turns_until_cadence(root_phase) {
        session
            .set_level2_entity_position(local_player, root_position)
            .expect("public player-position input remains on row 342");
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 344)
        {
            break;
        }
    }
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 344)
    );

    // Row 344's model-0 overlap activates group 24. The newly allocated
    // row 355 is model-1 at the adjacent authored tile. Its 31-cell
    // extent includes row 344, so leave that extent before its live source
    // cadence selects group 25/row 356.
    let row344_position = feature(344).world_position(0);
    session
        .set_level2_entity_position(local_player, row344_position)
        .expect("public player-position input for row 344");
    let mut group25_activated = false;
    let saw_group25_activation = |session: &Mc2Simulation| {
        session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated {
                    group: 25,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == [356]
            )
        })
    };
    let row344_phase = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 344)
        .map(|source| source.phase)
        .expect("row 344 producer remains live before its cadence");
    for _ in 0..level_two_switch_turns_until_cadence(row344_phase) {
        session
            .set_level2_entity_position(local_player, row344_position)
            .expect("public player-position input remains on row 344");
        session.step_outer_turn(PlayerCommand::default(), 0);
        group25_activated |= saw_group25_activation(&session);
    }
    if !group25_activated {
        let row355 = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 355)
            .copied()
            .expect("row 355 producer remains live before its cadence");
        let mut row355_exit_position = row355.position;
        row355_exit_position.x = row355_exit_position.x.wrapping_add(0x8000);
        for _ in 0..level_two_switch_turns_until_cadence(row355.phase) {
            session
                .set_level2_entity_position(local_player, row355_exit_position)
                .expect("public player-position input leaves the row-355 inverse-overlap extent");
            session.step_outer_turn(PlayerCommand::default(), 0);
            group25_activated |= saw_group25_activation(&session);
        }
    }
    assert!(
        group25_activated,
        "row 355 must activate group 25 in production"
    );

    let row355 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 355)
        .copied()
        .expect("group 24 must materialize row 355");
    assert_eq!((row355.model, row355.action, row355.stage_tag), (1, 1, 25));
    assert!(row355.fired);
    // CompleteAuthoredSwitch hides the one-shot producer after its group
    // scan; firing never leaves its drawing bit enabled.
    assert!(!row355.drawing_enabled);

    let row356 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 356)
        .copied()
        .expect("group 25 must materialize row 356");
    assert_eq!(
        (
            row356.source_class,
            row356.model,
            row356.action,
            row356.stage_tag,
            row356.word_10,
            row356.dword_0x10,
        ),
        (11, 0, 0, 26, 12, 0)
    );
    if row356.drawing_enabled {
        for _ in 0..level_two_switch_turns_until_cadence(row356.phase) {
            session
                .set_level2_entity_position(local_player, row344_position)
                .expect("public player-position input remains on row 344");
            session.step_outer_turn(PlayerCommand::default(), 0);
        }
    }
    let row356 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 356)
        .copied()
        .expect("group 25 row 356 remains live");
    let authored_row356 = feature(356);
    assert_eq!(
        (row356.position.x, row356.position.y, row356.position.z),
        (
            authored_row356.world_position(0).x,
            authored_row356.world_position(0).y,
            session
                .canonical_terrain()
                .ground_height(authored_row356.world_position(0))
        )
    );
    assert!(row356.active);
    assert!(!row356.drawing_enabled);
    assert!(row356.fired);

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}
