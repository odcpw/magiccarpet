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
            Level2DispatchEvent::AuthoredGroupActivated {
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
        Level2DispatchEvent::DynamicArena {
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
        Level2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
    )));

    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.level2_objective_done());
    assert!(session.economy.entities()[32].drawing_disabled());
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 8,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [153, 154]
    )));
}

#[test]
fn level_two_model32_switch_scans_non_local_objective_owner() {
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

    assert!(session.level2_objective_done());
    assert_eq!(
        session.runtime.objectives[usize::from(remote_color)].done,
        1
    );
    assert_eq!(session.runtime.objectives[0].done, 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
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
        Level2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
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
        Level2DispatchEvent::AuthoredGroupActivated {
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective stage-zero session");
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 objective level-end session");
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

    session
        .set_level2_entity_position(111, row_65.world_position(0))
        .expect("move live class-three model-zero target");
    assert!(
        session
            .level2_authored_switch_matches(65)
            .expect("model-zero overlap predicate")
    );
    assert!(
        session
            .level2_authored_switch_matches(83)
            .expect("model-one non-overlap predicate")
    );

    // Row 83 is 512 units away in Y. Moving the live model-0 target to
    // that center creates overlap and flips the model-1 predicate false.
    session
        .set_level2_entity_position(111, row_83.world_position(0))
        .expect("move live model-zero target");
    assert!(
        !session
            .level2_authored_switch_matches(83)
            .expect("model-one boundary predicate")
    );

    // Returning to the row-65 center restores strict interior overlap.
    session
        .set_level2_entity_position(111, row_65.world_position(0))
        .expect("restore live model-zero target");
    assert!(
        session
            .level2_authored_switch_matches(65)
            .expect("model-zero strict overlap predicate")
    );
    let strict_boundary = Position {
        x: row_65.world_position(0).x,
        y: row_65.world_position(0).y.wrapping_add(242),
        z: 0,
    };
    session
        .set_level2_entity_position(111, strict_boundary)
        .expect("move live model-zero target to strict boundary");
    assert!(
        !session
            .level2_authored_switch_matches(65)
            .expect("model-zero strict boundary predicate")
    );

    // A dead class-3/model-0 row is absent from remc2's linked player
    // chain, so it cannot satisfy an overlap switch even when its position
    // remains exactly at the authored center.
    session
        .set_level2_entity_position(111, row_65.world_position(0))
        .expect("restore overlap before dead-player check");
    session.economy.entities[111].life = -1;
    assert!(
        !session
            .level2_authored_switch_matches(65)
            .expect("dead model-zero row is excluded from overlap scan")
    );
}

#[test]
fn level_two_root_switch_materializes_authored_wind_on_production_turn() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 rooted wind-switch session");
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
    session.state.position = root.position;

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
        Level2DispatchEvent::AuthoredGroupActivated {
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
        .authored_model22
        .iter_mut()
        .find(|source| source.source_entity_index == wind_slot)
        .expect("live row-110 wind parent")
        .life = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model22()
            .iter()
            .all(|source| source.authored_index != 110)
    );
    session.state.position = Position {
        x: root.position.x.wrapping_add(4_096),
        y: root.position.y,
        z: root.position.z,
    };
    for _ in 0..12 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    session.state.position = root.position;
    let mut second_activation = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
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

    let mut boundary = inspection
        .level2_reset_session()
        .expect("Level 2 rooted wind-switch boundary session");
    boundary.state.position = Position {
        x: root.position.x,
        y: root.position.y.wrapping_add(242),
        z: root.position.z,
    };
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

    let mut dead = inspection
        .level2_reset_session()
        .expect("Level 2 rooted wind-switch dead-player session");
    dead.state.position = root.position;
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
            Level2DispatchEvent::AuthoredGroupActivated {
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
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [244]
    )));
    let switch = session.economy.entities()
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-4 exit-switch session");
    assert_eq!(
        session.economy.entities()[usize::from(session.state.entity_index)].model,
        0
    );

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
        Level2DispatchEvent::AuthoredGroupActivated {
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
    let switch = session.economy.entities()
        .iter()
        .find(|entity| {
            entity.active
                && entity.class == 11
                && entity.model == 4
                && entity.action == 4
                && entity.source_id == 31
        })
        .expect("live model-4 group-31 switch");
    assert!(switch.drawing_disabled());
}

#[test]
fn level_two_model12_group31_materializes_row209_without_empty_group32_work() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 model-12 session");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-12 group-31 session");
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
            session.canonical_terrain().ground_height(row209.world_position(0))
        )
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == LEVEL2_GROUP31_AUTHORED_INDICES
    )));
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated { group: 32, .. }
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
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 model-12 rollback session");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-12 rollback group-31 session");
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
        Level2DispatchEvent::AuthoredGroupActivated { group: 31, .. }
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Error { message, .. } if message.contains("row 244")
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
        Level2DispatchEvent::AuthoredGroupActivated { group: 31, .. }
    )));
    let switch = session.economy.entities()
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-4 exit-switch session");
    let local_index = usize::from(session.state.entity_index);
    session.economy.entities[local_index].model = 1;

    let remote_player = session
        .players
        .iter()
        .find(|player| player.slot != 0 && player.active_player_index >= 0)
        .map(|player| (player.entity_index, player.player_color_index()))
        .expect("fixture has a non-local live player row");
    let remote_index = usize::from(remote_player.0);
    let remote = session.economy.entities
        .get_mut(remote_index)
        .expect("remote player entity is in the entity arena");
    assert!(remote.active && remote.class == 3 && remote.life >= 0);
    remote.model = 0;
    session.runtime.objectives[usize::from(remote_player.1)].is_level_end = 1;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
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
    let local = session.economy.entities
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
        Level2DispatchEvent::AuthoredGroupActivated { group: 31, .. }
    )));
    let switch = session.economy.entities()
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-31 switch session");
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
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 28,
            authored_indices,
            source_slot,
            ..
        } if *source_slot == runtime_switch_slot
            && authored_indices.as_ref() == expected_group28.as_slice()
    )));
    let collected_scroll_slots = session
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
    assert_eq!(collected_scroll_slots.len(), expected_group28.len());
    assert!(
        !session
            .economy()
            .authored_class14()
            .iter()
            .any(|row| row.authored_index == 275)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
    )));
}

#[test]
fn level_two_group31_switch274_rejects_the_strict_boundary() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-31 switch boundary session");
    let row_274 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 274)
        .copied()
        .expect("group-31 row-274 switch");
    let strict_boundary = Position {
        x: row_274.world_position(0).x,
        y: row_274.world_position(0).y.wrapping_add(242),
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
                Level2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
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
        Level2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-31 reset session");
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
        Level2DispatchEvent::AuthoredGroupActivated {
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-21 Bee session");
    let baseline_hash = session.campaign_state_hash();
    assert_eq!(session.runtime().stage_variables[6].word_2, 0);
    assert!(session.allocate_level2_authored_bee(228).is_none());

    // Row 228 is the ordinary model-32 producer. Its real group-21
    // callback constructs the three Day Bees; no test mutates their
    // action or StageVar fields before this proof.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let group21 = [229_u16, 230, 231]
        .into_iter()
        .map(|authored_index| {
            session
                .economy()
                .authored_bees()
                .iter()
                .copied()
                .find(|source| source.authored_index == authored_index)
                .expect("ordinary group-21 Day Bee")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        group21
            .iter()
            .map(|source| {
                (
                    source.authored_index,
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
    // The same ordinary reset pass also reaches group 4's three qualified
    // Bees before group 21, so the source-owned L2-06 high-byte cadence
    // has advanced once for each of the six successful constructors.
    assert_eq!(session.runtime().stage_variables[6].word_2, 0x0600);
    assert_eq!(
        &session.runtime().bytes()[392 + 6 * 12 + 2..392 + 6 * 12 + 4],
        &0x0600_u16.to_le_bytes()
    );

    session.reset();
    assert_eq!(session.runtime().stage_variables[6].word_2, 0);
    assert!(session.economy().authored_bees().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);

    // An actual exhausted source arena cannot manufacture a Bee or spend
    // the StageVar construction cadence.
    session.economy_mut().free_slots.clear();
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().authored_bees().is_empty());
    assert_eq!(session.runtime().stage_variables[6].word_2, 0);
}

#[test]
fn level_two_bee_row229_death_advances_type7_objective_on_production_path() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-21 Bee objective session");
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
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee");
    session
        .economy_mut()
        .authored_bees
        .iter_mut()
        .find(|bee| bee.source_entity_index == row229_slot)
        .expect("production row 229 source")
        .action = 20;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .economy()
            .authored_bees()
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
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied()
            .expect("phase-gated action-21 row remains until its callback");
        session.step_outer_turn(PlayerCommand::default(), 0);
        let after = session
            .economy()
            .authored_bees()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied()
            .expect("phase-gated action-21 row remains until the following boundary");
        if before.source_byte_3e & 7 == 0 {
            phase_zero_callback_seen = true;
            assert!(!after.active);
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
        2
    );
    assert_eq!(
        session.runtime().bytes()[304 + LEVEL2_BEE_OBJECTIVE_STAGE_INDEX + 3],
        2
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_bees()
            .iter()
            .all(|bee| bee.authored_index != LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
    );

    // Reset restores the selector and the production activation ledger.
    // Killing the exact authored target while another objective stage is
    // selected must not advance the type-7 slot.
    session.reset();
    assert_eq!(session.runtime().objectives[0].text_selector, 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row229_slot = session
        .economy()
        .authored_bees()
        .iter()
        .find(|bee| bee.authored_index == LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee for selector-negative path");
    session
        .economy_mut()
        .authored_bees
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-2 production session");
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
        Level2DispatchEvent::AuthoredGroupActivated {
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
            session.canonical_terrain()
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 switch-band production session");
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
    let objective_position = |session: &Level2ResetSession| {
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
    let step_at = |session: &mut Level2ResetSession,
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
                if let Level2DispatchEvent::AuthoredGroupActivated {
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
    let wait_for_row42 = |session: &mut Level2ResetSession,
                          row11_position: Position,
                          trace: &mut Vec<(u16, u16)>| {
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
            step_at(
                session,
                row11_position,
                level_two_switch_turns_until_cadence(row41_phase),
                trace,
            );
        }
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
    step_at(
        &mut session,
        feature_position(65),
        16,
        &mut activation_trace,
    );
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
    step_at(
        &mut session,
        feature_position(65),
        16,
        &mut activation_trace,
    );
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

    let mut allocation_failure = inspection
        .level2_reset_session()
        .expect("allocation-failure session");
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
            Level2DispatchEvent::AuthoredGroupActivated {
                group: 3,
                authored_indices,
                ..
            } => Some(authored_indices),
            _ => None,
        })
        .collect::<Vec<_>>();
    let established_group3_batch = group3_main_batches
        .iter()
        .find(|batch| batch.as_ref() == expected_group3_main_indices.as_slice())
        .expect("group-3 established main batch");
    assert_eq!(established_group3_batch.len(), 22);
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
                Level2DispatchEvent::Error { message, .. }
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 switch-band-B production session");
    let baseline_hash = session.campaign_state_hash();
    let local_player = session.state.entity_index;

    let drive_band = |session: &mut Level2ResetSession| {
        let mut trace = Vec::<(u16, u16)>::new();
        let step_at = |session: &mut Level2ResetSession,
                       position: Position,
                       turns: usize,
                       trace: &mut Vec<(u16, u16)>| {
            for _ in 0..turns {
                session
                    .set_level2_entity_position(local_player, position)
                    .expect("public local-player position input");
                session.step_outer_turn(PlayerCommand::default(), 0);
                for event in session.dispatch_events() {
                    if let Level2DispatchEvent::AuthoredGroupActivated {
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

        // The four wind roots are live group-zero model-0 source rows.
        // Their ordinary overlap callbacks cover the alternating odd
        // groups; the same first source pass also consumes the already
        // live model-76 producers for groups 12, 14, and 16.
        for target_group in [9_u16, 11, 13, 15] {
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
        trace
    };

    let first_trace = drive_band(&mut session);
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
        session
            .economy()
            .authored_model32()
            .iter()
            .map(|source| source.authored_index)
            .filter(|index| matches!(index, 208 | 213))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([208, 213])
    );
    assert_eq!(
        session
            .economy()
            .authored_bees()
            .iter()
            .map(|source| source.authored_index)
            .filter(|index| matches!(index, 236 | 237 | 257))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([236, 237, 257])
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    let replay_trace = drive_band(&mut session);
    assert_eq!(replay_trace, first_trace, "reset preserves source order");
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-1 row-41 production session");
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

    let expected_group1_indices = (12_u16..=15)
        .chain(17..=40)
        .chain([41, 167, 168, 169, 206])
        .collect::<Vec<_>>();
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
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
    let row16_activation_seen = session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Level2DispatchEvent::AuthoredGroupActivated {
                group: 1,
                authored_indices,
                ..
            } if authored_indices.as_ref() == [16]
        )
    });
    let row16_dynamic_seen = session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Level2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaAttraction,
                ..
            } if *source_slot == row16_source_slot
        )
    });
    if !row41.fired {
        for _ in 0..level_two_switch_turns_until_cadence(row41.phase) {
            session
                .set_level2_entity_position(local_player, row11_position)
                .expect("public player-position input remains on row 11");
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
            session.canonical_terrain()
                .ground_height(authored_row41.world_position(0))
        )
    );
    assert!(row41.active);
    // Model-1 uses the inverse-overlap callback: the player remains on
    // row 11 while the live row-41 producer reaches its source cadence.
    assert!(row41.fired);
    assert_eq!(row41.cooldown, 10);
    assert!(row41.drawing_enabled);

    // Row 16 is the same source group but a class-10/model-54 field.
    // The source `sub_4A1E0` scan reaches it through the live row-11
    // producer after the group-1 switch callback; it is therefore a
    // separate activation event, not part of the model-0/1 constructor
    // batch above.  Keep this assertion on the real outer-turn route so
    // the row cannot be proven only by direct constructor allocation.
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
            session.canonical_terrain()
                .ground_height(authored_row16.world_position(0))
        )
    );
    assert!(row16_activation_seen);
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
            Level2DispatchEvent::DynamicArena {
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-6 production session");
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
    // 42 -> row 64 -> row 65 -> row 83.  The player remains on row 65 so
    // the model-1 inverse-overlap callback can select group 6.
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
    let row65_position = feature(65).world_position(0);
    let mut group6_activated = false;
    for source_index in [64_u16, 65, 83] {
        let source_phase = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == source_index)
            .map(|source| source.phase)
            .expect("group-6 producer remains live before its cadence");
        for _ in 0..level_two_switch_turns_until_cadence(source_phase) {
            session
                .set_level2_entity_position(local_player, row65_position)
                .expect("public player-position input remains on row 65");
            session.step_outer_turn(PlayerCommand::default(), 0);
            group6_activated |= session.dispatch_events().iter().any(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
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
    assert!(row83.drawing_enabled);

    let row84 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 84)
        .copied()
        .expect("group 6 must materialize row 84");
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
            session.canonical_terrain()
                .ground_height(authored_row84.world_position(0))
        )
    );
    assert!(row84.active);
    // The source runagain cursor visits the newly allocated row 84 in
    // this same pass.  Because the player is still on row 65's authored
    // tile (which is row 84's tile), the downstream group-7 callback
    // consumes it immediately and hides the source drawing bit.
    assert!(!row84.drawing_enabled);
    assert!(row84.fired);

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group3_model39_rows_materialize_from_row42_production_route() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-3 production session");
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
    let expected_group3_indices = (43_u16..=62).chain([64, 439]).collect::<Vec<_>>();

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
                Level2DispatchEvent::AuthoredGroupActivated {
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
            Level2DispatchEvent::DynamicArena {
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
    let row439_children = session
        .economy()
        .model17_spawned_children()
        .iter()
        .filter(|child| child.parent_entity_index == row439.source_entity_index)
        .collect::<Vec<_>>();
    assert_eq!(row439_children.len(), 3);
    assert!(row439_children.iter().all(|child| {
        (
            child.parent_entity_id,
            child.control_flags,
            child.shift_rot_x,
            child.shift_rot_y,
        ) == (row439.entity_id, 0x0003_0080, 512, 512)
    }));
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
            Level2DispatchEvent::DynamicArena {
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored switch session");
    let baseline_hash = session.campaign_state_hash();
    let baseline_sphere_count = session.economy().spheres().len();
    let baseline_owner_mana_global = session.economy().owner_mana_global();
    let expected_group1_model39_indices = (12..=15).chain(17..=40).collect::<Vec<_>>();
    let expected_group5_model39_indices = (66..=81).collect::<Vec<_>>();
    let expected_group7_indices = (85..=104).collect::<Vec<_>>();
    let expected_group7_event_indices = expected_group7_indices
        .iter()
        .copied()
        .chain([165])
        .collect::<Vec<_>>();
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
    let expected_group1_indices = expected_group1_model39_indices
        .iter()
        .copied()
        .chain([41, 167, 168, 169, 206])
        .collect::<Vec<_>>();
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
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
    let imported_root = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 11)
        .expect("imported row-11 root");
    assert_eq!(imported_root.source_slot, 23);
    assert_eq!(imported_root.phase, baseline_root_phase.wrapping_add(1));
    assert_eq!(imported_root.cooldown, 10);
    let imported_entity = session.economy.entities()
        .iter()
        .find(|entity| entity.index == 23)
        .expect("imported row-11 entity");
    assert_eq!(imported_entity.phase, baseline_root_phase.wrapping_add(1));
    assert_eq!(imported_entity.source_word_10(), 10);
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
            session
                .set_level2_entity_position(local_player, row11_position)
                .expect("public player-position input remains on row 11");
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
    assert!(switch_indices.contains(&42));
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

    // Row 42 is the model-0 overlap gate for row 64; row 64 is the
    // model-1 inverse gate for row 65.  Both transitions are driven by
    // ordinary production turns, not direct allocator calls.
    let row42_position = feature_position(42);
    let row42_phase = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 42)
        .map(|source| source.phase)
        .expect("row 42 producer remains live before its cadence");
    for _ in 0..level_two_switch_turns_until_cadence(row42_phase) {
        session
            .set_level2_entity_position(local_player, row42_position)
            .expect("public player-position input remains on row 42");
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let switch_indices = session
        .economy()
        .authored_switch_producers()
        .iter()
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert!(switch_indices.contains(&64));

    // Rows 42 and 64 share the authored tile: move away after firing
    // row 42 so row 64's inverse-overlap callback can select group 4.
    let row65_position = feature_position(65);
    session
        .set_level2_entity_position(local_player, row65_position)
        .expect("public player-position input for row 65");
    let expected_group5_indices = expected_group5_model39_indices
        .iter()
        .copied()
        .chain([83, 164])
        .collect::<Vec<_>>();
    let mut group5_activated = false;
    let mut group5_owner_mana_global_before = None;
    let mut group5_owner_mana_global_after = None;
    let mut group7_activated = false;
    let mut group7_owner_mana_global_before = None;
    let mut group7_owner_mana_global_after = None;
    let mut group4_bees_activated = false;
    let mut mana_attraction_source_slots = BTreeSet::new();
    let mut class5_source_slots = BTreeSet::new();
    let mut row105_group_activated = false;
    let mut group5_sphere_snapshot = None;
    let mut group7_sphere_snapshot = None;
    let classify_chain_events = |events: &[Level2DispatchEvent]| {
        let mut group5_this_turn = false;
        let mut group7_this_turn = false;
        let mut row105_this_turn = false;
        let mut group4_this_turn = false;
        let mut mana_attraction_slots = BTreeSet::new();
        let mut class5_slots = BTreeSet::new();
        for event in events {
            match event {
                Level2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 5
                    && authored_indices.as_ref() == expected_group5_indices.as_slice() =>
                {
                    group5_this_turn = true;
                }
                Level2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 7
                    && authored_indices.as_ref() == expected_group7_event_indices.as_slice() =>
                {
                    group7_this_turn = true;
                }
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 7,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == [105] => {
                    row105_this_turn = true;
                }
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 4,
                    authored_indices,
                    ..
                } if matches!(authored_indices.as_ref(), [210] | [211] | [212]) => {
                    group4_this_turn = true;
                }
                Level2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::ManaAttraction,
                    ..
                } => {
                    mana_attraction_slots.insert(*source_slot);
                }
                Level2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::AuthoredClass5,
                    ..
                } => {
                    class5_slots.insert(*source_slot);
                }
                _ => {}
            }
        }
        (
            group5_this_turn,
            group7_this_turn,
            row105_this_turn,
            group4_this_turn,
            mana_attraction_slots,
            class5_slots,
        )
    };
    let record_chain_turn =
        |session: &Level2ResetSession,
         owner_global_at_boundary: i32,
         owner_global_after: i32,
         group5_activated: &mut bool,
         group5_owner_mana_global_before: &mut Option<i32>,
         group5_owner_mana_global_after: &mut Option<i32>,
         group7_activated: &mut bool,
         group7_owner_mana_global_before: &mut Option<i32>,
         group7_owner_mana_global_after: &mut Option<i32>,
         row105_group_activated: &mut bool,
         group4_bees_activated: &mut bool,
         mana_attraction_source_slots: &mut BTreeSet<u16>,
         class5_source_slots: &mut BTreeSet<u16>,
         group5_sphere_snapshot: &mut Option<Vec<ManaSphere>>,
         group7_sphere_snapshot: &mut Option<Vec<ManaSphere>>| {
            let (
                group5_this_turn,
                group7_this_turn,
                row105_this_turn,
                group4_this_turn,
                mana_attraction_slots,
                class5_slots,
            ) = classify_chain_events(session.dispatch_events());
            if group5_this_turn {
                *group5_owner_mana_global_before = Some(owner_global_at_boundary);
                *group5_owner_mana_global_after = Some(owner_global_after);
            }
            if group7_this_turn {
                *group7_owner_mana_global_before = Some(owner_global_at_boundary);
                *group7_owner_mana_global_after = Some(owner_global_after);
            }
            *group5_activated |= group5_this_turn;
            *group7_activated |= group7_this_turn;
            *row105_group_activated |= row105_this_turn;
            *group4_bees_activated |= group4_this_turn;
            if group5_this_turn && group5_sphere_snapshot.is_none() {
                *group5_sphere_snapshot = Some(session.economy().spheres().to_vec());
            }
            if group7_this_turn && group7_sphere_snapshot.is_none() {
                *group7_sphere_snapshot = Some(session.economy().spheres().to_vec());
            }
            mana_attraction_source_slots.extend(mana_attraction_slots);
            class5_source_slots.extend(class5_slots);
        };
    for source_index in [64_u16, 65, 83, 84] {
        if (source_index == 65 && group5_activated)
            || (matches!(source_index, 83 | 84) && group7_activated)
        {
            continue;
        }
        let source_phase = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == source_index)
            .map(|source| source.phase)
            .expect("chain producer remains live before its cadence");
        for _ in 0..level_two_switch_turns_until_cadence(source_phase) {
            let (_, owner_global_at_boundary) =
                level2_owner_mana_projection(&session.economy().owner_mana_rows);
            session
                .set_level2_entity_position(local_player, row65_position)
                .expect("public player-position input remains on row 65");
            session.step_outer_turn(PlayerCommand::default(), 0);
            record_chain_turn(
                &session,
                owner_global_at_boundary,
                session.economy().owner_mana_global(),
                &mut group5_activated,
                &mut group5_owner_mana_global_before,
                &mut group5_owner_mana_global_after,
                &mut group7_activated,
                &mut group7_owner_mana_global_before,
                &mut group7_owner_mana_global_after,
                &mut row105_group_activated,
                &mut group4_bees_activated,
                &mut mana_attraction_source_slots,
                &mut class5_source_slots,
                &mut group5_sphere_snapshot,
                &mut group7_sphere_snapshot,
            );
        }
    }
    let switch_indices = session
        .economy()
        .authored_switch_producers()
        .iter()
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert!(switch_indices.contains(&65));
    assert!(group4_bees_activated, "row 64 must activate group 4 Bees");
    let expected_group4_bee_indices = BTreeSet::from([210_u16, 211, 212]);
    assert!(expected_group4_bee_indices.iter().all(|index| {
        session
            .economy()
            .authored_bees()
            .iter()
            .any(|source| source.active && source.authored_index == *index)
    }));
    // Action 17 now runs its source movement callback on every subsequent
    // cursor visit, so these live rows are expected to have moved since
    // activation. Constructor placement is asserted on the fresh row-229
    // production route in
    // `level_two_bee_row229_action17_moves_on_outer_cursor_and_reset_restores_hash`.

    // Firing row 65 selects authored group 5.  The bounded downstream
    // consumers are already production-owned: row 83 is the next switch,
    // row 82 is model-54, and rows 248..251 are class-14/model-5.
    let switch_indices = session
        .economy()
        .authored_switch_producers()
        .iter()
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert!(switch_indices.contains(&83));
    let row65 = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 65)
        .expect("fired row65 source");
    assert!(row65.fired);
    assert!(row65.cooldown > 0);
    let row82 = session
        .economy()
        .mana_attraction_fields()
        .iter()
        .find(|field| field.authored_index == 82)
        .copied()
        .expect("group 5 must materialize authored model-54 row 82");
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
        .filter(|source| matches!(source.authored_index, 164 | 165))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        group5_class5
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        vec![164, 165]
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
            Level2DispatchEvent::DynamicArena {
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
    let group5_owner_mana_global_before = group5_owner_mana_global_before
        .expect("group 5 owner-mana baseline captured on activation");
    let group5_owner_mana_global_after = group5_owner_mana_global_after
        .expect("group 5 owner-mana projection captured on activation");
    assert_eq!(
        group5_owner_mana_global_after, group5_owner_mana_global_before,
        "group 5 spheres cannot publish sub_60F00's global counter in their allocation pass"
    );
    assert!(
        session.economy().owner_mana_global() >= group5_owner_mana_global_before + 16 * 512,
        "later production turns publish the group 5 spheres"
    );
    let expected_group5_positions = expected_group5_model39_indices
        .iter()
        .copied()
        .map(feature_position)
        .collect::<Vec<_>>();
    let expected_group5_position_set = expected_group5_positions
        .iter()
        .map(|position| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    let mut actual_group5_position_counts = BTreeMap::new();
    for sphere in group5_sphere_snapshot
        .expect("group 5 consumer snapshot captured on activation")
        .iter()
        .filter(|sphere| {
            expected_group5_position_set.contains(&(sphere.position.x, sphere.position.y))
        })
    {
        *actual_group5_position_counts
            .entry((sphere.position.x, sphere.position.y))
            .or_insert(0usize) += 1;
        assert!(sphere.active);
        assert_eq!(sphere.mana, 512);
        assert_eq!(
            (sphere.owner_entity_index, sphere.player_entity_index),
            (0, 0)
        );
    }
    let expected_group5_position_counts = expected_group5_position_set
        .iter()
        .copied()
        .map(|position| (position, 1usize))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual_group5_position_counts, expected_group5_position_counts,
        "group 5 materializes one sphere for every row 66..81 position"
    );
    let expected_group7_positions = expected_group7_indices
        .iter()
        .copied()
        .map(feature_position)
        .collect::<Vec<_>>();
    let expected_group7_position_set = expected_group7_positions
        .iter()
        .map(|position| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    let mut actual_group7_position_counts = BTreeMap::new();
    for sphere in group7_sphere_snapshot
        .expect("group 7 consumer snapshot captured on activation")
        .iter()
        .filter(|sphere| {
            expected_group7_position_set.contains(&(sphere.position.x, sphere.position.y))
        })
    {
        *actual_group7_position_counts
            .entry((sphere.position.x, sphere.position.y))
            .or_insert(0usize) += 1;
        assert!(sphere.active);
        assert_eq!(sphere.mana, 512);
        assert_eq!(
            (sphere.owner_entity_index, sphere.player_entity_index),
            (0, 0)
        );
    }
    let expected_group7_position_counts = expected_group7_position_set
        .iter()
        .copied()
        .map(|position| (position, 1usize))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual_group7_position_counts, expected_group7_position_counts,
        "group 7 materializes one sphere for every row 85..104 position"
    );
    let group7_owner_mana_global_before = group7_owner_mana_global_before
        .expect("group 7 owner-mana baseline captured on activation");
    let group7_owner_mana_global_after = group7_owner_mana_global_after
        .expect("group 7 owner-mana projection captured on activation");
    assert_eq!(
        group7_owner_mana_global_after, group7_owner_mana_global_before,
        "group 7 spheres cannot publish sub_60F00's global counter in their allocation pass"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session.economy().owner_mana_global() >= group7_owner_mana_global_before + 20 * 512,
        "group 7 spheres publish at the next source boundary"
    );

    session.reset();
    assert!(session.economy().authored_switch_producers().is_empty());
    assert!(session.economy().authored_bees().is_empty());
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored model-32 session");
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
    assert_eq!(session.runtime.objectives[0].done, 1);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            source_slot,
            group: 17,
            authored_indices,
            ..
        } if *source_slot == row206.source_entity_index
            && authored_indices.as_ref() == [208]
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 17,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [236]
            || authored_indices.as_ref() == [237]
            || authored_indices.as_ref() == [257]
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
        .authored_bees()
        .iter()
        .filter(|source| matches!(source.authored_index, 236 | 237 | 257))
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(bee_indices, BTreeSet::from([236, 237, 257]));

    session.reset();
    assert!(session.economy().authored_model32().is_empty());
    assert!(
        session
            .economy()
            .authored_bees()
            .iter()
            .all(|source| !matches!(source.authored_index, 236 | 237 | 257))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group18_row213_materializes_from_model32_production_route() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-18 production session");
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
        Level2DispatchEvent::AuthoredGroupActivated {
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
            session.canonical_terrain()
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored model-32 continuation session");
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
        Level2DispatchEvent::AuthoredGroupActivated {
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
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 19,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [214, 224, 225, 226, 227]
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-20 production session");
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
    session.runtime.objectives[0].stage_selectors[3] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_switch_producers()
            .iter()
            .any(|source| source.authored_index == 214)
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
                Level2DispatchEvent::AuthoredGroupActivated {
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
        (1, 1, 400, 1, 41, 0x0002_0002)
    );
    assert_eq!(model1.last_sound_id, 3);
    assert!(model1.active);
    let quickfair_children = session
        .economy()
        .model17_spawned_children()
        .iter()
        .filter(|child| child.parent_entity_index == model1.source_entity_index)
        .collect::<Vec<_>>();
    assert!(!quickfair_children.is_empty());
    assert!(quickfair_children.iter().all(|child| {
        (
            child.parent_entity_id,
            child.control_flags,
            child.shift_rot_x,
            child.shift_rot_y,
        ) == (model1.source_entity_index, 0x0002_0080, 128, 128)
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
    assert_eq!(
        (
            archer.dword_0x10,
            archer.particle_index,
            archer.yaw,
            archer.pitch,
            archer.fov,
            archer.shift_rot_x,
            archer.shift_rot_y,
        ),
        (0, 0, 125, 66, 125, 128, 256,)
    );
    assert!((0_i16..=0x07ff_i16).contains(&archer.roll));
    assert_eq!(
        session
            .economy()
            .source_rng
            .get(&archer.source_entity_index),
        Some(&archer.entity_rng)
    );
    assert!(archer.active && archer.drawing_enabled);

    // The root uses remc2's `life-- < 0` callback guard: after the
    // activation turn's first visit, two more source turns exhaust life
    // and hide drawing while retaining the allocated row.
    session.step_outer_turn(PlayerCommand::default(), 0);
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
            Level2DispatchEvent::DynamicArena {
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
    expected_archer_position.z = session.canonical_terrain().ground_height(expected_archer_position);
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-21 Bee session");
    let baseline_hash = session.campaign_state_hash();

    // The qualified reset contains a live class-11/model-32 producer at
    // source slot 76 with source id/group 21. The ordinary source pass
    // must consume that real producer and materialize the three authored
    // group-21 Bees without a direct allocator or fixture-only route.
    let producer_slot = session.economy.entities()
        .iter()
        .find(|entity| {
            entity.index == 76
                && entity.active
                && entity.class == 11
                && entity.model == 32
                && entity.action == 32
                && entity.source_id == 21
                && !entity.drawing_disabled()
        })
        .expect("reset group-21 model-32 producer")
        .index;
    session.step_outer_turn(PlayerCommand::default(), 0);

    let expected_bees = BTreeSet::from([229_u16, 230, 231]);
    let actual_bees = session
        .economy()
        .authored_bees()
        .iter()
        .filter(|source| source.active && expected_bees.contains(&source.authored_index))
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_bees, expected_bees);
    let activated_from_producer = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 21,
                authored_indices,
                ..
            } if *source_slot == producer_slot => authored_indices.first().copied(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(activated_from_producer, vec![229, 230, 231]);

    session.reset();
    assert!(
        session
            .economy()
            .authored_bees()
            .iter()
            .all(|source| !expected_bees.contains(&source.authored_index))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_reset_model32_producer_reaches_group29_bees_from_production_turns() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-29 Bee session");
    let baseline_hash = session.campaign_state_hash();

    let producer_slot = session.economy.entities()
        .iter()
        .find(|entity| {
            entity.index == 87
                && entity.active
                && entity.class == 11
                && entity.model == 32
                && entity.action == 32
                && entity.source_id == 29
                && !entity.drawing_disabled()
        })
        .expect("reset group-29 model-32 producer")
        .index;
    session.step_outer_turn(PlayerCommand::default(), 0);

    let expected_bees = BTreeSet::from([307_u16, 308, 312]);
    let actual_bees = session
        .economy()
        .authored_bees()
        .iter()
        .filter(|source| source.active && expected_bees.contains(&source.authored_index))
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_bees, expected_bees);
    let activated_from_producer = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Level2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group: 29,
                authored_indices,
                ..
            } if *source_slot == producer_slot => authored_indices.first().copied(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(activated_from_producer, vec![307, 308, 312]);

    session.reset();
    assert!(
        session
            .economy()
            .authored_bees()
            .iter()
            .all(|source| !expected_bees.contains(&source.authored_index))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_group23_model59_materializes_from_rooted_production_turns() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-23 model-59 session");
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
    session.state.position = feature_position(342);
    let mut activated = false;
    for _ in 0..8 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
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
    assert!(activated, "row 342 must activate group 23 in production");

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
        session.canonical_terrain().ground_height(checkpoint_position)
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
        (10, 13, 13, 32, 31, 67, 0x0002_0000)
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
            Level2DispatchEvent::DynamicArena {
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-26 model-39 session");
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
    let expected_group26_indices = [357_u16, 358, 359, 360];

    // Rebind one real imported model-0 root to the authored group-25
    // source position. The callback, arena allocation, and downstream
    // group-26 switch still execute through ordinary outer turns.
    let root_position = feature_position(355);
    let root = session
        .level2_imported_switch_roots
        .iter_mut()
        .find(|root| root.authored_index == 11 && root.model == 0)
        .expect("imported model-0 root for production chain");
    root.position = root_position;
    root.target_group = 25;
    root.cooldown = 0;
    session.state.position = root_position;

    let mut group25_activated = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        group25_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
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
    session.state.position = row356_position;
    let mut group26_activated = false;
    let mut group26_owner_mana_global_at_boundary = None;
    let mut group26_owner_mana_global_after = None;
    for _ in 0..16 {
        let (_, owner_mana_global_at_boundary) =
            level2_owner_mana_projection(&session.economy().owner_mana_rows);
        session.step_outer_turn(PlayerCommand::default(), 0);
        let activated_this_turn = session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group,
                    authored_indices,
                    ..
                } if *group == 26 && authored_indices.as_ref() == expected_group26_indices
            )
        });
        if activated_this_turn {
            group26_owner_mana_global_at_boundary = Some(owner_mana_global_at_boundary);
            group26_owner_mana_global_after = Some(session.economy().owner_mana_global());
        }
        group26_activated |= activated_this_turn;
        if expected_group26_indices.iter().all(|index| {
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
    let expected_positions = expected_group26_indices
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

    // The generic class-11 producer bridge runs at the next source-pass
    // boundary after row 356's switch callback. Keep this follow-up
    // turn outside the model-39 sphere assertions above so those checks
    // remain about the direct group-26 allocation turn.
    let model13_indices = BTreeSet::from([365_u16, 366, 367, 368]);
    let mut model13_activated = BTreeSet::new();
    for _ in 0..4 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        for event in session.dispatch_events() {
            if let Level2DispatchEvent::AuthoredGroupActivated {
                group: 26,
                authored_indices,
                ..
            } = event
            {
                if let [authored_index] = authored_indices.as_ref() {
                    if model13_indices.contains(authored_index) {
                        model13_activated.insert(*authored_index);
                    }
                }
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-24 model-39/model-6 session");
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
    session.state.position = root_position;
    let mut group23_activated = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        group23_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
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
    session.state.position = feature_position(344);
    let mut group24_activated = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        group24_activated |= session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
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
    assert!(
        expected_group24_indices
            .iter()
            .all(
                |index| session.dispatch_events().iter().any(|event| matches!(
                    event,
                    Level2DispatchEvent::AuthoredGroupActivated {
                        group: 24,
                        authored_indices,
                        ..
                    } if authored_indices.as_ref().contains(index)
                ))
            )
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
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-25 production session");
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
    // row 355 is model-1 at the adjacent authored tile, so its inverse
    // overlap fires at its live source cadence and selects group 25/row
    // 356.
    let row344_position = feature(344).world_position(0);
    session
        .set_level2_entity_position(local_player, row344_position)
        .expect("public player-position input for row 344");
    let mut group25_activated = false;
    let saw_group25_activation = |session: &Level2ResetSession| {
        session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
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
        let row355_phase = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.authored_index == 355)
            .map(|source| source.phase)
            .expect("row 355 producer remains live before its cadence");
        for _ in 0..level_two_switch_turns_until_cadence(row355_phase) {
            session
                .set_level2_entity_position(local_player, row344_position)
                .expect("public player-position input remains on row 344");
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
    assert!(row355.drawing_enabled);

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
            session.canonical_terrain()
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
