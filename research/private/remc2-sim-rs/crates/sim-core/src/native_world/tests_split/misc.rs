#[test]
fn level_two_qualified_area_exit_hides_after_inverse_overlap_success() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let drive_exit = |session: &mut Level2ResetSession| {
        let feature = session
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == 41)
            .copied()
            .expect("qualified AreaExit row41");
        assert_eq!(
            (feature.kind, feature.subtype, feature.stage_tag),
            (11, 1, 2)
        );
        let terrain = session.canonical_terrain().clone();
        let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
        let root = session
            .economy_mut()
            .allocate_authored_switch_producer(
                feature.index,
                1,
                2,
                i16::try_from(feature.word_10).expect("source overlap extent"),
                position,
                &terrain,
            )
            .expect("real qualified AreaExit constructor");
        let generation = session.economy().slot_generation(root.source_entity_index);
        assert!(generation > 0);
        assert_eq!((root.source_class, root.model, root.action), (11, 1, 1));
        // An ordinary overlapping visit must leave the real inverse trigger live.
        for _ in 0..level_two_switch_turns_until_cadence(root.phase) {
            session
                .set_level2_entity_position(session.state.entity_index, root.position)
                .expect("authoritative overlapping player carrier");
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                !session.dispatch_events().iter().any(|event| matches!(event,
                Level2DispatchEvent::AuthoredGroupActivated { source_slot, group: 2, .. }
                    if *source_slot == root.source_entity_index))
            );
        }
        let overlapping = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.source_entity_index == root.source_entity_index)
            .copied()
            .expect("overlap retains the actual AreaExit occurrence");
        assert!(!overlapping.fired && overlapping.drawing_enabled);
        let mut exit_position = root.position;
        exit_position.x = exit_position.x.wrapping_add(0x8000);
        let mut receipts = Vec::new();
        for _ in 0..16 {
            session
                .set_level2_entity_position(session.state.entity_index, exit_position)
                .expect("authoritative player leaves the inverse-overlap extent");
            session.step_outer_turn(PlayerCommand::default(), 0);
            receipts.extend_from_slice(session.dispatch_events());
            if session.dispatch_events().iter().any(|event| {
                matches!(event,
                Level2DispatchEvent::AuthoredGroupActivated { source_slot, group: 2, .. }
                    if *source_slot == root.source_entity_index)
            }) {
                break;
            }
        }
        assert_eq!(
            receipts
                .iter()
                .filter(|event| matches!(event,
            Level2DispatchEvent::AuthoredGroupActivated { source_slot, group: 2, .. }
                if *source_slot == root.source_entity_index))
                .count(),
            1,
            "AREA_EXIT_REFUSAL root_before={root:?} root_after={:?} gen={generation}->{} player={:?} model0={:?} errors={:?} root_schedule={:?}",
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .find(|source| source.source_entity_index == root.source_entity_index),
            session.economy().slot_generation(root.source_entity_index),
            session.state,
            session
                .entities()
                .iter()
                .filter(|entity| entity.active && entity.class == 3 && entity.model == 0)
                .map(|entity| (
                    entity.index,
                    entity.life,
                    entity.position,
                    i16::from_le_bytes(entity.record_bytes()[88..90].try_into().expect("width")),
                    i16::from_le_bytes(entity.record_bytes()[90..92].try_into().expect("depth"))
                ))
                .collect::<Vec<_>>(),
            receipts
                .iter()
                .filter(|event| matches!(event, Level2DispatchEvent::Error { .. }))
                .take(8)
                .collect::<Vec<_>>(),
            session
                .cf10_production_schedule_receipts()
                .iter()
                .filter(|receipt| matches!(receipt,
                crate::campaign::schedule::ScheduleReceipt::SlotResolved { slot, .. }
                | crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                | crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. }
                    if *slot == root.source_entity_index))
                .collect::<Vec<_>>()
        );
        let hidden = session
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|source| source.source_entity_index == root.source_entity_index)
            .expect("AreaExit remains through its callback pass");
        assert!(hidden.fired && !hidden.drawing_enabled);
        assert_eq!(
            session.economy().slot_generation(root.source_entity_index),
            generation
        );
        assert!(
            session
                .economy()
                .map_chains()
                .is_linked(root.source_entity_index)
        );
        assert!(session.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, selector, .. }
                if *slot == root.source_entity_index
                    && selector.registry_key().selector == crate::campaign::spec::BehaviorSelector::ClassAction {
                        class: 11, action: 1,
                    })));
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|source| source.source_entity_index == root.source_entity_index)
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .is_linked(root.source_entity_index)
        );
        assert!(
            !session.dispatch_events().iter().any(|event| matches!(event,
            Level2DispatchEvent::AuthoredGroupActivated { source_slot, group: 2, .. }
                if *source_slot == root.source_entity_index))
        );
        receipts
    };
    let first_receipts = drive_exit(&mut session);
    let first_hash = session.campaign_state_hash();
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(drive_exit(&mut session), first_receipts);
    assert_eq!(session.campaign_state_hash(), first_hash);
}

#[test]
fn level_two_model76_producers_activate_rows_116_120_124_only() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    assert!(session.economy().authored_model76().is_empty());
    let mut source_trace = Vec::new();
    for group in [12_u16, 14, 16] {
        let root = session
            .level2_imported_switch_roots
            .iter()
            .find(|root| root.target_group == group)
            .copied()
            .expect("package model-0 producer for each model-76 group");
        assert_eq!(root.model, 0);
        for _ in 0..level_two_switch_turns_until_cadence(root.phase) {
            session
                .set_level2_entity_position(session.state.entity_index, root.position)
                .expect("public player input enters the actual source producer overlap");
            session.step_outer_turn(PlayerCommand::default(), 0);
            source_trace.extend_from_slice(session.dispatch_events());
        }
    }

    let authored = session
        .economy()
        .authored_model76()
        .iter()
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(authored, BTreeSet::from([116_u16, 120, 124]));
    assert!(!source_trace.iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated { group: 10, .. }
    )));
    assert!(source_trace.iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 12,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [116]
    )));
    assert!(source_trace.iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 14,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [120]
    )));
    assert!(source_trace.iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 16,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [124]
    )));
    assert!(source_trace.iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel76,
            ..
        }
    )));
    assert!(
        !source_trace.iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Economy {
                source_slot: 25,
                event: Level2EconomyEvent::PreparedSound { sound_id: 41, .. },
                ..
            }
        )),
        "source model-0 switches activate silently"
    );

    session.reset();
    assert!(session.economy().authored_model76().is_empty());
    assert!(session.economy().model76_children().is_empty());
    assert!(session.economy().model76_tail_spawns().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_model76_switch25_stays_latent_without_overlap() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 switch25 latent session");
    let switch = session
        .level2_group10_switch25
        .expect("live group-10 switch slot 25");
    assert_eq!(switch.source_slot, 25);
    assert_eq!(switch.phase, 26);
    assert!(
        session
            .economy()
            .authored_model76()
            .iter()
            .all(|source| source.authored_index != 112)
    );

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(
        session
            .economy()
            .authored_model76()
            .iter()
            .all(|source| source.authored_index != 112)
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated { group: 10, .. }
    )));
    let live_switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == 25)
        .expect("live switch slot 25");
    assert!(!live_switch.drawing_disabled());
}

#[test]
fn level_two_model76_switch25_rejects_far_and_strict_boundary_positions() {
    let mut far = level_two_campaign_session();
    let switch = far
        .level2_group10_switch25
        .expect("live group-10 switch slot 25");
    let activation_turns = level_two_model76_switch25_activation_turns(switch.phase);
    far.set_level2_entity_position(
        far.state.entity_index,
        Position {
            x: switch.position.x.wrapping_add(4_096),
            y: switch.position.y,
            z: switch.position.z,
        },
    )
    .expect("fixture player position updates the authoritative row and map link");
    for _ in 0..activation_turns {
        far.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(
        far.economy()
            .authored_model76()
            .iter()
            .all(|source| source.authored_index != 112)
    );
    assert!(!far.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated { group: 10, .. }
    )));

    let mut boundary = level_two_campaign_session();
    let switch = boundary
        .level2_group10_switch25
        .expect("live group-10 switch slot 25");
    let activation_turns = level_two_model76_switch25_activation_turns(switch.phase);
    let switch_half_depth = i16::from_le_bytes(
        boundary.economy.entities()[usize::from(switch.source_slot)].record_bytes()[90..92]
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
                x: switch.position.x,
                y: switch.position.y.wrapping_add(boundary_distance),
                z: switch.position.z,
            },
        )
        .expect("fixture player position updates the authoritative row and map link");
    for _ in 0..activation_turns {
        boundary.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(
        boundary
            .economy()
            .authored_model76()
            .iter()
            .all(|source| source.authored_index != 112)
    );
    assert!(!boundary.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated { group: 10, .. }
    )));
}

#[test]
fn level_two_model76_switch25_activates_row112_once_and_enters_cursor() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 switch25 positive session");
    let switch = session
        .level2_group10_switch25
        .expect("live group-10 switch slot 25");
    let activation_turns = level_two_model76_switch25_activation_turns(switch.phase);
    assert_eq!(switch.phase, 26);
    session.state.position = switch.position;

    for _ in 0..activation_turns {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }

    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            source_slot,
            group,
            authored_indices,
            ..
        } if *source_slot == 25 && *group == 10 && authored_indices.as_ref() == [112]
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel76,
            ..
        }
    )));
    assert!(
        session
            .economy()
            .authored_model76()
            .iter()
            .any(|source| source.authored_index == 112),
        "slot 25 must materialize row 112 on overlap"
    );
    assert!(session.level2_group10_switch25.is_none());
    let live_switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == 25)
        .expect("live switch slot 25");
    assert!(live_switch.drawing_disabled());

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated { group: 10, .. }
            ))
            .count(),
        0,
        "slot 25 must not emit a second group-10 activation"
    );
}

#[test]
fn level_two_model76_switch25_allocation_failure_consumes_row_and_hides() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 switch25 exhaustion session");
    let switch = session
        .level2_group10_switch25
        .expect("live group-10 switch slot 25");
    let activation_turns = level_two_model76_switch25_activation_turns(switch.phase);
    let crafted_free_slots = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 0, 906, 905, 904, 903,
        902, 901,
    ];
    session.economy_mut().free_slots = crafted_free_slots.clone();
    for entity in session.economy.entities.iter_mut().filter(|entity| {
        entity.active
            && entity.class == 11
            && entity.model == 0
            && matches!(entity.source_id, 12 | 14 | 16)
    }) {
        entity.disable_drawing();
    }
    session.state.position = switch.position;
    for _ in 0..activation_turns {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }

    assert!(session.economy().authored_model76().is_empty());
    assert!(session.economy().model76_children().is_empty());
    assert!(session.economy().model76_tail_spawns().is_empty());
    assert!(
        session
            .session
            .state
            .live
            .authored_features_consumed
            .contains(&112),
        "sub_4A1E0 consumes row 112 even when its constructor cannot allocate"
    );
    assert!(session.level2_group10_switch25.is_none());
    let hidden_switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == 25)
        .expect("reserved switch slot 25");
    assert!(hidden_switch.drawing_disabled());
    assert!(hidden_switch.active);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            source_slot: 25,
            group: 10,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [112]
    )));
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Error {
            source_slot: 25,
            ..
        }
    )));

    session.step_outer_turn(PlayerCommand::default(), 0);

    let reclaimed_switch = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == 25)
        .expect("reclaimed switch row 25");
    assert!(!reclaimed_switch.active);
    assert_eq!(reclaimed_switch.class, 0);
    assert!(session.economy().authored_model76().is_empty());
    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter(|event| matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated { group: 10, .. }
            ))
            .count(),
        0,
        "consumed row 112 must not retry on a later pass"
    );
}

#[test]
fn level_two_model76_switch25_reset_restores_hash_and_live_switch_state() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 switch25 reset session");
    let baseline_hash = session.campaign_state_hash();
    let switch = session
        .level2_group10_switch25
        .expect("live group-10 switch slot 25");
    let activation_turns = level_two_model76_switch25_activation_turns(switch.phase);
    session.state.position = switch.position;

    for _ in 0..activation_turns {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }

    assert!(session.level2_group10_switch25.is_none());
    assert!(
        session
            .economy()
            .authored_model76()
            .iter()
            .any(|source| source.authored_index == 112)
    );

    session.reset();

    let switch = session
        .level2_group10_switch25
        .expect("reset live group-10 switch slot 25");
    assert_eq!(switch.source_slot, 25);
    assert_eq!(switch.phase, 26);
    assert!(session.economy().authored_model76().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_model76_builds_full_cluster_then_spawns_tail_on_expiry() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 constructor session");
    let baseline_hash = session.campaign_state_hash();

    let source = session
        .allocate_level2_authored_model76(112)
        .expect("authored model-76 row 112");
    assert_eq!(
        (
            source.entity_id,
            source.source_class,
            source.model,
            source.action,
            source.max_life,
            source.life,
            source.subspell_index,
            source.actual_speed,
            source.minimum_speed,
            source.maximum_speed,
            source.word_0x2c,
            source.font_type_index,
        ),
        (
            source.source_entity_index,
            10,
            76,
            83,
            80,
            80,
            70,
            40,
            480,
            192,
            192,
            18,
        )
    );
    assert_eq!((source.shift_rot_x, source.shift_rot_y), (640, 640));
    assert_eq!(source.target_entity_index, 0);
    assert_eq!(session.economy().model76_children().len(), 25);
    assert!(
        session
            .economy()
            .model76_children()
            .iter()
            .enumerate()
            .all(|(index, child)| {
                child.parent_entity_index == source.source_entity_index
                    && child.parent_entity_id == source.entity_id
                    && child.entity_id == source.entity_id
                    && child.chain_index == u8::try_from(index).expect("index fits")
                    && child.row_index == u8::try_from(index / 5).expect("row fits")
                    && child.column_index == u8::try_from(index % 5).expect("column fits")
                    && (child.source_class, child.model, child.action) == (10, 77, 84)
            })
    );

    let first_child = session.economy().model76_children()[0];
    session
        .economy_mut()
        .authored_model76
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .expect("model-76 root for callback")
        .life = 1;
    assert!(
        session
            .economy_mut()
            .step_authored_model76_slot(source.source_entity_index)
    );
    let stepped = session
        .economy()
        .authored_model76()
        .iter()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .copied()
        .expect("model-76 root survives first callback");
    assert_eq!(stepped.life, 0);
    assert_eq!(stepped.state, 2);
    assert_eq!(stepped.phase, source.phase.wrapping_add(1));
    assert_ne!(
        session
            .economy()
            .model76_children()
            .iter()
            .find(|candidate| candidate.source_entity_index == first_child.source_entity_index)
            .expect("linked child remains live")
            .position,
        first_child.position
    );

    let mut economy_view = session.economy_mut();
    let root = economy_view
        .authored_model76
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .expect("model-76 root for terminal countdown");
    root.word_0x2c = 8;
    root.font_type_index = 18;
    root.state = 2;
    assert!(
        session
            .economy_mut()
            .step_authored_model76_slot(source.source_entity_index)
    );
    assert_eq!(session.economy().model76_tail_spawns().len(), 1);
    let tail = session.economy().model76_tail_spawns()[0];
    assert_eq!(
        (
            tail.parent_entity_index,
            tail.parent_entity_id,
            tail.source_class,
            tail.model,
            tail.action,
            tail.subspell_index,
            tail.max_life,
            tail.life,
            tail.shift_rot_x,
            tail.shift_rot_y,
            tail.particle_index,
        ),
        (
            source.source_entity_index,
            source.entity_id,
            10,
            0,
            0,
            400,
            8,
            8,
            128,
            128,
            7,
        )
    );
    assert!(
        session
            .economy()
            .authored_model76()
            .iter()
            .find(|candidate| candidate.source_entity_index == source.source_entity_index)
            .is_some_and(|candidate| !candidate.active)
    );
    assert!(
        session
            .economy()
            .model76_children()
            .iter()
            .all(|child| !child.active)
    );
    session.economy_mut().reclaim_inactive_slots();
    assert!(
        session
            .economy()
            .authored_model76()
            .iter()
            .all(|candidate| candidate.source_entity_index != source.source_entity_index)
    );
    assert!(session.economy().model76_children().is_empty());
    assert!(session.economy().live_arena_slots().contains(&(
        tail.source_entity_index,
        Level2DynamicArenaKind::Model76TailSpawn
    )));

    session.reset();
    assert!(session.economy().authored_model76().is_empty());
    assert!(session.economy().model76_children().is_empty());
    assert!(session.economy().model76_tail_spawns().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_production_model76_terminal_return_advances_the_retained_raw_phase() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("production model-76 terminal session");
    assert!(
        !session
            .step_outer_turn(PlayerCommand::default(), 0)
            .is_empty(),
        "prime the qualified one-shot authored activations"
    );
    let source = session
        .allocate_level2_authored_model76(112)
        .expect("production model-76 root");
    {
        let mut economy = session.economy_mut();
        let root = economy
            .authored_model76
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == source.source_entity_index)
            .expect("model-76 terminal root");
        root.state = 2;
        root.word_0x2c = 8;
        root.font_type_index = 18;
    }
    let phase_before = source.phase;

    let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !evidence.is_empty(),
        "production model-76 terminal pass failed: {:?}",
        session.dispatch_events()
    );
    let retired = session
        .economy()
        .authored_model76()
        .iter()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .expect("model-76 tombstone remains through its callback pass");
    assert!(!retired.active);
    assert_eq!(
        retired.phase,
        phase_before.wrapping_add(1),
        "UpdateEntities advances byte 0x3E after the terminal callback disables the row"
    );
    assert!(
        session
            .cf10_production_schedule_receipts()
            .iter()
            .any(|receipt| matches!(
                receipt,
                crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                    slot,
                    original,
                    current: crate::campaign::schedule::CurrentOccupant::Inactive,
                    ..
                } if *slot == source.source_entity_index
                    && original.registry_key().selector
                        == crate::campaign::spec::BehaviorSelector::ClassAction {
                            class: 10,
                            action: 83,
                        }
            ))
    );
}

#[test]
fn level_two_model76_callback_snaps_to_live_target_cache() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 target session");
    let source = session
        .allocate_level2_authored_model76(112)
        .expect("authored model-76 row 112");
    let target_slot = session.state.entity_index;
    let target_index = usize::from(target_slot);
    let target_xy = Position {
        x: source.position.x.wrapping_add(1_024),
        y: source.position.y.wrapping_add(2_048),
        z: 0,
    };
    let ground = session.canonical_terrain().ground_height(target_xy);
    let target_position = Position {
        z: ground.wrapping_add(512),
        ..target_xy
    };
    let target_pitch = 300_i16;
    let target_height_offset = 64_i16;
    session.economy.entities[target_index].position = target_position;
    session.economy.entities[target_index].control_flags = 0;
    session.economy.entities[target_index].life = 500;
    session.economy.entities[target_index].record[86..88]
        .copy_from_slice(&target_height_offset.to_le_bytes());
    session.economy.entities[target_index].record[88..90]
        .copy_from_slice(&target_pitch.to_le_bytes());
    {
        // `sub_4A310` leaves the authored Level 2 producer untargeted in
        // this bounded slice; seed the callback state directly so the
        // retained `sub_33C70` branch stays covered without claiming the
        // producer already supplies that relation.
        let mut economy_view = session.economy_mut();
        let root = economy_view
            .authored_model76
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == source.source_entity_index)
            .expect("model-76 root");
        root.target_entity_index = target_slot;
    }

    session.sync_level2_model76_targets_from_entities();
    assert!(
        session
            .economy_mut()
            .step_authored_model76_slot(source.source_entity_index)
    );

    let stepped = session
        .economy()
        .authored_model76()
        .iter()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .copied()
        .expect("stepped model-76 root");
    assert_eq!(stepped.state, 1);
    assert_eq!(stepped.target_entity_index, target_slot);
    assert_eq!(stepped.position.x, target_position.x);
    assert_eq!(stepped.position.y, target_position.y);
    assert_eq!(
        stepped.position.z,
        target_position.z.wrapping_add(target_height_offset)
    );
    assert_eq!(stepped.maximum_speed, 150);
    assert_eq!(stepped.minimum_speed, 450);
}

#[test]
fn level_two_model76_callback_enters_terminal_state_for_dead_target() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 dead-target session");
    let source = session
        .allocate_level2_authored_model76(112)
        .expect("authored model-76 row 112");
    let target_slot = session.state.entity_index;
    let target_index = usize::from(target_slot);
    let child_before = session
        .economy()
        .model76_children()
        .iter()
        .find(|child| child.parent_entity_index == source.source_entity_index)
        .map(|child| child.position)
        .expect("model-76 child before terminal state");
    session.economy.entities[target_index].life = -1;
    {
        let mut economy_view = session.economy_mut();
        let root = economy_view
            .authored_model76
            .iter_mut()
            .find(|candidate| candidate.source_entity_index == source.source_entity_index)
            .expect("model-76 root");
        root.target_entity_index = target_slot;
    }

    session.sync_level2_model76_targets_from_entities();
    assert!(
        session
            .economy_mut()
            .step_authored_model76_slot(source.source_entity_index)
    );

    let stepped = session
        .economy()
        .authored_model76()
        .iter()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .copied()
        .expect("terminal model-76 root");
    assert_eq!(stepped.state, 2);
    assert_eq!(stepped.word_0x2c, 174);
    let child_after = session
        .economy()
        .model76_children()
        .iter()
        .find(|child| child.parent_entity_index == source.source_entity_index)
        .map(|child| child.position)
        .expect("model-76 child after terminal state");
    assert_eq!(child_after, child_before);
}

#[test]
fn level_two_model76_generic_collision_callback_is_exact() {
    // The model-45 branch consumes the qualified bank with the source's
    // second VGA half-size and effective-width stride. This synthetic
    // carrier isolates that arithmetic; package admission separately pins
    // the real carrier byte identity.
    let mask_bank = |occupied: bool| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MCM1");
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&12_u32.to_le_bytes());
        bytes.extend_from_slice(&64_u16.to_le_bytes());
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&5_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        let mut pixels = [-1_i16; 20];
        if occupied {
            pixels[3] = 0;
        }
        for pixel in pixels {
            bytes.extend_from_slice(&pixel.to_le_bytes());
        }
        CollisionMaskBank::parse(&bytes).expect("bounded model-45 mask")
    };
    let mask_probe_position = Position {
        x: 10 << 8,
        y: 10 << 8,
        z: 0,
    };
    assert!(level2_model76_model45_occupancy(
        &mask_bank(true),
        64,
        mask_probe_position,
    ));
    assert!(!level2_model76_model45_occupancy(
        &mask_bank(false),
        64,
        mask_probe_position,
    ));

    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");

    // A production allocator with no admitted target must not invent a
    // mailbox write or sound. The v4 package correctly supplies model-45
    // authority, so make target absence explicit instead of relying on the
    // historical v3 carrier omission.
    let mut no_collision = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 no-collision session");
    let no_collision_root = no_collision
        .allocate_level2_authored_model76(112)
        .expect("authored model-76 no-collision root");
    no_collision
        .level2_model76_activated
        .extend([116_u16, 120, 124]);
    for entity in &mut no_collision.economy.entities {
        entity.active = false;
    }
    let no_collision_rng = no_collision
        .economy()
        .model76_children()
        .iter()
        .filter(|child| child.parent_entity_index == no_collision_root.source_entity_index)
        .map(|child| (child.source_entity_index, child.entity_rng))
        .collect::<Vec<_>>();
    no_collision.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !no_collision.economy().events().iter().any(|event| {
            matches!(
                event,
                Level2EconomyEvent::FireSphereCollisionSound {
                    source_entity_index,
                    ..
                } if *source_entity_index == no_collision_root.source_entity_index
            )
        }),
        "no-collision root emitted events: {:?}",
        no_collision.economy().events()
    );
    assert_eq!(
        no_collision_rng,
        no_collision
            .economy()
            .model76_children()
            .iter()
            .filter(|child| { child.parent_entity_index == no_collision_root.source_entity_index })
            .map(|child| (child.source_entity_index, child.entity_rng))
            .collect::<Vec<_>>()
    );

    // Reach row 112 through its ordinary imported switch. The local
    // class-3/model-0 player is a genuine generic-map target, not the
    // already-covered model-2 diagnostic branch.
    let run_positive =
        || {
            let mut session = inspection
                .level2_reset_session()
                .expect("Level 2 model-76 generic collision session");
            let baseline_hash = session.campaign_state_hash();
            let switch = session
                .level2_group10_switch25
                .expect("live group-10 switch slot 25");
            let local_player = session.state.entity_index;
            assert_eq!(
                (
                    session.economy.entities[usize::from(local_player)].class,
                    session.economy.entities[usize::from(local_player)].model
                ),
                (3, 0)
            );
            session
                .set_level2_entity_position(local_player, switch.position)
                .expect("move local player through the public Level 2 carrier");
            for _ in 0..level_two_model76_switch25_activation_turns(switch.phase) {
                session.step_outer_turn(PlayerCommand::default(), 0);
            }
            let root = session
                .economy()
                .authored_model76()
                .iter()
                .find(|source| source.authored_index == 112)
                .copied()
                .expect("switch production route materializes row 112");
            let mut preview = session.clone();
            preview.step_outer_turn(PlayerCommand::default(), 0);
            let positive_position = preview
                .economy()
                .model76_children()
                .iter()
                .find(|child| {
                    child.parent_entity_index == root.source_entity_index && child.column_index == 0
                })
                .map(|child| child.position)
                .expect("preview retains a column-zero child");
            session
                .set_level2_entity_position(local_player, positive_position)
                .expect("move local player to the deterministic collision point");
            let target_before = session.economy.entities[usize::from(local_player)].clone();
            session.step_outer_turn(PlayerCommand::default(), 0);
            let root = session
                .economy()
                .authored_model76()
                .iter()
                .find(|source| source.authored_index == 112)
                .copied()
                .expect("row 112 remains live for the collision turn");
            let target = &session.economy.entities[usize::from(local_player)];
            let payload_delta = target
                .source_wind_spell_payload()
                .wrapping_sub(target_before.source_wind_spell_payload());
            assert!(payload_delta > 0);
            assert_eq!(payload_delta % i32::from(root.subspell_index), 0);
            let hit_children = usize::try_from(payload_delta / i32::from(root.subspell_index))
                .expect("positive model-76 hit count fits usize");
            assert_eq!(target.source_wind_spell_owner(), root.entity_id);
            assert_eq!(target.entity_rng, target_before.entity_rng);
            assert_eq!(target.class, 3);
            assert_eq!(target.model, 0);

            let sounds = session
                .dispatch_events()
                .iter()
                .filter_map(|event| match event {
                    Level2DispatchEvent::Economy {
                        source_slot,
                        event:
                            Level2EconomyEvent::FireSphereCollisionSound {
                                source_entity_index,
                                sound_id,
                            },
                        ..
                    } if *source_slot == root.source_entity_index
                        && *source_entity_index == root.source_entity_index =>
                    {
                        Some((*source_slot, *source_entity_index, *sound_id))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            // The local-player payload counts that one target's hits. Sound 3
            // is broader: source sub_33C00 emits once for every column-zero
            // child that hits any admitted target, including v4 model-45
            // targets now backed by the qualified collision-mask carrier.
            let sound_children = session
                .economy()
                .model76_children()
                .iter()
                .filter(|child| {
                    child.parent_entity_index == root.source_entity_index && child.column_index == 0
                })
                .count();
            assert_eq!(sounds.len(), sound_children);
            assert!(sounds.len() >= hit_children);
            assert!(sounds.iter().all(|sound| {
                *sound == (root.source_entity_index, root.source_entity_index, 3)
            }));
            let state_hash = session.campaign_state_hash();
            let event_hash = session.dispatch_observability_hash();
            session.reset();
            assert_eq!(session.campaign_state_hash(), baseline_hash);
            assert!(session.economy().authored_model76().is_empty());
            (state_hash, event_hash, hit_children)
        };

    let first = run_positive();
    let replay = run_positive();
    assert_eq!(first, replay);
    assert_eq!(first.2, 1);
}

#[test]
fn level_two_model76_column_zero_owner_reaches_model2_mailbox_on_production_turn() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 collision session");
    let baseline_hash = session.campaign_state_hash();
    let source = session
        .allocate_level2_authored_model76(112)
        .expect("authored model-76 row 112");
    // The qualified reset does not carry a live model-2 target. Reuse the
    // local class-3 player row as an explicit diagnostic fixture for the
    // source's model-2 mailbox branch; the cursor and owner code remain
    // the production path, while reset reachability stays a residual.
    let target_slot = session.state.entity_index;
    let target_index = usize::from(target_slot);
    session.economy.entities[target_index].model = 2;
    session.economy.entities[target_index].active = true;
    session.economy.entities[target_index].life = 500;
    let target = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == target_slot)
        .expect("model-2 target");
    assert_eq!((target.class, target.model), (3, 2));
    let first_column_zero = session
        .economy()
        .model76_children()
        .iter()
        .find(|child| child.parent_entity_index == source.source_entity_index)
        .copied()
        .expect("model-76 column-zero child");
    assert!(first_column_zero.column_index == 0);
    assert!(first_column_zero.collision_pitch > 0);
    assert!(first_column_zero.collision_fov > 0);
    let particle = inspection
        .presentation()
        .particles
        .get(340)
        .expect("source SetEntityIndexAndRot particle 340");
    assert_eq!((particle.speed, particle.rotation_speed), (97, 100));
    let expected_axes = (
        i16::try_from(particle.rotation_speed / 2).expect("source collision yaw"),
        i16::try_from(particle.speed / 2).expect("source collision pitch"),
        i16::try_from(particle.speed / 2).expect("source collision roll"),
        i16::try_from(particle.rotation_speed / 2).expect("source collision FOV"),
    );
    assert_eq!(expected_axes, (50, 48, 48, 50));
    assert!(
        session
            .economy()
            .model76_children()
            .iter()
            .filter(|child| child.parent_entity_index == source.source_entity_index)
            .all(|child| (
                child.collision_yaw,
                child.collision_pitch,
                child.collision_roll,
                child.collision_fov,
            ) == expected_axes),
        "every constructed child retains the actual source particle axes in release builds"
    );
    // Keep the production turn focused on the explicitly allocated root;
    // the reset's independent class-11 producer rows would otherwise
    // activate their own model-76 families on this same pass.
    session
        .level2_model76_activated
        .extend([116_u16, 120_u16, 124_u16]);
    let expected_hits = session
        .economy()
        .model76_children()
        .iter()
        .filter(|child| child.column_index == 0)
        .count();

    // Use a state-1 row with a one-tick life so the production callback
    // moves the child, probes collision, then transitions the root to
    // state 2. Widen only this diagnostic target's axes so the test does
    // not depend on an unqualified reset target's orbit position.
    session
        .set_level2_entity_position(session.state.entity_index, first_column_zero.position)
        .expect("fixture player position updates the authoritative row and map link");
    session
        .set_level2_entity_position(target_slot, first_column_zero.position)
        .expect("model-2 target position");
    session.economy.entities[target_index].record[86..88].copy_from_slice(&0_i16.to_le_bytes());
    session.economy.entities[target_index].record[88..90].copy_from_slice(&i16::MAX.to_le_bytes());
    session.economy.entities[target_index].record[90..92].copy_from_slice(&i16::MAX.to_le_bytes());
    session.economy.entities[target_index].record[92..94].copy_from_slice(&i16::MAX.to_le_bytes());
    session.economy.entities[target_index].record[98..102].copy_from_slice(&9_i32.to_le_bytes());
    session.economy.entities[target_index].record[102..104].copy_from_slice(&7_u16.to_le_bytes());
    session
        .economy_mut()
        .authored_model76
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .expect("model-76 root")
        .life = 1;

    session.step_outer_turn(PlayerCommand::default(), 0);

    let target = session
        .economy
        .entities()
        .iter()
        .find(|entity| entity.index == target_slot)
        .expect("model-2 target after owner callback");
    assert_eq!(
        target.source_wind_spell_payload(),
        9 + 70 * expected_hits as i32
    );
    assert_eq!(
        target.source_wind_spell_owner(),
        first_column_zero.entity_id
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredModel76,
            source_slot,
            ..
        } if *source_slot == source.source_entity_index
    )));

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert!(session.economy().authored_model76().is_empty());
    assert!(session.economy().model76_children().is_empty());
}

#[test]
fn level_two_model76_allocation_failure_rolls_back_partial_family() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-76 rollback session");
    let baseline_live = session.economy().live_arena_slots();
    let crafted_free_slots = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 0, 906, 905, 904, 903,
        902, 901,
    ];
    session.economy_mut().free_slots = crafted_free_slots.clone();

    assert!(session.allocate_level2_authored_model76(112).is_none());
    assert!(session.economy().authored_model76().is_empty());
    assert!(session.economy().model76_children().is_empty());
    assert!(session.economy().model76_tail_spawns().is_empty());
    assert_eq!(session.economy().free_slots, crafted_free_slots);
    assert_eq!(session.economy().live_arena_slots(), baseline_live);
}

#[test]
fn level_two_shield_active_holder_ticks_through_outer_turn_and_reset() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Shield production session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let baseline_hash = session.shared_spells().reset_hash();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 155)
        .copied()
        .expect("authored family-6 source");
    session.state.position = source.position;
    let acquired = (0..4).find_map(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 6,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = acquired.expect("authored family-6 acquisition");
    let cold = session
        .shared_spells()
        .shield_state()
        .expect("family-6 acquisition seeds Shield state");
    assert_eq!(cold.holder_entity_index, outcome.source_slot);
    assert_eq!(cold.active_value, 0);
    // Player slot 111 activates the selected holder before the later
    // dynamic family-6 source row. The same outer pass therefore includes
    // Shield's first active pulse rather than deferring it one turn.
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 6,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        0,
    );
    let active = session
        .shared_spells()
        .shield_state()
        .expect("selected family-6 Shield state");
    assert_eq!(active.active_value, active.activation_value - 1);
    assert_ne!(session.shared_spells().reset_hash(), baseline_hash);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let stepped = session
        .shared_spells()
        .shield_state()
        .expect("Shield state after outer source turn");
    assert_eq!(stepped.active_value, active.active_value - 1);
    assert_ne!(
        stepped.parent_status_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
        0
    );

    session.reset();
    assert!(session.shared_spells().shield_state().is_none());
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
}

#[test]
fn level_two_fireball_tiers_reach_actor_hit_experience_and_fire_spheres() {
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-level2-vault-successor-environment-20261025");
    for tier in 0..=2 {
        let mut session = level_two_campaign_session_with_owner_cache(&gameplay_cache);
        fire_level_two_model32_group(&mut session, 21);
        let bee_slot = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|bee| bee.authored_index() == Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX))
            .unwrap()
            .source_entity_index;
        let player = session.state.entity_index;
        session
            .shared_spells_mut()
            .award_spell_experience(player, 0, 5_000);
        let castle = session
            .economy_mut()
            .build_castle_at(
                Position::default(),
                Position::default(),
                player,
                false,
                true,
            )
            .expect("ordinary Castle constructor");
        session.economy_mut().castle.as_mut().unwrap().mana = 10_000;
        assert_eq!(session.economy().player_castle_entity_index(), castle);
        {
            let mut shared = session.shared_spells_mut();
            shared.player_entity.set_mana(10_000);
            shared.player_entities[0] = shared.player_entity.clone();
            shared.caster.mana = 10_000;
            shared.player_mana = 10_000;
        }
        session.economy.entities[usize::from(player)].set_mana(10_000);
        let ui_index = crate::SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|family| *family == 0)
            .unwrap() as i8;
        session.step_outer_turn(
            PlayerCommand {
                opcode: 31,
                argument_1: ui_index,
                argument_2: tier,
                ..PlayerCommand::default()
            },
            0,
        );
        let bee = drive_level_two_bee_to_action17_for_fireball(&mut session, bee_slot);
        session
            .set_level2_entity_position(
                player,
                Position {
                    x: bee.position.x.wrapping_add(256),
                    y: bee.position.y.wrapping_add(384),
                    z: bee.position.z.wrapping_sub(session.state.entity_fov),
                },
            )
            .unwrap();
        session.state.yaw = 0;
        session.state.display_pitch = 0;
        session.state.effective_pitch = 0;
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        let experience_before = session.shared_spells().spell_experience()[0];
        let cast_evidence = session.step_outer_turn(
            PlayerCommand {
                flags: 0x10,
                ..PlayerCommand::default()
            },
            0,
        );
        assert_eq!(
            cast_evidence.len(),
            1,
            "Fireball cast commits: {:?}",
            session.dispatch_events()
        );
        let flyer = session
            .economy()
            .fireball_projectiles()
            .iter()
            .find(|row| row.active && row.owner_entity_index == player)
            .copied()
            .expect("ordinary hand fire constructs a projectile");
        assert_eq!(
            (flyer.source_model, flyer.action),
            if tier == 2 { (28, 29) } else { (0, 0) }
        );
        if tier == 2 {
            let mut arena = session.session.state.live.economy.clone();
            let terrain = session.canonical_terrain().clone();
            let mut probe = arena.clone();
            let collision = probe.economy.advance_fireball_projectile_slot(
                flyer.source_entity_index,
                &terrain,
                &probe.entities,
                true,
            );
            assert_eq!(collision.direct_hit_owner, Some(player));
            // AddFireSpheres needs 26 actual arena rows. Failed allocation
            // must retain the flyer and return no XP award to its dispatcher.
            arena.economy.free_slots.truncate(25);
            let mut terrain = terrain;
            let failed = crate::native_world::effects::step_projectile(
                &mut arena,
                &mut terrain,
                &session.session.definition.presentation.particles,
                flyer.source_entity_index,
            )
            .unwrap();
            assert!(failed.live);
            assert_eq!(failed.impact_slot, None);
            assert_eq!(failed.direct_hit_owner, None);
        }
        for _ in 0..16 {
            let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
            assert_eq!(
                evidence.len(),
                1,
                "Fireball target pass commits: {:?}",
                session.dispatch_events()
            );
            if session.shared_spells().spell_experience()[0] != experience_before {
                break;
            }
        }
        assert_eq!(
            session.shared_spells().spell_experience()[0],
            experience_before + 1,
            "tier {tier} actor impact awards family-zero XP after its effect allocation; flyer={:?}, events={:?}",
            session
                .economy()
                .fireball_projectiles()
                .iter()
                .find(|row| row.source_entity_index == flyer.source_entity_index),
            session
                .economy()
                .events()
                .iter()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
        );
        if tier == 2 {
            let root = session
                .economy()
                .authored_model76()
                .iter()
                .find(|row| row.entity_id == player)
                .unwrap();
            assert_eq!((root.model, root.action), (76, 83));
            assert_eq!(root.max_life, 30);
            assert_eq!(root.subspell_index, 80);
            let children = session
                .economy()
                .model76_children()
                .iter()
                .filter(|row| row.parent_entity_index == root.source_entity_index)
                .collect::<Vec<_>>();
            assert_eq!(children.len(), 25);
            assert!(children.iter().all(|row| row.entity_id == player));
        }
    }
}

#[test]
fn level_two_cold_shield_and_invisible_menu_tiers_reach_typed_holder_visit() {
    for (family, authored_index) in [(6, 155), (11, 156)] {
        let mut session = level_two_campaign_session();
        let group = session
            .mission()
            .authored_features()
            .iter()
            .find(|feature| feature.index == authored_index)
            .unwrap()
            .activation_group;
        // Stage the authored group boundary, then use its actual constructor
        // and ordinary overlap acquisition. Objective completion is separate
        // from the cold menu/profile behavior under test.
        session
            .activate_authored_group_prerequisite_for_test(u16::try_from(group).unwrap())
            .unwrap();
        let source = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == authored_index)
            .copied()
            .expect("authored group spell constructor");
        session
            .set_level2_entity_position(session.state.entity_index, source.position)
            .unwrap();
        for _ in 0..4 {
            session.step_outer_turn(PlayerCommand::default(), 0);
            if session.shared_spells().local_player().enabled_spell_holders[family] != 0 {
                break;
            }
        }
        let holder_index = session.shared_spells().local_player().enabled_spell_holders[family];
        assert_eq!(holder_index, source.source_entity_index);
        let ui_index = crate::SOURCE_SPELL_INDEX_MAP
            .iter()
            .position(|candidate| usize::from(*candidate) == family)
            .unwrap() as i8;
        for (opcode, tier) in [(31, 1), (32, 2), (31, 0)] {
            session.step_outer_turn(
                PlayerCommand {
                    opcode,
                    argument_1: ui_index,
                    argument_2: tier,
                    ..PlayerCommand::default()
                },
                0,
            );
            let profile =
                crate::spell_activation::level2_set_spell_profile(family as u8, tier as u8)
                    .unwrap();
            let shared = session.shared_spells();
            let holder = shared.pickup(holder_index).unwrap();
            assert_eq!(holder.level_gate, tier);
            assert_eq!(holder.activation_value, profile.word_0x30);
            assert_eq!(holder.max_mana, profile.max_mana);
            if family == 6 {
                let state = shared.shield_state().unwrap();
                assert_eq!(state.tier, tier as u8);
                assert_eq!(state.active_value, 0);
                assert_eq!(state.activation_value, profile.word_0x30 as i16);
                assert_eq!(state.mana_cost, profile.max_mana);
            } else {
                let state = shared.invisible_state().unwrap();
                assert_eq!(state.tier, tier as u8);
                assert_eq!(state.active_value, 0);
                assert_eq!(state.activation_value, profile.word_0x30 as i16);
                assert_eq!(state.mana_cost, profile.max_mana);
                assert_eq!(state.mana_regen_requirement, profile.mana_regen);
            }
        }
        if family == 11 {
            // The actual reward holder above now fires through the selected
            // left hand. Supply its source Castle/mana predicate explicitly;
            // this check does not stand in for earning a Castle in a mission.
            let player = session.state.entity_index;
            session
                .economy_mut()
                .build_castle_at(
                    Position::default(),
                    Position::default(),
                    player,
                    false,
                    true,
                )
                .expect("source Castle constructor");
            session.economy_mut().castle.as_mut().unwrap().mana = 5_000;
            session.economy.entities[usize::from(player)].set_mana(5_000);
            {
                let mut shared = session.shared_spells_mut();
                shared.player_entity.set_mana(5_000);
                shared.player_entities[0] = shared.player_entity.clone();
                shared.caster.mana = 5_000;
                shared.player_mana = 5_000;
            }
            let before = session.shared_spells().spell_experience()[11];
            session.step_outer_turn(
                PlayerCommand {
                    flags: 0x10,
                    ..PlayerCommand::default()
                },
                0,
            );
            assert_eq!(
                session.shared_spells().spell_experience()[11],
                before + 1,
                "fresh Invisible pulse reaches source player experience"
            );
            assert_eq!(
                session.players[0].runtime_spell_experience[11],
                before + 1,
                "canonical publication retains the awarded experience"
            );
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert_eq!(
                session.shared_spells().spell_experience()[11],
                before + 1,
                "sustained Invisible does not award a second fresh-pulse point"
            );
        }
    }
}

#[test]
fn level_three_acquired_shield_charge_reaches_player_upkeep() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level3-day-block32-campaign-recipe-v2");
    let mut campaign = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL3,
    )
    .expect("installed Level 3 descriptor");
    let player = campaign
        .level2_compatibility()
        .shared_spells()
        .local_player()
        .entity_index;
    let pickup = campaign.level2_compatibility().entities()[124].clone();
    assert_eq!((pickup.class, pickup.model, pickup.action), (15, 6, 19));
    campaign
        .level2_compatibility_mut()
        .set_level2_entity_position(player, pickup.position)
        .expect("player approaches the reset-imported Shield pickup");
    for _ in 0..4 {
        campaign.step(PlayerCommand::default(), 0);
        if campaign
            .level2_compatibility()
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
            .level2_compatibility()
            .shared_spells()
            .local_player()
            .enabled_spell_holders[6],
        pickup.index,
        "ordinary pickup callback installs the actual holder"
    );
    let ui_index = crate::SOURCE_SPELL_INDEX_MAP
        .iter()
        .position(|family| *family == 6)
        .expect("Shield menu binding") as i8;
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
            .level2_compatibility()
            .shared_spells()
            .shield_state()
            .is_some_and(|shield| shield.active_value > 0)
        {
            break;
        }
    }
    let shared = campaign.level2_compatibility().shared_spells();
    let armed = shared.shield_state().expect("acquired Shield state");
    assert!(pickup.index < player, "holder visits before its caster");
    assert_eq!(armed.active_value, armed.activation_value);
    assert_eq!(armed.tier, 0);
    let mana_before_charge = shared.player_mana();
    assert!(mana_before_charge >= armed.mana_cost);

    // The next numeric holder visit produces the negative regen mailbox;
    // the later player visit in that same pass consumes it into actual mana.
    campaign.step(PlayerCommand::default(), 0);
    let shared = campaign.level2_compatibility().shared_spells();
    let first = shared.shield_state().expect("first Shield pulse");
    assert_eq!(first.active_value, armed.active_value - 1);
    assert_eq!(shared.player_mana(), mana_before_charge - armed.mana_cost);
    assert_eq!(shared.player_entity().mana, shared.player_mana());
    assert_eq!(
        campaign.level2_compatibility().entities()[usize::from(player)].mana,
        shared.player_mana(),
        "numeric arena and HUD mana observe the consumed charge"
    );
    let mana_after_charge = shared.player_mana();
    campaign.step(PlayerCommand::default(), 0);
    assert_eq!(
        campaign
            .level2_compatibility()
            .shared_spells()
            .player_mana(),
        mana_after_charge,
        "sustained Shield suppresses recharge without charging again"
    );
}

#[test]
fn level_two_shield_damage_consumer_updates_parent_and_holder_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Shield damage session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let baseline_hash = session.shared_spells().reset_hash();

    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 155)
        .copied()
        .expect("authored family-6 source");
    session.state.position = source.position;
    let acquired = (0..4).find_map(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 6,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = acquired.expect("authored family-6 acquisition");
    assert!(session.shared_spells_mut().activate_shield(256, false));

    // The normal source route establishes the active mana-status phase
    // before the hit consumer runs; this is not a direct ShieldState test.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let active = session
        .shared_spells()
        .shield_state()
        .expect("active family-6 Shield state");
    assert_ne!(
        active.parent_status_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
        0
    );

    assert!(
        session
            .shared_spells_mut()
            .submit_shield_damage(active.parent_entity_index, 100, 152,)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    let damage = session
        .shared_spells()
        .pickup_events()
        .iter()
        .find_map(|event| match event {
            Level2SharedSpellEvent::ShieldDamageApplied { damage, .. } => Some(*damage),
            _ => None,
        })
        .expect("active source route consumes the Shield hit mailbox");
    assert_eq!(damage.raw_damage, 100);
    assert_eq!(damage.applied_damage, 25);
    assert_eq!(damage.mana_drained, 25);
    let after = session
        .shared_spells()
        .shield_state()
        .expect("Shield state remains projected after hit");
    assert_eq!(after.parent_life, active.parent_life - 25);
    assert_eq!(after.parent_mana, active.parent_mana - 25);
    assert_eq!(after.experience_awarded, active.experience_awarded + 1);
    assert_eq!(
        damage.status_after & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
        0
    );
    let holder = session
        .shared_spells()
        .dynamic_pickups()
        .iter()
        .find(|entity| entity.index == outcome.source_slot)
        .expect("active Shield holder projection");
    assert_eq!(
        holder.record_bytes()[50..52],
        after.active_value.to_le_bytes()
    );
    assert_eq!(
        holder.record_bytes()[44..46],
        after.parent_entity_index.to_le_bytes()
    );
    assert_ne!(session.shared_spells().reset_hash(), baseline_hash);

    session.reset();
    assert!(session.shared_spells().shield_state().is_none());
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
}

#[test]
fn level_two_automatic_bee_contact_consumes_shield_status_at_player_visit() {
    let mut session = level_two_campaign_session();
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let baseline_hash = session.shared_spells().reset_hash();
    fire_level_two_model32_group(&mut session, 21);
    let acquire_shield = |session: &mut Level2ResetSession| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let source = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == 155)
            .copied()
            .expect("authored family-6 source");
        session
            .set_level2_entity_position(session.state.entity_index, source.position)
            .expect("ordinary local-player position carrier remains valid");
        (0..4)
            .find_map(|_| {
                session.step_outer_turn(PlayerCommand::default(), 0);
                session
                    .dispatch_events()
                    .iter()
                    .find_map(|event| match event {
                        Level2DispatchEvent::SpellAcquisition {
                            family: 6,
                            result: Some(outcome),
                            ..
                        } => Some(*outcome),
                        _ => None,
                    })
            })
            .expect("ordinary authored family-6 acquisition")
    };

    let outcome = acquire_shield(&mut session);
    assert!(session.shared_spells_mut().activate_shield(256, false));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let active = session
        .shared_spells()
        .shield_state()
        .expect("active family-6 Shield state");
    assert_ne!(
        active.parent_status_flags & crate::spell_activation::SHIELD_MANA_STATUS_BIT,
        0
    );
    let bee = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX))
        .copied()
        .expect("production row-229 Bee source");
    assert_eq!(bee.subspell_index, 200);
    assert!(session.state.entity_index < bee.source_entity_index);
    assert!(bee.source_entity_index < outcome.source_slot);
    let mut damage = None;
    for _ in 0..128 {
        let live_bee = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
            .copied()
            .expect("production row-229 Bee remains live");
        let mut target = live_bee.position;
        crate::move_entity(&mut target, live_bee.yaw, 0, live_bee.actual_speed);
        session
            .set_level2_entity_position(session.state.entity_index, target)
            .expect("ordinary local-player position carrier remains valid");
        session.step_outer_turn(PlayerCommand::default(), 0);
        damage = session
            .shared_spells()
            .pickup_events()
            .iter()
            .find_map(|event| match event {
                Level2SharedSpellEvent::ShieldDamageApplied {
                    target_entity_index,
                    attacker_entity_index,
                    damage,
                } if *target_entity_index == session.state.entity_index
                    && *attacker_entity_index == bee.entity_id =>
                {
                    Some(*damage)
                }
                _ => None,
            });
        if damage.is_some() {
            break;
        }
    }
    let damage = damage.expect("ordinary Bee collision reaches the player Shield consumer");
    assert_eq!(damage.raw_damage, 200);
    assert_eq!(damage.applied_damage, 50);
    assert_eq!(damage.mana_drained, 50);
    assert_eq!(
        session.player_life,
        active.parent_life - damage.applied_damage
    );
    let assert_player_mailbox =
        |session: &Level2ResetSession, expected_damage: i32, expected_owner: u16| {
            let player_record =
                session.economy.entities[usize::from(session.state.entity_index)].record_bytes();
            assert_eq!(
                i32::from_le_bytes(
                    player_record[98..102]
                        .try_into()
                        .expect("player payload bytes")
                ),
                expected_damage
            );
            assert_eq!(
                u16::from_le_bytes(
                    player_record[102..104]
                        .try_into()
                        .expect("player owner bytes")
                ),
                expected_owner
            );
            let shared_player_record = session.shared_spells().player_entity.record_bytes();
            assert_eq!(
                i32::from_le_bytes(
                    shared_player_record[98..102]
                        .try_into()
                        .expect("shared player payload bytes")
                ),
                expected_damage
            );
            assert_eq!(
                u16::from_le_bytes(
                    shared_player_record[102..104]
                        .try_into()
                        .expect("shared player owner bytes")
                ),
                expected_owner
            );
            let projected_player = session
                .shared_spells()
                .player_entities
                .iter()
                .find(|entity| entity.index == session.state.entity_index)
                .expect("shared player entity projection");
            assert_eq!(
                i32::from_le_bytes(
                    projected_player.record_bytes()[98..102]
                        .try_into()
                        .expect("projected player payload bytes")
                ),
                expected_damage
            );
            assert_eq!(
                u16::from_le_bytes(
                    projected_player.record_bytes()[102..104]
                        .try_into()
                        .expect("projected player owner bytes")
                ),
                expected_owner
            );
            assert_eq!(session.player_pending_damage, expected_damage);
            assert_eq!(session.player_pending_attacker, expected_owner);
        };
    assert_player_mailbox(&session, damage.applied_damage, 0);
    session.queue_level2_player_contact_damage(7, bee.entity_id);
    assert_player_mailbox(&session, damage.applied_damage + 7, bee.entity_id);
    assert_ne!(session.shared_spells().reset_hash(), baseline_hash);

    session.reset();
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel after replay");
    fire_level_two_model32_group(&mut session, 21);
    let outcome = acquire_shield(&mut session);
    let live_bee = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
        .copied()
        .expect("production row-229 Bee remains live before expiry");
    let shield_ticks = session
        .shared_spells()
        .shield_state()
        .expect("active family-6 Shield state before expiry alignment")
        .activation_value;
    let cadence = u16::try_from(live_bee.action_window).expect("Bee cadence fits");
    let turns_to_last_live = u16::try_from(shield_ticks - 1).expect("Shield duration fits");
    let warmup =
        (cadence - (u16::from(live_bee.source_byte_3e) + turns_to_last_live) % cadence) % cadence;
    let set_player_outside_contact = |session: &mut Level2ResetSession| {
        let live_bee = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|candidate| {
                candidate.authored_index() == Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX)
            })
            .copied()
            .expect("production row-229 Bee remains live");
        let mut position = live_bee.position;
        crate::move_entity(&mut position, live_bee.yaw, 0, 1_500);
        session
            .set_level2_entity_position(session.state.entity_index, position)
            .expect("ordinary local-player position carrier remains valid");
    };
    for _ in 0..warmup {
        set_player_outside_contact(&mut session);
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(session.shared_spells_mut().activate_shield(256, false));
    while session
        .shared_spells()
        .shield_state()
        .is_some_and(|shield| shield.active_value > 2)
    {
        set_player_outside_contact(&mut session);
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let before_acquisition = session
        .shared_spells()
        .shield_state()
        .expect("Shield remains live before expiry contact");
    assert_eq!(before_acquisition.active_value, 2);
    assert_eq!(outcome.source_slot, before_acquisition.holder_entity_index);
    let bee_before_acquisition = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
        .copied()
        .expect("production row-229 Bee remains before expiry contact");
    assert_eq!(
        (u16::from(bee_before_acquisition.source_byte_3e) + 1) % cadence,
        0
    );
    set_player_outside_contact(&mut session);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let last_live = session
        .shared_spells()
        .shield_state()
        .expect("Shield remains live on the expiry contact turn");
    assert_eq!(last_live.active_value, 1);
    let bee_before_contact = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|candidate| candidate.source_entity_index == bee.source_entity_index)
        .copied()
        .expect("production row-229 Bee remains before expiry contact");
    assert_eq!(bee_before_contact.action, 18);
    assert_eq!(u16::from(bee_before_contact.source_byte_3e) % cadence, 0);
    let life_before = session.player_life;
    session
        .set_level2_entity_position(session.state.entity_index, bee_before_contact.position)
        .expect("ordinary local-player position carrier remains valid");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let expired = session
        .shared_spells()
        .shield_state()
        .expect("Shield state remains projected through cleanup");
    assert_eq!(expired.active_value, 0);
    assert_eq!(
        expired.parent_status_flags
            & (crate::spell_activation::SHIELD_MANA_STATUS_BIT
                | crate::spell_activation::SHIELD_INVULNERABLE_STATUS_BIT),
        0
    );
    assert_eq!(session.player_pending_damage, 200);
    assert!(
        session
            .shared_spells()
            .pickup_events()
            .iter()
            .all(|event| !matches!(event, Level2SharedSpellEvent::ShieldDamageApplied { .. }))
    );

    set_player_outside_contact(&mut session);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.player_life, life_before - 200);
    assert!(
        session
            .shared_spells()
            .pickup_events()
            .iter()
            .all(|event| !matches!(event, Level2SharedSpellEvent::ShieldDamageApplied { .. }))
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            event: Level2EconomyEvent::DamageApplied {
                attacker_entity_index,
                damage: 200,
                ..
            },
            ..
        } if *attacker_entity_index == bee.entity_id
    )));
    session.reset();
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
}

#[test]
fn level_two_remote_shield_owner_does_not_overwrite_local_mana_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 remote Shield session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 155)
        .copied()
        .expect("authored family-6 source");
    let source_position = source.position;
    let local_mana = session.shared_spells().player_mana();
    let remote_mana = local_mana.wrapping_add(7_777);
    {
        let mut shared = session.shared_spells_mut();
        let remote = &mut shared.player_entities[1];
        remote.active = true;
        remote.model = 0;
        remote.mana = remote_mana;
        remote.record[2] = 1;
        remote.record[68] = 0;
        remote.position = source_position;
        remote.record[80..82].copy_from_slice(&source_position.x.to_le_bytes());
        remote.record[82..84].copy_from_slice(&source_position.y.to_le_bytes());
        remote.record[84..86].copy_from_slice(&source_position.z.to_le_bytes());
    }
    let far_position = Position {
        x: source_position.x.wrapping_add(0x2000),
        y: source_position.y,
        z: source_position.z,
    };
    session
        .shared_spells_mut()
        .dynamic_pickups
        .iter_mut()
        .find(|entity| entity.index == source.source_entity_index)
        .expect("authored family-6 source row")
        .phase = 0;
    session
        .shared_spells_mut()
        .dynamic_pickups
        .iter_mut()
        .find(|entity| entity.index == source.source_entity_index)
        .expect("authored family-6 source row")
        .record[66] = 0;
    let mut acquired = None;
    for _ in 0..4 {
        acquired = session
            .shared_spells_mut()
            .step_level2_spell_acquisition(6, 111, far_position)
            .expect("remote family-6 source pass");
        if acquired.is_some() {
            break;
        }
    }
    let outcome = acquired.expect("remote model-0 player acquires Shield");
    assert_eq!(outcome.parent_entity_index, 112);
    let shield = session
        .shared_spells()
        .shield_state()
        .expect("remote acquisition seeds Shield state");
    assert_eq!(shield.parent_entity_index, 112);
    assert_eq!(shield.parent_mana, remote_mana);
    assert_eq!(session.shared_spells().player_mana(), local_mana);
}

#[test]
fn level_two_shield_expiry_replays_queued_setspell_word() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Shield cleanup session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 155)
        .copied()
        .expect("authored family-6 source");
    session.state.position = source.position;
    let acquired = (0..4).find_map(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 6,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = acquired.expect("authored family-6 acquisition");
    assert!(session.shared_spells_mut().activate_shield(256, false));
    {
        let mut shared = session.shared_spells_mut();
        shared
            .shield
            .as_mut()
            .expect("active Shield state")
            .active_value = 1;
        shared
            .pickup_mut(outcome.source_slot)
            .expect("active Shield holder row")
            .record[48..50]
            .copy_from_slice(&2_u16.to_le_bytes());
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let holder = session
        .shared_spells()
        .dynamic_pickups()
        .iter()
        .find(|entity| entity.index == outcome.source_slot)
        .expect("active Shield holder remains projected");
    assert_eq!(holder.record_bytes()[48..50], [0, 0]);
    assert_eq!(holder.activation_value, 201);
    assert_eq!(holder.record_bytes()[52..54], 201_u16.to_le_bytes());
    assert_eq!(
        session
            .shared_spells()
            .shield_state()
            .expect("Shield state retained after expiry")
            .active_value,
        0
    );
}

#[test]
fn level_two_ordinary_invisible_acquisition_reconciles_command_latch_and_reset() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Invisible command session");
    let baseline_hash = session.campaign_state_hash();
    let selected_invisible = PlayerCommand {
        opcode: 6,
        argument_1: 11,
        flags: 0x40,
        ..PlayerCommand::default()
    };

    // No acquired local holder remains the source sentinel: the ordinary
    // selected-spell command must not manufacture family-11 state.
    session.step_outer_turn(selected_invisible, 0);
    assert!(session.shared_spells().invisible_state().is_none());
    let no_holder_fire = session
        .context
        .spell_fire
        .as_ref()
        .expect("Level 2 command context");
    assert_eq!(no_holder_fire.enabled_holder_indices[11], 0);
    assert!(!no_holder_fire.accepted_models.contains(&11));
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);

    // Group-8's ordinary objective route materializes authored row 156;
    // collision with the local flight body executes SetSpell_6D5E0 and
    // installs the dynamic family-11 holder.
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 156)
        .copied()
        .expect("authored family-11 source");
    session.state.position = source.position;
    let outcome = (0..4).find_map(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 11,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = outcome.expect("ordinary local family-11 acquisition");
    assert_eq!(outcome.source_slot, source.source_entity_index);
    assert_eq!(outcome.parent_entity_index, session.state.entity_index);
    let acquired_fire = session
        .context
        .spell_fire
        .as_ref()
        .expect("Level 2 command context remains available");
    assert_eq!(
        acquired_fire.enabled_holder_indices[11], 0,
        "the later-slot acquisition cannot rewrite the already-visited player callback's command projection"
    );
    assert!(
        acquired_fire
            .holders
            .iter()
            .all(|holder| holder.entity_index != outcome.source_slot),
        "the next player visit refreshes the newly acquired holder before selected fire"
    );

    // The actual campaign supplies this mana through Castle/economy. The
    // narrow command-bridge regression seeds only the existing owner
    // carriers so it can isolate SetSpell -> sub_5F660 -> family-11
    // state reconciliation without a second economy journey.
    let local_entity_index = session.state.entity_index;
    {
        let mut shared = session.shared_spells_mut();
        shared.player_entity.set_mana(5_000);
        shared.player_entity.mana_regen = 0;
        shared.player_entity.record[140..144].fill(0);
        shared.player_entities[0] = shared.player_entity.clone();
        shared.caster.mana = 5_000;
        shared.player_mana = 5_000;
    }
    session
        .economy
        .entities
        .get_mut(usize::from(local_entity_index))
        .expect("local source entity")
        .set_mana(5_000);
    let castle_slot = session
        .economy_mut()
        .build_castle_at(
            Position::default(),
            Position::default(),
            local_entity_index,
            false,
            true,
        )
        .expect("existing Level 2 Castle authority");
    session
        .economy_mut()
        .castle
        .as_mut()
        .expect("allocated Castle")
        .mana = 5_000;

    session.step_outer_turn(selected_invisible, 0);
    let armed_fire = session
        .context
        .spell_fire
        .as_ref()
        .expect("selected-spell context remains live");
    assert!(armed_fire.accepted_models.contains(&11));
    let armed_holder = armed_fire
        .holders
        .iter()
        .find(|holder| holder.entity_index == outcome.source_slot)
        .expect("selected command retains the family-11 registry holder");
    assert_eq!(
        armed_holder.active_value,
        i16::try_from(armed_holder.activation_value).expect("source timer fits i16"),
        "the player command projection records the slot-111 activation before the later typed holder pulse"
    );
    let armed = session
        .shared_spells()
        .invisible_state()
        .expect("ordinary command retains acquired Invisible authority");
    assert_eq!(armed.active_value, armed.activation_value - 1);
    assert_eq!(
        armed.parent_mana,
        session.shared_spells().player_entity.mana,
        "the typed holder observes the authoritative post-upkeep player mana"
    );
    assert!(armed.parent_mana >= 5_000);
    assert_eq!(
        session
            .shared_spells()
            .acquisitions
            .iter()
            .find(|state| state.family == 11)
            .expect("family-11 acquisition state")
            .source_slot,
        outcome.source_slot
    );
    assert_eq!(
        session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|holder| holder.index == outcome.source_slot)
            .expect("active family-11 holder")
            .action,
        INVISIBLE_ACTIVE_ACTION
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::SpellAcquisition {
            family: 11,
            source_slot,
            result: None,
            ..
        } if *source_slot == outcome.source_slot
    )));
    let second_pulse = session
        .shared_spells()
        .invisible_state()
        .expect("family-11 remains authoritative on its following source pass");
    assert_eq!(second_pulse.active_value, armed.active_value - 1);
    assert_eq!(second_pulse.parent_visibility_mode, 1);
    assert_ne!(second_pulse.spell_status_flags & INVISIBLE_STATUS_BIT, 0);
    assert!(
        session
            .economy()
            .castle()
            .is_some_and(|castle| castle.source_entity_index == castle_slot)
    );
    assert_ne!(session.campaign_state_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_invisible_active_holder_ticks_through_outer_turn_and_reset() {
    let mut session = level_two_campaign_session();
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    let baseline_hash = session.shared_spells().reset_hash();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 156)
        .copied()
        .expect("authored family-11 source");
    session
        .set_level2_entity_position(session.state.entity_index, source.position)
        .expect("fixture player position updates the authoritative row and map link");
    let acquired = (0..4).find_map(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 11,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = acquired.expect("authored family-11 acquisition");
    assert_eq!(outcome.source_slot, source.source_entity_index);
    let cold = session
        .shared_spells()
        .invisible_state()
        .expect("family-11 acquisition seeds Invisible state");
    assert_eq!(cold.holder_entity_index, outcome.source_slot);
    assert_eq!(cold.action, INVISIBLE_ACTIVE_ACTION);
    assert_eq!(cold.active_value, 0);

    // Give the source predicate a live castle with sufficient mana. This
    // exercises the production economy-fed branch rather than the
    // fail-closed `None` castle case.
    let parent_entity_index = cold.parent_entity_index;
    let castle_slot = session
        .economy_mut()
        .build_castle_at(
            Position::default(),
            Position::default(),
            parent_entity_index,
            false,
            true,
        )
        .expect("Level 2 test castle allocation");
    session
        .economy_mut()
        .castle
        .as_mut()
        .expect("allocated Level 2 test castle")
        .mana = 5_000;

    {
        let mut shared = session.shared_spells_mut();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared.player_entities[0].mana = 5_000;
        shared.player_entities[0].record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        // Distinguish the source first-pulse write from the qualified
        // frame-zero value, which is already zero in the owner package.
        shared.players[0].record[345..347].copy_from_slice(&0x1234_u16.to_le_bytes());
        shared
            .invisible
            .as_mut()
            .expect("cold Invisible state")
            .parent_mana = 5_000;
    }
    // The player callback refreshes the typed holder's parent from this
    // numeric source row. Seed the same declared unit-fixture mana there.
    session.economy.entities[usize::from(parent_entity_index)].set_mana(5_000);
    session.shared_spells_mut().local_player.record[345..347]
        .copy_from_slice(&0x1234_u16.to_le_bytes());
    session.players[0].record[345..347].copy_from_slice(&0x1234_u16.to_le_bytes());
    assert!(session.shared_spells_mut().activate_invisible(256, false));
    let active = session
        .shared_spells()
        .invisible_state()
        .expect("activated family-11 Invisible state");
    assert_eq!(active.active_value, active.activation_value);
    assert_ne!(session.shared_spells().reset_hash(), baseline_hash);

    session.step_outer_turn(PlayerCommand::default(), 0);
    let stepped = session
        .shared_spells()
        .invisible_state()
        .expect("Invisible state after outer source turn");
    assert_eq!(stepped.active_value, active.active_value - 1);
    assert_eq!(stepped.parent_visibility_mode, 1);
    assert_eq!(stepped.experience_awarded, 1);
    assert_ne!(stepped.spell_status_flags & INVISIBLE_STATUS_BIT, 0);
    assert_eq!(
        session.shared_spells().players[0].record_bytes()[345..347],
        [0, 0]
    );
    assert_eq!(stepped.countdown, active.countdown - 1);
    assert!(
        session
            .economy()
            .castle()
            .is_some_and(|castle| castle.source_entity_index == castle_slot)
    );

    // The Invisible source write is one-shot. A later writer to word
    // 0x159 is not erased by subsequent Invisible projection sync, but
    // the ordinary player-upkeep countdown still decrements it once per
    // unpaused source pass.
    session.shared_spells_mut().players[0].record[345..347]
        .copy_from_slice(&0x5678_u16.to_le_bytes());
    session.shared_spells_mut().local_player.record[345..347]
        .copy_from_slice(&0x5678_u16.to_le_bytes());
    session.players[0].record[345..347].copy_from_slice(&0x5678_u16.to_le_bytes());
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.shared_spells().players[0].record_bytes()[345..347],
        0x5677_u16.to_le_bytes()
    );

    session.reset();
    assert!(session.shared_spells().invisible_state().is_none());
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
}

#[test]
fn level_two_remote_invisible_owner_does_not_overwrite_local_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 remote Invisible session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 156)
        .copied()
        .expect("authored family-11 source");
    let source_position = source.position;
    let local_mana = session.shared_spells().player_mana();
    let local_visibility = session.shared_spells().local_player.record[447];
    let remote_mana = local_mana.wrapping_add(7_777);
    {
        let mut shared = session.shared_spells_mut();
        let remote = &mut shared.player_entities[1];
        remote.active = true;
        remote.model = 0;
        remote.mana = remote_mana;
        remote.record[2] = 1;
        remote.record[68] = 0;
        remote.position = source_position;
        remote.record[80..82].copy_from_slice(&source_position.x.to_le_bytes());
        remote.record[82..84].copy_from_slice(&source_position.y.to_le_bytes());
        remote.record[84..86].copy_from_slice(&source_position.z.to_le_bytes());
        let source = shared
            .dynamic_pickups
            .iter_mut()
            .find(|entity| entity.index == source.source_entity_index)
            .expect("authored family-11 source row");
        source.phase = 0;
        source.record[66] = 0;
    }
    let far_position = Position {
        x: source_position.x.wrapping_add(0x2000),
        y: source_position.y,
        z: source_position.z,
    };
    let outcome = (0..4).find_map(|_| {
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(11, 111, far_position)
            .expect("remote family-11 source pass")
    });
    let outcome = outcome.expect("remote model-0 player acquires Invisible");
    assert_eq!(outcome.parent_entity_index, 112);
    let invisible = session
        .shared_spells()
        .invisible_state()
        .expect("remote acquisition seeds Invisible state");
    assert_eq!(invisible.parent_entity_index, 112);
    assert_eq!(invisible.parent_mana, remote_mana);
    assert_eq!(session.shared_spells().player_mana(), local_mana);
    assert_eq!(
        session.shared_spells().local_player.record[447],
        local_visibility
    );

    // A normal local selected-spell request must not borrow the remote
    // owner's dynamically acquired family-11 holder.  Give the local
    // command caster enough mana so this would be an accepted cast if the
    // local registry leaked that remote identity into `sub_5F660`.
    {
        let mut shared = session.shared_spells_mut();
        shared.player_entity.set_mana(5_000);
        shared.player_entity.mana_regen = 0;
        shared.player_entity.record[140..144].fill(0);
        shared.player_entities[0] = shared.player_entity.clone();
        shared.caster.mana = 5_000;
        shared.player_mana = 5_000;
    }
    session.step_outer_turn(
        PlayerCommand {
            opcode: 6,
            argument_1: 11,
            flags: 0x40,
            ..PlayerCommand::default()
        },
        0,
    );
    let local_fire = session
        .context
        .spell_fire
        .as_ref()
        .expect("Level 2 local command context");
    assert_eq!(local_fire.enabled_holder_indices[11], 0);
    assert!(!local_fire.accepted_models.contains(&11));
    let remote_invisible = session
        .shared_spells()
        .invisible_state()
        .expect("remote Invisible remains authoritative");
    assert_eq!(remote_invisible.parent_entity_index, 112);
    assert_eq!(remote_invisible.active_value, 0);
}

#[test]
fn level_two_nyphur_visibility_consumes_invisible_owner_and_beyond_sight_tier() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 visibility session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);

    let invisible_source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 156)
        .copied()
        .expect("authored family-11 source");
    let invisible_position = invisible_source.position;
    let remote_mana = session.shared_spells().player_mana().wrapping_add(7_777);
    {
        let mut shared = session.shared_spells_mut();
        let remote = &mut shared.player_entities[1];
        remote.active = true;
        remote.model = 0;
        remote.mana = remote_mana;
        remote.record[2] = 1;
        remote.record[68] = 0;
        remote.position = invisible_position;
        remote.record[80..82].copy_from_slice(&invisible_position.x.to_le_bytes());
        remote.record[82..84].copy_from_slice(&invisible_position.y.to_le_bytes());
        remote.record[84..86].copy_from_slice(&invisible_position.z.to_le_bytes());
        let source = shared
            .dynamic_pickups
            .iter_mut()
            .find(|entity| entity.index == invisible_source.source_entity_index)
            .expect("authored family-11 source row");
        source.phase = 0;
        source.record[66] = 0;
    }
    let far_position = Position {
        x: invisible_position.x.wrapping_add(0x2000),
        y: invisible_position.y,
        z: invisible_position.z,
    };
    let acquired = (0..4).find_map(|_| {
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(11, 111, far_position)
            .expect("remote family-11 source pass")
    });
    let acquired = acquired.expect("remote model-0 player acquires Invisible");
    {
        let mut shared = session.shared_spells_mut();
        shared
            .invisible
            .as_mut()
            .expect("remote acquisition seeds Invisible state")
            .parent_mana = remote_mana;
        assert!(shared.activate_invisible(256, false));
    }
    {
        session
            .shared_spells
            .step_invisible_source_slot(acquired.source_slot, Some(2_000));
    }
    let hidden = session
        .nyphur_visibility()
        .expect("remote Invisible visibility projection");
    assert_eq!(hidden.owner_visibility_mode, 1);
    assert!(!hidden.beyond_sight_holder_present);
    assert!(!hidden.visible_to_local_player);

    let beyond_position = session.shared_spells().pickups()[0].position;
    {
        let mut shared = session.shared_spells_mut();
        shared.players[0].family_selected_tiers[12] = 1;
        shared.pickups[0].phase = 0;
        shared.pickups[0].record[66] = 0;
    }
    {
        let terrain = session.canonical_terrain().clone();
        let live = &mut session.session.state.live;
        let shared = &mut live.shared_spells;
        let economy = &mut live.economy.economy;
        shared
            .step_level2_spell_acquisition_with_economy(economy, 12, 111, beyond_position, &terrain)
            .expect("local family-12 source pass")
            .expect("local model-0 player acquires Beyond Sight");
    }
    {
        let mut shared = session.shared_spells_mut();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared.player_entities[0].mana = 5_000;
        shared.player_entities[0].record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared
            .beyond_sight
            .as_mut()
            .expect("local family-12 acquisition seeds Beyond Sight state")
            .parent_mana = 5_000;
        assert!(shared.activate_beyond_sight(256, false));
    }
    {
        let mut shared = session.shared_spells_mut();
        shared.players[1].enabled_spell_holders[4] = 113;
        shared.holders[0].active_value = 1;
    }
    let blocked = session
        .nyphur_visibility()
        .expect("family-4 visibility gate projection");
    assert!(blocked.family4_gate_active);
    assert!(!blocked.visible_to_local_player);
    {
        let mut shared = session.shared_spells_mut();
        shared.players[1].enabled_spell_holders[4] = 0;
        shared.holders[0].active_value = 0;
    }
    let revealed = session
        .nyphur_visibility()
        .expect("Beyond Sight visibility projection");
    assert_eq!(revealed.owner_visibility_mode, 1);
    assert!(revealed.beyond_sight_holder_present);
    assert!(revealed.beyond_sight_active);
    assert_eq!(revealed.beyond_sight_tier, Some(1));
    assert!(revealed.visible_to_local_player);
    assert_eq!(acquired.parent_entity_index, 112);
}

#[test]
fn level_two_invisible_expiry_replays_queued_setspell_word() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Invisible cleanup session");
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let source = session
        .economy()
        .spell_pickups()
        .iter()
        .find(|source| source.authored_index == 156)
        .copied()
        .expect("authored family-11 source");
    session.state.position = source.position;
    let acquired = (0..4).find_map(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 11,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = acquired.expect("authored family-11 acquisition");
    session
        .shared_spells_mut()
        .invisible
        .as_mut()
        .expect("cold Invisible state")
        .parent_mana = 5_000;
    assert!(session.shared_spells_mut().activate_invisible(256, false));
    {
        let mut shared = session.shared_spells_mut();
        shared
            .invisible
            .as_mut()
            .expect("active Invisible state")
            .active_value = 1;
        shared
            .pickup_mut(outcome.source_slot)
            .expect("active Invisible holder row")
            .record[48..50]
            .copy_from_slice(&2_u16.to_le_bytes());
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let holder = session
        .shared_spells()
        .dynamic_pickups()
        .iter()
        .find(|entity| entity.index == outcome.source_slot)
        .expect("active Invisible holder remains projected");
    assert_eq!(holder.record_bytes()[48..50], [0, 0]);
    assert_eq!(holder.activation_value, 183);
    assert_eq!(holder.record_bytes()[52..54], 183_u16.to_le_bytes());
    let invisible = session
        .shared_spells()
        .invisible_state()
        .expect("Invisible state retained after expiry");
    assert_eq!(invisible.active_value, 0);
    assert_eq!(invisible.tier, 1);
    assert_eq!(invisible.parent_visibility_mode, 0);
}

#[test]
fn level_two_beyond_sight_active_holder_ticks_through_outer_turn_and_reset() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.shared_spells().reset_hash();
    let outcome = acquire_level_two_beyond_sight_from_ordinary_route(&mut session);
    let cold = session
        .shared_spells()
        .beyond_sight_state()
        .expect("family-12 acquisition seeds Beyond Sight state");
    assert_eq!(cold.holder_entity_index, outcome.source_slot);
    assert_eq!(cold.action, BEYOND_SIGHT_ACTIVE_ACTION);
    assert_eq!(cold.active_value, 0);

    {
        let mut shared = session.shared_spells_mut();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared.player_entities[0].mana = 5_000;
        shared.player_entities[0].record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared
            .beyond_sight
            .as_mut()
            .expect("cold Beyond Sight state")
            .parent_mana = 5_000;
    }
    assert!(
        session
            .shared_spells_mut()
            .activate_beyond_sight(256, false)
    );
    let active = session
        .shared_spells()
        .beyond_sight_state()
        .expect("activated family-12 Beyond Sight state");
    assert_eq!(active.active_value, active.activation_value);
    assert_ne!(session.shared_spells().reset_hash(), baseline_hash);

    session.step_outer_turn(
        PlayerCommand {
            argument_6: 0x1234,
            argument_8: 0xabcd,
            ..PlayerCommand::default()
        },
        0,
    );
    let stepped = session
        .shared_spells()
        .beyond_sight_state()
        .expect("Beyond Sight state after outer source turn");
    assert_eq!(stepped.active_value, active.active_value - 1);
    assert_eq!(stepped.experience_awarded, 1);
    assert_eq!(
        stepped.parent_mana_regen,
        session.shared_spells().player_entity.mana_regen
    );
    assert!(
        stepped.parent_mana_regen > 0,
        "slot 38 charges the fresh pulse before slot 111 consumes it and selects the next ordinary recharge rate"
    );
    assert_eq!(stepped.countdown, active.countdown - 1);
    assert!(
        session.economy().beyond_sight_effects().is_empty(),
        "ordinary class-15/action-36 never constructs the separate action-72/model-74 row"
    );
    assert_eq!(
        session
            .shared_spells()
            .beyond_sight_auxiliary()
            .effect_parameter,
        0
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::BeyondSightPulseEffect,
            ..
        }
    )));
    assert!(
        !session
            .shared_spells()
            .pickup_events()
            .iter()
            .any(|event| matches!(event, Level2SharedSpellEvent::AbilityImproved { .. }))
    );
    assert!(
        !session
            .shared_spells()
            .activation_events()
            .iter()
            .any(|event| matches!(event, FlightEvent::PrepareSound { sound_id: 61, .. }))
    );

    session.reset();
    assert!(session.shared_spells().beyond_sight_state().is_none());
    assert!(session.economy().beyond_sight_effects().is_empty());
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
}

#[test]
fn level_two_beyond_sight_threshold_crossing_emits_improvement_after_award() {
    let mut session = level_two_campaign_session();
    let outcome = acquire_level_two_beyond_sight_from_ordinary_route(&mut session);
    let local_entity_index = session.state.entity_index;
    session
        .economy_mut()
        .build_castle_at(
            Position::default(),
            Position::default(),
            local_entity_index,
            false,
            true,
        )
        .expect("source-order threshold Castle allocation");
    session
        .economy_mut()
        .castle
        .as_mut()
        .expect("source-order threshold Castle")
        .mana = 9_999;

    {
        let mut shared = session.shared_spells_mut();
        let owner_index = shared
            .players
            .iter()
            .position(|player| player.entity_index == local_entity_index)
            .expect("local player row");
        let owner = &mut shared.players[owner_index];
        owner.runtime_spell_experience[12] = 359;
        owner.record[212..216].copy_from_slice(&359_i32.to_le_bytes());
        owner.spell_unlocked_levels[12] = 0;
        owner.record[514] = 0;
        owner.family_selected_tiers[12] = 0;
        owner.record[540] = 0;
        owner.enabled_spell_holders[12] = 0;
        owner.record[268 + 12 * 2..270 + 12 * 2].fill(0);
        owner.spell_hand_eligibility[12] = 0;
        owner.record[398 + 12] = 0;
        owner.spell_enabled_flags[12] = 1;
        owner.record[450 + 12] = 1;
        shared.local_player = owner.clone();
        shared.spell_experience[12] = 359;
        shared.spell_levels[12] = 0;
        shared.pickup_events.clear();
        shared.events.clear();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared.player_entities[0].mana = 5_000;
        shared.player_entities[0].record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared
            .beyond_sight
            .as_mut()
            .expect("cold Beyond Sight state")
            .parent_mana = 5_000;
        let beyond_sight = shared
            .beyond_sight
            .as_mut()
            .expect("fresh tier-1 Beyond Sight state");
        beyond_sight.tier = 1;
        beyond_sight.activation_value = 261;
        beyond_sight.mana_cost = 2_400;
        beyond_sight.mana_regen_requirement = 10_000;
    }
    assert!(
        session
            .shared_spells_mut()
            .activate_beyond_sight(256, false)
    );
    session.shared_spells_mut().events.clear();
    session.step_outer_turn(PlayerCommand::default(), 0);

    let mut shared = session.shared_spells();
    let owner = shared
        .players
        .iter()
        .find(|player| player.entity_index == local_entity_index)
        .expect("local player row after threshold crossing");
    assert_eq!(owner.runtime_spell_experience[12], 360);
    assert_eq!(owner.record_bytes()[212..216], 360_i32.to_le_bytes());
    assert_eq!(owner.spell_unlocked_levels[12], 1);
    assert_eq!(owner.record_bytes()[514], 1);
    assert_eq!(owner.family_selected_tiers[12], 0);
    assert_eq!(owner.record_bytes()[540], 0);
    assert_eq!(shared.spell_experience()[12], 360);
    assert_eq!(shared.spell_levels()[12], 1);
    let stepped = shared
        .beyond_sight_state()
        .expect("tier-1 Beyond Sight state after source-order pulse");
    assert_eq!(stepped.tier, 1);
    assert_eq!(stepped.active_value, 0);
    assert_eq!(stepped.experience_awarded, 1);
    assert_eq!(
        shared.pickup_events(),
        &[Level2SharedSpellEvent::AbilityImproved {
            source_family: 12,
            tier: 1,
        }]
    );
    assert_eq!(
        shared
            .activation_events()
            .iter()
            .filter(|event| matches!(event, FlightEvent::PrepareSound { sound_id: 61, .. }))
            .count(),
        1
    );

    let runtime_after_first_pulse = owner.runtime_spell_experience[12];
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session.shared_spells().spell_experience()[12],
        runtime_after_first_pulse,
        "a terminal/later pulse cannot award a second first-pulse XP"
    );
    assert_eq!(
        session
            .shared_spells()
            .pickup_events()
            .iter()
            .filter(|event| matches!(event, Level2SharedSpellEvent::AbilityImproved { .. }))
            .count(),
        0
    );
    assert_eq!(outcome.parent_entity_index, local_entity_index);
}

#[test]
fn level_two_beyond_sight_threshold_without_source_enabled_array_or_holder_is_silent() {
    let mut session = level_two_campaign_session();
    let outcome = acquire_level_two_beyond_sight_from_ordinary_route(&mut session);
    let local_entity_index = session.state.entity_index;

    {
        let mut shared = session.shared_spells_mut();
        let owner_index = shared
            .players
            .iter()
            .position(|player| player.entity_index == local_entity_index)
            .expect("local player row");
        let owner = &mut shared.players[owner_index];
        owner.runtime_spell_experience[12] = 359;
        owner.record[212..216].copy_from_slice(&359_i32.to_le_bytes());
        owner.spell_unlocked_levels[12] = 0;
        owner.record[514] = 0;
        owner.family_selected_tiers[12] = 2;
        owner.record[540] = 2;
        owner.left_spell_slot_raw = -1;
        owner.right_spell_slot_raw = -1;
        owner.record[554..556].copy_from_slice(&(-1_i16).to_le_bytes());
        owner.record[556..558].copy_from_slice(&(-1_i16).to_le_bytes());
        owner.spell_hand_eligibility[12] = 1;
        owner.record[398 + 12] = 1;
        owner.spell_enabled_flags[12] = 0;
        owner.record[450 + 12] = 0;
        owner.enabled_spell_holders[12] = 0;
        owner.record[268 + 12 * 2..270 + 12 * 2].fill(0);
        shared.local_player = owner.clone();
        shared.spell_experience[12] = 359;
        shared.spell_levels[12] = 0;
        shared.pickup_events.clear();
        shared.events.clear();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared.player_entities[0].mana = 5_000;
        shared.player_entities[0].record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        let beyond_sight = shared
            .beyond_sight
            .as_mut()
            .expect("cold Beyond Sight state");
        beyond_sight.tier = 1;
        beyond_sight.activation_value = 261;
        beyond_sight.mana_cost = 2_400;
        beyond_sight.mana_regen_requirement = 10_000;
        beyond_sight.parent_mana = 5_000;
    }
    assert!(
        session
            .shared_spells_mut()
            .activate_beyond_sight(256, false)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);

    let mut shared = session.shared_spells();
    let owner = shared
        .players
        .iter()
        .find(|player| player.entity_index == local_entity_index)
        .expect("local player row after silent threshold crossing");
    assert_eq!(owner.runtime_spell_experience[12], 360);
    assert_eq!(owner.record_bytes()[212..216], 360_i32.to_le_bytes());
    assert_eq!(owner.spell_unlocked_levels[12], 1);
    assert_eq!(owner.record_bytes()[514], 1);
    assert_eq!(owner.family_selected_tiers[12], 1);
    assert_eq!(owner.record_bytes()[540], 1);
    assert_eq!(shared.spell_experience()[12], 360);
    assert_eq!(shared.spell_levels()[12], 1);
    assert!(shared.pickup_events().is_empty());
    assert!(
        !shared
            .activation_events()
            .iter()
            .any(|event| matches!(event, FlightEvent::PrepareSound { sound_id: 61, .. }))
    );
    assert_eq!(outcome.parent_entity_index, local_entity_index);
}

#[test]
fn level_two_beyond_sight_wrapping_level_drop_still_emits_source_improvement() {
    let mut session = level_two_campaign_session();
    acquire_level_two_beyond_sight_from_ordinary_route(&mut session);
    let local_entity_index = session.state.entity_index;
    {
        let mut shared = session.shared_spells_mut();
        let owner_index = shared
            .players
            .iter()
            .position(|player| player.entity_index == local_entity_index)
            .expect("local player row");
        let owner = &mut shared.players[owner_index];
        owner.persistent_spell_experience[12] = 0;
        owner.record[108..112].fill(0);
        owner.runtime_spell_experience[12] = i32::MAX;
        owner.record[212..216].copy_from_slice(&i32::MAX.to_le_bytes());
        owner.spell_unlocked_levels[12] = 2;
        owner.record[514] = 2;
        owner.family_selected_tiers[12] = 2;
        owner.record[540] = 2;
        shared.local_player = owner.clone();
        shared.spell_experience[12] = i32::MAX;
        shared.spell_levels[12] = 2;
        shared.pickup_events.clear();
        shared.events.clear();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared
            .beyond_sight
            .as_mut()
            .expect("cold Beyond Sight state")
            .parent_mana = 5_000;
    }
    assert!(
        session
            .shared_spells_mut()
            .activate_beyond_sight(256, false)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);

    let mut shared = session.shared_spells();
    let owner = shared
        .players
        .iter()
        .find(|player| player.entity_index == local_entity_index)
        .expect("local player row after wrapping award");
    assert_eq!(owner.runtime_spell_experience[12], i32::MIN);
    assert_eq!(owner.record_bytes()[212..216], i32::MIN.to_le_bytes());
    assert_eq!(owner.spell_unlocked_levels[12], 0);
    assert_eq!(owner.record_bytes()[514], 0);
    assert_eq!(owner.family_selected_tiers[12], 0);
    assert_eq!(owner.record_bytes()[540], 0);
    assert_eq!(shared.spell_experience()[12], i32::MIN);
    assert_eq!(shared.spell_levels()[12], 0);
    assert_eq!(
        shared.pickup_events(),
        &[Level2SharedSpellEvent::AbilityImproved {
            source_family: 12,
            tier: 0,
        }]
    );
    assert_eq!(
        shared.activation_events(),
        &[FlightEvent::PrepareSound {
            audio_site: crate::MissionAudioSite::AbilityImproved,
            entity_index: 0,
            player_index: -1,
            sound_id: 61,
        }]
    );
}

#[test]
fn level_two_remote_beyond_sight_owner_does_not_overwrite_local_mana_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 remote Beyond Sight session");
    let source_position = session.shared_spells().pickups()[0].position;
    let local_mana = session.shared_spells().player_mana();
    let remote_mana = local_mana.wrapping_add(7_777);
    {
        let mut shared = session.shared_spells_mut();
        let remote = &mut shared.player_entities[1];
        remote.active = true;
        remote.model = 0;
        remote.mana = remote_mana;
        remote.record[2] = 1;
        remote.record[68] = 0;
        remote.position = source_position;
        remote.record[80..82].copy_from_slice(&source_position.x.to_le_bytes());
        remote.record[82..84].copy_from_slice(&source_position.y.to_le_bytes());
        remote.record[84..86].copy_from_slice(&source_position.z.to_le_bytes());
        shared.pickups[0].phase = 0;
        shared.pickups[0].record[66] = 0;
    }
    let far_position = Position {
        x: source_position.x.wrapping_add(0x2000),
        y: source_position.y,
        z: source_position.z,
    };
    let outcome = (0..4).find_map(|_| {
        session
            .shared_spells_mut()
            .step_level2_spell_acquisition(12, 111, far_position)
            .expect("remote family-12 source pass")
    });
    let outcome = outcome.expect("remote model-0 player acquires Beyond Sight");
    assert_eq!(outcome.parent_entity_index, 112);
    let beyond_sight = session
        .shared_spells()
        .beyond_sight_state()
        .expect("remote acquisition seeds Beyond Sight state");
    assert_eq!(beyond_sight.parent_entity_index, 112);
    assert_eq!(beyond_sight.parent_mana, remote_mana);
    assert_eq!(session.shared_spells().player_mana(), local_mana);

    {
        let mut shared = session.shared_spells_mut();
        let remote_player = &mut shared.players[1];
        remote_player.runtime_spell_experience[12] = 359;
        remote_player.record[212..216].copy_from_slice(&359_i32.to_le_bytes());
        remote_player.spell_unlocked_levels[12] = 0;
        remote_player.record[514] = 0;
        remote_player.family_selected_tiers[12] = 0;
        remote_player.record[540] = 0;
        assert!(shared.activate_beyond_sight(0, false));
        let activation = shared
            .beyond_sight
            .expect("remote Beyond Sight state remains active")
            .activation_value;
        shared
            .beyond_sight
            .as_mut()
            .expect("remote Beyond Sight state remains mutable")
            .active_value = activation;
        shared.pickup_events.clear();
        shared.events.clear();
    }
    let local_runtime_before = session.shared_spells().spell_experience()[12];
    let local_level_before = session.shared_spells().spell_levels()[12];
    let local_record_before =
        session.shared_spells().local_player.record_bytes()[212..216].to_vec();
    let remote_runtime_before = session.shared_spells().players[1].runtime_spell_experience[12];
    let hash_before = session.shared_spells().reset_hash();
    session
        .shared_spells_mut()
        .step_level2_spell_acquisition(12, 111, far_position)
        .expect("remote action-36 source pulse");
    let mut shared = session.shared_spells();
    assert_eq!(
        shared.players[1].runtime_spell_experience[12],
        remote_runtime_before.wrapping_add(1)
    );
    assert_eq!(
        shared.players[1].record_bytes()[212..216],
        360_i32.to_le_bytes()
    );
    assert_eq!(shared.players[1].spell_unlocked_levels[12], 1);
    assert_eq!(shared.players[1].record_bytes()[514], 1);
    assert_eq!(shared.players[1].family_selected_tiers[12], 0);
    assert_eq!(shared.players[1].record_bytes()[540], 0);
    assert_eq!(shared.spell_experience()[12], local_runtime_before);
    assert_eq!(shared.spell_levels()[12], local_level_before);
    assert_eq!(
        shared.local_player.record_bytes()[212..216],
        local_record_before
    );
    assert_ne!(
        shared.reset_hash(),
        hash_before,
        "the global hash includes the remote row"
    );
    assert_eq!(
        shared.pickup_events(),
        &[Level2SharedSpellEvent::AbilityImproved {
            source_family: 12,
            tier: 1,
        }]
    );
    assert_eq!(
        shared.activation_events(),
        &[FlightEvent::PrepareSound {
            audio_site: crate::MissionAudioSite::AbilityImproved,
            entity_index: 0,
            player_index: -1,
            sound_id: 61,
        }]
    );
}

#[test]
fn level_two_beyond_sight_core_menu_preserves_raw_opcode41_without_physical_mutation() {
    let (mut session, source_slot) = level_two_acquired_family12_menu_session();

    let mut shared = session.shared_spells();
    let holder_before_locked_menu = shared
        .pickups()
        .iter()
        .chain(shared.dynamic_pickups().iter())
        .find(|holder| holder.index == source_slot)
        .expect("acquired family-12 holder");
    assert_eq!(holder_before_locked_menu.level_gate, 0);
    assert_eq!(holder_before_locked_menu.action, BEYOND_SIGHT_ACTIVE_ACTION);
    assert_eq!(holder_before_locked_menu.record_bytes()[74], 0);
    assert_eq!(holder_before_locked_menu.mana_cost, 1_800);
    let holder_before_locked_menu = (
        holder_before_locked_menu.level_gate,
        holder_before_locked_menu.action,
        holder_before_locked_menu.activation_value,
        holder_before_locked_menu.mana_cost,
        holder_before_locked_menu.record_bytes()[74],
    );

    session.step_outer_turn(
        PlayerCommand {
            opcode: 40,
            argument_1: 12,
            argument_2: 7,
            ..PlayerCommand::default()
        },
        0,
    );
    let mut shared = session.shared_spells();
    assert_eq!(shared.players[0].record_bytes()[560], 7);
    assert_eq!(shared.players[0].record_bytes()[561], 12);
    assert_eq!(shared.players[0].record_bytes()[562], 0);
    assert_eq!(shared.players[0].family_selected_tiers[12], 0);
    let holder_after_op40 = shared
        .pickups()
        .iter()
        .chain(shared.dynamic_pickups().iter())
        .find(|holder| holder.index == source_slot)
        .map(|holder| {
            (
                holder.level_gate,
                holder.action,
                holder.activation_value,
                holder.mana_cost,
                holder.record_bytes()[74],
            )
        })
        .expect("family-12 holder after opcode 40");
    assert_eq!(holder_after_op40, holder_before_locked_menu);

    session.step_outer_turn(
        PlayerCommand {
            opcode: 41,
            argument_1: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    let mut shared = session.shared_spells();
    assert_eq!(shared.players[0].record_bytes()[561], 12);
    assert_eq!(shared.players[0].record_bytes()[562], 1);
    assert_eq!(shared.players[0].selected_spell_ui_index, 12);
    assert_eq!(shared.players[0].selected_subspell_index, 1);
    assert_eq!(shared.players[0].family_selected_tiers[12], 1);
    assert_eq!(shared.players[0].record_bytes()[540], 1);
    assert_eq!(shared.spell_levels()[12], 0);
    let holder_after_op41 = shared
        .pickups()
        .iter()
        .chain(shared.dynamic_pickups().iter())
        .find(|holder| holder.index == source_slot)
        .map(|holder| {
            (
                holder.level_gate,
                holder.action,
                holder.activation_value,
                holder.mana_cost,
                holder.record_bytes()[74],
            )
        })
        .expect("family-12 holder after opcode 41");
    assert_eq!(holder_after_op41, holder_before_locked_menu);
}

#[test]
fn level_two_beyond_sight_core_opcode31_applies_profile_at_zero_unlocked_tier() {
    let (mut session, source_slot) = level_two_acquired_family12_menu_session();
    // The source menu protocol is 40 -> 41 -> 31/32. The raw opcode-41
    // write remains visible at XP tier zero; opcode 31 is the separate
    // physical SetSpell projection, not a UI-level locked-tier refusal.
    session.step_outer_turn(
        PlayerCommand {
            opcode: 40,
            argument_1: 12,
            argument_2: 7,
            ..PlayerCommand::default()
        },
        0,
    );
    session.step_outer_turn(
        PlayerCommand {
            opcode: 41,
            argument_1: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    session.step_outer_turn(
        PlayerCommand {
            opcode: 31,
            argument_1: 12,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    let mut shared = session.shared_spells();
    assert_eq!(shared.spell_levels()[12], 0);
    assert_eq!(shared.players[0].family_selected_tiers[12], 1);
    assert_eq!(shared.players[0].record_bytes()[540], 1);
    assert_eq!(shared.players[0].left_spell_slot_raw, 12);
    assert_eq!(shared.players[0].left_subspell_index, 1);
    assert_eq!(shared.players[0].record_bytes()[558], 1);
    let holder_after_op31 = shared
        .pickups()
        .iter()
        .chain(shared.dynamic_pickups().iter())
        .find(|holder| holder.index == source_slot)
        .expect("family-12 holder after opcode 31");
    assert_eq!(holder_after_op31.level_gate, 1);
    assert_eq!(holder_after_op31.record_bytes()[74], 1);
    assert_eq!(holder_after_op31.activation_value, 261);
    assert_eq!(
        shared
            .beyond_sight_state()
            .expect("cold Beyond Sight state")
            .tier,
        1
    );

    session.step_outer_turn(
        PlayerCommand {
            opcode: 32,
            argument_1: 12,
            argument_2: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    let mut shared = session.shared_spells();
    assert_eq!(shared.players[0].right_spell_slot_raw, 12);
    assert_eq!(shared.players[0].right_subspell_index, 1);
    assert_eq!(shared.players[0].record_bytes()[559], 1);
}

#[test]
fn level_two_beyond_sight_expiry_replays_queued_setspell_word() {
    let mut session = level_two_campaign_session();
    let source_slot = acquire_level_two_beyond_sight_from_ordinary_route(&mut session).source_slot;
    {
        let mut shared = session.shared_spells_mut();
        shared.player_mana = 5_000;
        shared.player_entity.mana = 5_000;
        shared.player_entity.record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared.player_entities[0].mana = 5_000;
        shared.player_entities[0].record[148..152].copy_from_slice(&5_000_i32.to_le_bytes());
        shared
            .beyond_sight
            .as_mut()
            .expect("cold Beyond Sight state")
            .parent_mana = 5_000;
        assert!(shared.activate_beyond_sight(256, false));
    }
    // While the holder is active, SetSpell queues the newly selected tier in
    // word 0x2c. The earlier synthetic test wrote that projection directly and
    // therefore bypassed the authoritative imported row.
    for command in [
        PlayerCommand {
            opcode: 40,
            argument_1: BEYOND_SIGHT_FAMILY.cast_signed(),
            argument_2: 7,
            ..PlayerCommand::default()
        },
        PlayerCommand {
            opcode: 41,
            argument_1: 1,
            ..PlayerCommand::default()
        },
        PlayerCommand {
            opcode: 31,
            argument_1: BEYOND_SIGHT_FAMILY.cast_signed(),
            argument_2: 1,
            ..PlayerCommand::default()
        },
    ] {
        session.step_outer_turn(command, 0);
    }
    assert_eq!(
        session
            .shared_spells()
            .pickup(source_slot)
            .expect("active Beyond Sight holder after SetSpell")
            .record_bytes()[48..50],
        2_u16.to_le_bytes()
    );
    session
        .shared_spells_mut()
        .beyond_sight
        .as_mut()
        .expect("active Beyond Sight state")
        .active_value = 1;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let holder = session
        .shared_spells()
        .pickups()
        .iter()
        .chain(session.shared_spells().dynamic_pickups().iter())
        .find(|entity| entity.index == source_slot)
        .expect("active Beyond Sight holder remains projected");
    assert_eq!(holder.record_bytes()[48..50], [0, 0]);
    assert_eq!(holder.activation_value, 261);
    assert_eq!(holder.record_bytes()[52..54], 261_u16.to_le_bytes());
    let beyond_sight = session
        .shared_spells()
        .beyond_sight_state()
        .expect("Beyond Sight state retained after expiry");
    assert_eq!(beyond_sight.active_value, 0);
    assert_eq!(beyond_sight.tier, 1);
    assert_eq!(beyond_sight.mana_regen_requirement, 10_000);
}

#[test]
fn level_two_persistent_replacement_reaches_production_acquisition_and_reallocates() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 persistent replacement session");
    let position = Position {
        x: 3_072,
        y: 5_120,
        z: 128,
    };
    let first = session
        .allocate_level2_spell_replacement(11, 38, position)
        .expect("persistent family-11 source allocation");
    session.state.position = position;
    let mut acquired = None;
    for _ in 0..4 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        acquired = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 11,
                    source_slot,
                    result: Some(outcome),
                    ..
                } => Some((*source_slot, *outcome)),
                _ => None,
            });
        if acquired.is_some() {
            break;
        }
    }
    let (source_slot, outcome) = acquired.expect("persistent family-11 source acquisition");
    assert_eq!(source_slot, first.source_entity_index);
    assert_eq!(outcome.holder_action, 33);
    assert_eq!(outcome.replacement_action, Some(35));
    let rows = session.economy().spell_replacements();
    let holder = rows
        .iter()
        .find(|row| row.source_entity_index == first.source_entity_index)
        .expect("acquired persistent source remains in arena");
    assert_eq!(holder.action, 33);
    let replacement = rows
        .iter()
        .find(|row| row.source_entity_index != first.source_entity_index && row.action == 35)
        .expect("sub_69250 replacement source");
    assert_eq!(replacement.family, 11);
    assert_eq!(replacement.parent_entity_index, outcome.parent_entity_index);
    assert_eq!(replacement.position, position);
    assert!(
        session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .any(|entity| {
                entity.index == replacement.source_entity_index && entity.action == 35
            })
    );
    assert_eq!(
        session
            .shared_spells()
            .persistent_acquisition_states()
            .get(&replacement.source_entity_index)
            .expect("persistent family-11 acquisition state")
            .source_slot,
        replacement.source_entity_index
    );
}

#[test]
fn level_two_persistent_replacement_boundary_is_complete() {
    let persistent_spell_sources = |session: &Level2ResetSession| {
        session
            .shared_spells()
            .pickups()
            .iter()
            .chain(session.shared_spells().dynamic_pickups().iter())
            .filter(|entity| {
                entity.class == 15
                    && matches!(entity.model, 6 | 11 | 12)
                    && matches!(entity.action, 20 | 35 | 38)
            })
            .map(|entity| (entity.index, entity.model, entity.action))
            .collect::<Vec<_>>()
    };

    let package_session = level_two_campaign_session();
    let ordinary_authored_spell_rows = package_session
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| feature.kind == 15 && matches!(feature.subtype, 6 | 11 | 12))
        .map(|feature| (feature.index, feature.subtype, feature.stage_tag))
        .collect::<Vec<_>>();
    assert_eq!(
        ordinary_authored_spell_rows,
        vec![(155, 6, 1), (156, 11, 1), (166, 12, 1)]
    );
    let stage_two_spell_rows = package_session
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| {
            feature.kind == 15 && matches!(feature.subtype, 6 | 11 | 12) && feature.stage_tag == 2
        })
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    assert!(
        stage_two_spell_rows.is_empty(),
        "qualified Level 2 package unexpectedly contains stage-2 spell rows: {stage_two_spell_rows:?}"
    );
    assert_eq!(
        physical_spell_action(6, PhysicalSpellActionRole::OrdinaryPickup),
        Some(19)
    );
    assert_eq!(
        physical_spell_action(11, PhysicalSpellActionRole::OrdinaryPickup),
        Some(34)
    );
    assert_eq!(
        physical_spell_action(12, PhysicalSpellActionRole::OrdinaryPickup),
        Some(37)
    );
    assert_eq!(
        [
            physical_spell_action(6, PhysicalSpellActionRole::PersistentPickup),
            physical_spell_action(11, PhysicalSpellActionRole::PersistentPickup),
            physical_spell_action(12, PhysicalSpellActionRole::PersistentPickup),
        ],
        [Some(20), Some(35), Some(38)]
    );

    let acquire_ordinary = |session: &mut Level2ResetSession,
                            authored_index: u16,
                            family: u8,
                            source_action: u8,
                            holder_action: u8| {
        let source = session
            .economy()
            .spell_pickups()
            .iter()
            .find(|source| source.authored_index == authored_index)
            .copied()
            .expect("authored ordinary spell source");
        assert_eq!(source.family, family);
        assert_eq!(source.action, source_action);
        let source_slot = source.source_entity_index;
        let baseline_replacements = session.economy().spell_replacements().to_vec();
        let baseline_persistent_sources = persistent_spell_sources(session);
        let player_entity_index = session.state.entity_index;
        session
            .set_level2_entity_position(player_entity_index, source.position)
            .expect("qualified player-position input");
        let acquired = (0..4).find_map(|_| {
            session.step_outer_turn(PlayerCommand::default(), 0);
            session
                .dispatch_events()
                .iter()
                .find_map(|event| match event {
                    Level2DispatchEvent::SpellAcquisition {
                        family: observed_family,
                        source_slot: observed_slot,
                        result: Some(outcome),
                        ..
                    } if *observed_family == family && *observed_slot == source_slot => {
                        Some(*outcome)
                    }
                    _ => None,
                })
        });
        let outcome = acquired.expect("ordinary spell source acquisition");
        assert_eq!(outcome.family, family);
        assert_eq!(outcome.source_slot, source_slot);
        assert_eq!(outcome.holder_action, holder_action);
        assert_eq!(outcome.replacement_action, None);
        assert_eq!(
            session.economy().spell_replacements(),
            baseline_replacements
        );
        assert_eq!(
            persistent_spell_sources(session),
            baseline_persistent_sources
        );
        let holder = session
            .shared_spells()
            .dynamic_pickups()
            .iter()
            .find(|entity| entity.index == source_slot)
            .expect("ordinary holder remains in the shared arena");
        assert_eq!(holder.action, holder_action);
        source_slot
    };

    for (authored_index, family, source_action, holder_action) in
        [(155, 6, 19, 18), (156, 11, 34, 33)]
    {
        let mut session = level_two_campaign_session();
        let baseline_campaign_hash = session.campaign_state_hash();
        let baseline_shared_hash = session.shared_spells().reset_hash();
        let baseline_economy_hash = session.economy().reset_hash();
        session
            .set_level2_selected_objective_stage_state(2)
            .expect("qualified Level 2 objective source selector");
        session.step_outer_turn(PlayerCommand::default(), 0);
        let source_slot = acquire_ordinary(
            &mut session,
            authored_index,
            family,
            source_action,
            holder_action,
        );
        assert!(persistent_spell_sources(&session).is_empty());
        assert!(session.economy().spell_replacements().is_empty());
        assert_ne!(session.campaign_state_hash(), baseline_campaign_hash);
        assert_ne!(session.shared_spells().reset_hash(), baseline_shared_hash);
        assert_ne!(session.economy().reset_hash(), baseline_economy_hash);
        assert!(session.dispatch_events().iter().all(|event| {
            !matches!(
                event,
                Level2DispatchEvent::DynamicArena {
                    source_slot: observed_slot,
                    kind: Level2DynamicArenaKind::SpellReplacement,
                    ..
                } if *observed_slot == source_slot
            )
        }));

        session.reset();
        assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
        assert_eq!(session.shared_spells().reset_hash(), baseline_shared_hash);
        assert_eq!(session.economy().reset_hash(), baseline_economy_hash);
        assert!(session.economy().spell_replacements().is_empty());
        assert!(persistent_spell_sources(&session).is_empty());
    }

    let mut family12_session = level_two_campaign_session();
    let baseline_campaign_hash = family12_session.campaign_state_hash();
    let baseline_shared_hash = family12_session.shared_spells().reset_hash();
    let baseline_economy_hash = family12_session.economy().reset_hash();
    let baseline_persistent_sources = persistent_spell_sources(&family12_session);
    let (source_class, source_model, source_action, source_position) = family12_session
        .shared_spells()
        .pickups()
        .iter()
        .find(|source| source.index == 38)
        .map(|source| (source.class, source.model, source.action, source.position))
        .expect("qualified reset family-12 source");
    assert_eq!(source_class, 15);
    assert_eq!(source_model, 12);
    assert_eq!(source_action, 37);
    let player_entity_index = family12_session.state.entity_index;
    family12_session
        .set_level2_entity_position(player_entity_index, source_position)
        .expect("qualified player-position input");
    let acquired = (0..4).find_map(|_| {
        family12_session.step_outer_turn(PlayerCommand::default(), 0);
        family12_session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Level2DispatchEvent::SpellAcquisition {
                    family: 12,
                    source_slot: 38,
                    result: Some(outcome),
                    ..
                } => Some(*outcome),
                _ => None,
            })
    });
    let outcome = acquired.expect("ordinary family-12 source acquisition");
    assert_eq!(outcome.family, 12);
    assert_eq!(outcome.source_slot, 38);
    assert_eq!(outcome.holder_action, 36);
    assert_eq!(outcome.replacement_action, None);
    assert_eq!(family12_session.shared_spells().pickups()[0].action, 36);
    assert!(family12_session.economy().spell_replacements().is_empty());
    assert_eq!(
        persistent_spell_sources(&family12_session),
        baseline_persistent_sources
    );
    assert_ne!(
        family12_session.campaign_state_hash(),
        baseline_campaign_hash
    );
    assert_ne!(
        family12_session.shared_spells().reset_hash(),
        baseline_shared_hash
    );
    assert_ne!(
        family12_session.economy().reset_hash(),
        baseline_economy_hash
    );

    family12_session.reset();
    assert_eq!(
        family12_session.campaign_state_hash(),
        baseline_campaign_hash
    );
    assert_eq!(
        family12_session.shared_spells().reset_hash(),
        baseline_shared_hash
    );
    assert_eq!(
        family12_session.economy().reset_hash(),
        baseline_economy_hash
    );
    assert_eq!(family12_session.shared_spells().pickups()[0].action, 37);
    assert!(family12_session.economy().spell_replacements().is_empty());
    assert!(persistent_spell_sources(&family12_session).is_empty());
}

#[test]
fn level_one_v2_rejects_level_two_extra_members() {
    let (package, closure) = fixture_paths();
    let temp = TempPackage::copy_from(&package, &closure);
    let l2 = level_two_fixture_path();
    fs::copy(
        l2.join("feature-start.bin"),
        temp.package.join("feature-start.bin"),
    )
    .unwrap();
    let manifest_path = temp.package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let l2_manifest: Value =
        serde_json::from_slice(&fs::read(l2.join("manifest.json")).unwrap()).unwrap();
    manifest["files"]["feature-start.bin"] = l2_manifest["files"]["feature-start.bin"].clone();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let error = NativeWorld::load(&temp.package, &temp.closure).unwrap_err();
    assert!(error.to_string().contains("exact typed package member set"));
}

#[test]
fn native_world_delegates_exact_legacy_terrain_queries() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    for position in [
        Position { x: 0, y: 0, z: 0 },
        Position {
            x: 19_840,
            y: 56_960,
            z: 5_024,
        },
        Position {
            x: 0xffff,
            y: 0xffff,
            z: -1,
        },
        Position {
            x: 0x7f55,
            y: 0x80aa,
            z: 1_234,
        },
    ] {
        assert_eq!(
            world.ground_height(position),
            world.terrain.ground_height(position)
        );
        assert_eq!(
            world.material_mask(position),
            world.terrain.material_mask(position)
        );
    }
}

#[test]
fn level_two_possession_holder_dispatch_phase_advances_once_and_replays_after_reset() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Possession holder session");
    session.select_level2_campaign_entry_stage();

    let holder = |session: &Level2ResetSession| {
        session
            .shared_spells()
            .dynamic_pickups
            .iter()
            .find(|candidate| {
                candidate.index == 116
                    && candidate.active
                    && candidate.class == 15
                    && candidate.model == POSSESSION_SPELL_FAMILY
                    && candidate.action == POSSESSION_SPELL_FAMILY.saturating_mul(3)
            })
            .cloned()
            .expect("ordinary local Possession holder 116")
    };
    let baseline_holder = holder(&session);
    let baseline_hash = session.campaign_state_hash();
    let baseline_shared_hash = session.shared_spells().reset_hash();
    assert_eq!(baseline_holder.active_value, 0);
    assert_eq!(baseline_holder.record[66], baseline_holder.phase);

    // PlayerEvents arms the holder before its later source-slot 116
    // callback. The same ordinary pass therefore consumes W=3 -> 2 and
    // allocates the class-9/model-1 projectile before the phase epilogue.
    let request = PlayerCommand {
        opcode: 6,
        argument_1: 1,
        flags: 0x40,
        argument_6: 17,
        argument_8: (-23_i16).cast_unsigned(),
        ..PlayerCommand::default()
    };
    session.state.entity_fov = 29;
    let mut expected_launch = session.state.position;
    crate::move_entity(
        &mut expected_launch,
        session.state.yaw.wrapping_sub(512),
        0,
        256,
    );
    if session.canonical_terrain().ground_height(expected_launch) > expected_launch.z {
        expected_launch = session.state.position;
    }
    expected_launch.z = expected_launch.z.wrapping_add(session.state.entity_fov);
    let mut expected_axis_position = session.state.position;
    crate::move_entity(
        &mut expected_axis_position,
        session.state.yaw.wrapping_add(17),
        session.state.display_pitch.wrapping_sub(23),
        10_240,
    );
    let mut expected_first_callback_position = expected_launch;
    crate::move_entity(
        &mut expected_first_callback_position,
        session.state.yaw.wrapping_add(17),
        session.state.display_pitch.wrapping_sub(23),
        384_i16.wrapping_add(session.state.actual_speed),
    );
    let ground_height = session
        .canonical_terrain()
        .ground_height(expected_first_callback_position);
    if ground_height > expected_first_callback_position.z {
        expected_first_callback_position.z = ground_height;
    }
    if let Some(cave_limit) = session
        .canonical_terrain()
        .ceiling_height(expected_first_callback_position)
        .map(|height| height.wrapping_sub(POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT))
        && cave_limit < expected_first_callback_position.z
    {
        expected_first_callback_position.z = cave_limit;
    }
    session.step_outer_turn(request, 0);
    let armed_holder = holder(&session);
    assert_eq!(
        armed_holder.active_value, 2,
        "same-pass player111 -> Possession116 consumes W=3 to W=2"
    );
    assert_eq!(
        armed_holder.phase,
        baseline_holder.phase.wrapping_add(1),
        "same-pass holder callback advances byte_0x3E exactly once"
    );
    assert_eq!(armed_holder.record[66], armed_holder.phase);
    assert_eq!(armed_holder.activation_value, 3);
    let first_projectile = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.active)
        .copied()
        .expect("same-pass Possession callback allocates one live projectile");
    assert_eq!(
        first_projectile.position, expected_first_callback_position,
        "the production selected request applies sub_68E50 hand displacement, strict terrain rollback, parent FOV, then the first model-1 callback"
    );
    assert_eq!(
        (first_projectile.yaw, first_projectile.pitch),
        (
            session.state.yaw.wrapping_add(17) & 0x07ff,
            session.state.display_pitch.wrapping_sub(23) & 0x07ff,
        ),
        "the production flyer carries the player's wrapping A4 yaw/pitch sums"
    );
    assert_eq!(
        first_projectile.axis_position, expected_axis_position,
        "the persistent model-1 axis projects 10240 units from the raw parent position"
    );
    assert_ne!(session.shared_spells().reset_hash(), baseline_shared_hash);

    // The following ordinary source pass consumes W=2 -> 1. It is
    // nonfresh, so it must not allocate a second projectile.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let second_pass_holder = holder(&session);
    assert_eq!(second_pass_holder.active_value, 1);
    assert_eq!(
        second_pass_holder.phase,
        armed_holder.phase.wrapping_add(1),
        "the following holder callback advances exactly one additional phase"
    );
    assert_eq!(second_pass_holder.record[66], second_pass_holder.phase);
    let second_projectile = session
        .economy()
        .possession_projectiles()
        .iter()
        .find(|projectile| projectile.active)
        .copied()
        .expect("the first Possession projectile remains live");
    assert_eq!(
        second_projectile.source_entity_index, first_projectile.source_entity_index,
        "nonfresh W=2 callback does not allocate another projectile"
    );
    let stepped_hash = session.campaign_state_hash();
    let stepped_shared_hash = session.shared_spells().reset_hash();
    assert_ne!(stepped_hash, baseline_hash);

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(session.shared_spells().reset_hash(), baseline_shared_hash);
    assert_eq!(holder(&session), baseline_holder);

    session.state.entity_fov = 29;
    session.step_outer_turn(request, 0);
    assert_eq!(holder(&session), armed_holder);
    assert_eq!(
        session
            .economy()
            .possession_projectiles()
            .iter()
            .find(|projectile| projectile.active)
            .map(|projectile| projectile.source_entity_index),
        Some(first_projectile.source_entity_index)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.campaign_state_hash(), stepped_hash);
    assert_eq!(session.shared_spells().reset_hash(), stepped_shared_hash);
    assert_eq!(holder(&session), second_pass_holder);

    // The source field is a byte, so the dispatcher uses byte wrapping
    // rather than saturating after every ordinary callback.  The first
    // two assertions above exclude skipped/double callbacks; this full
    // byte orbit pins the wrapping rule without a test-only phase setter.
    session.reset();
    for _ in 0..=u8::MAX {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let wrapped_holder = holder(&session);
    assert_eq!(wrapped_holder.phase, baseline_holder.phase);
    assert_eq!(wrapped_holder.record[66], baseline_holder.phase);
}
