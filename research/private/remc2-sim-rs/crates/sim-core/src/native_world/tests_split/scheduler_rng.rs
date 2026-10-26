#[test]
fn level_two_outer_turn_dispatches_typed_source_rows() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 production dispatch session");
    let pickup_position = session.shared_spells().pickups()[1].position;
    let baseline_hash = session.shared_spells().reset_hash();
    session.state.position = pickup_position;

    let evidence = session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(evidence.len(), 1);
    assert_eq!(session.shared_spells().pickups()[1].action, 9);
    assert!(session.shared_spells().speed_pickup_state().collected);
    assert_eq!(
        session.shared_spells().pickup_events(),
        &[Level2SharedSpellEvent::SpellCollected {
            source_slot: 75,
            player_entity_index: session.state.entity_index,
            family: SPEED_SPELL_FAMILY,
        }]
    );
    assert!(matches!(
        session.shared_spells().activation_events(),
        [FlightEvent::PrepareSound { sound_id: 18, .. }]
    ));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::SharedPickup {
                pass: 0,
                source_slot: 75,
                result: Some(Level2SpeedPickupStep {
                    collected: true,
                    ..
                }),
            }
        )
    }));
    let dispatch_hash = session.dispatch_observability_hash();
    session.reset();
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
    assert_eq!(session.shared_spells().pickups()[1].action, 10);
    assert!(session.dispatch_events().is_empty());
    assert_ne!(session.dispatch_observability_hash(), dispatch_hash);
}

#[test]
fn level_two_group31_tree_constructor_preserves_source_order_and_draws() {
    let mut session = level_two_campaign_session();
    let initial_global = session.economy().global_rng;
    session.runtime.set_level2_is_level_end(1);
    session.step_outer_turn(PlayerCommand::default(), 0);

    let pass_global = level2_next_rng32(initial_global);
    let trees = session.economy().authored_trees().to_vec();
    assert_eq!(
        trees
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        LEVEL2_GROUP31_TREE_INDICES
    );
    for tree in trees {
        let feature = session
            .mission()
            .authored_features()
            .iter()
            .copied()
            .find(|feature| feature.index == tree.authored_index)
            .expect("qualified group-31 tree feature");
        let base_position = feature.world_position(
            session
                .canonical_terrain()
                .ground_height(feature.world_position(0)),
        );
        let initial_rng = tree.source_entity_index.wrapping_add(pass_global as u16);
        let life_rng = level2_next_rng16(initial_rng);
        let x_rng = level2_next_rng16(life_rng);
        let y_rng = level2_next_rng16(x_rng);
        let particle_rng = level2_next_rng16(y_rng);
        let x_offset = i16::try_from(x_rng & 0x3f)
            .expect("tree x constructor draw fits")
            .wrapping_sub(32);
        let y_offset = i16::try_from(y_rng & 0x3f)
            .expect("tree y constructor draw fits")
            .wrapping_sub(32);
        assert_eq!(
            (
                tree.source_class,
                tree.model,
                tree.action,
                tree.entity_id,
                tree.dword_0x10,
                tree.source_byte_38,
                tree.max_life,
                tree.life,
            ),
            (
                2,
                0,
                0,
                tree.source_entity_index,
                i32::from(tree.source_entity_index % 11),
                1,
                300,
                300,
            )
        );
        assert_eq!(
            (
                tree.entity_rng,
                tree.particle_index,
                tree.yaw,
                tree.pitch,
                tree.roll,
                tree.fov,
                tree.phase,
                tree.control_flags,
            ),
            (
                particle_rng,
                if particle_rng & 1 != 0 { 84 } else { 83 },
                400,
                0,
                0,
                400,
                (tree.source_entity_index as u8).wrapping_add(1),
                0x0002_000c,
            )
        );
        assert_eq!(
            tree.position,
            Position {
                x: base_position.x.wrapping_add(x_offset as u16),
                y: base_position.y.wrapping_add(y_offset as u16),
                z: session.canonical_terrain().ground_height(Position {
                    x: base_position.x.wrapping_add(x_offset as u16),
                    y: base_position.y.wrapping_add(y_offset as u16),
                    z: base_position.z,
                }),
            }
        );
        assert_eq!(
            session.economy().source_rng.get(&tree.source_entity_index),
            Some(&particle_rng)
        );
        let map_chain = session
            .economy()
            .map_chains()
            .entity_chain((tree.position.x >> 8) as u8, (tree.position.y >> 8) as u8);
        assert!(
            map_chain.contains(&tree.source_entity_index),
            "tree row {} must be linked at its constructor position",
            tree.authored_index
        );
    }
}

#[test]
fn level_two_group31_tree_callback_consumes_rng_after_following_pass_gate() {
    let mut session = level_two_campaign_session();
    session.runtime.set_level2_is_level_end(1);

    // The first pass is the ordinary producer transition and source walk.
    // All trees are dispatched in that same pass, but their owner mailbox
    // is empty, so neither forbidden draw is reachable yet.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let tree_before = session
        .economy()
        .authored_trees()
        .iter()
        .find(|source| source.authored_index == 260)
        .copied()
        .expect("group-31 row 260 tree");
    assert_eq!(tree_before.owner_entity_index, 0);
    assert_eq!(tree_before.action, 0);
    let no_owner_rng = tree_before.entity_rng;
    assert!(!session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            ..
        } if *source_entity_index == tree_before.source_entity_index
    )));

    // The owner/life gate is written between source passes, just as the
    // ordinary damage mailbox is. The following pass reaches the same
    // tree callback and is the first pass allowed to allocate its sphere.
    let owner_entity_index = session.state.entity_index;
    {
        let mut economy_view = session.economy_mut();
        let tree = economy_view
            .authored_trees
            .iter_mut()
            .find(|source| source.source_entity_index == tree_before.source_entity_index)
            .expect("mutable group-31 row 260 tree");
        tree.owner_entity_index = owner_entity_index;
        tree.pending_damage = tree.life + 1;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);

    let tree_after = session
        .economy()
        .authored_trees()
        .iter()
        .find(|source| source.source_entity_index == tree_before.source_entity_index)
        .copied()
        .expect("group-31 row 260 remains live after callback");
    let expected_rng = level2_next_rng16(no_owner_rng);
    assert_eq!(tree_after.entity_rng, expected_rng);
    assert_eq!(tree_after.life, i32::from(expected_rng % 0x3c) + 130);
    assert_eq!(tree_after.action, 1);
    assert_eq!(tree_after.owner_entity_index, 0);
    assert_ne!(tree_after.last_sphere_source_entity_index, 0);
    assert_eq!(tree_after.last_sphere_word_0x2c, 300);
    assert_eq!(
        tree_after.last_sphere_position_z,
        if tree_before.position.z <= 128 {
            0
        } else {
            tree_before.position.z - 128
        }
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            pass: 0,
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredTree,
        } if *source_slot == tree_before.source_entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Economy {
            pass: 0,
            source_slot,
            event: Level2EconomyEvent::ManaSphereSpawned { .. },
        } if *source_slot == tree_before.source_entity_index
    )));
    let sphere = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == tree_after.last_sphere_source_entity_index)
        .copied()
        .expect("tree callback sphere remains in the following pass");
    let owner_id = session
        .economy
        .entities
        .get(usize::from(owner_entity_index))
        .map_or(owner_entity_index, |entity| entity.source_id);
    assert_eq!(sphere.id, u32::from(owner_id));
    assert_eq!(sphere.mana, 512);
}

#[test]
fn level_two_group31_tree_callback_rejects_nonlethal_and_failed_sphere_without_rng() {
    let mut nonlethal = level_two_campaign_session();
    nonlethal.runtime.set_level2_is_level_end(1);
    nonlethal.step_outer_turn(PlayerCommand::default(), 0);
    let nonlethal_before = nonlethal
        .economy()
        .authored_trees()
        .iter()
        .find(|source| source.authored_index == 260)
        .copied()
        .expect("nonlethal group-31 tree");
    let nonlethal_owner = nonlethal.state.entity_index;
    {
        let mut economy_view = nonlethal.economy_mut();
        let tree = economy_view
            .authored_trees
            .iter_mut()
            .find(|source| source.source_entity_index == nonlethal_before.source_entity_index)
            .expect("mutable nonlethal tree");
        tree.owner_entity_index = nonlethal_owner;
        tree.pending_damage = 0;
    }
    nonlethal.step_outer_turn(PlayerCommand::default(), 0);
    let nonlethal_after = nonlethal
        .economy()
        .authored_trees()
        .iter()
        .find(|source| source.source_entity_index == nonlethal_before.source_entity_index)
        .copied()
        .expect("nonlethal tree remains live");
    assert_eq!(nonlethal_after.entity_rng, nonlethal_before.entity_rng);
    assert_eq!(nonlethal_after.action, 0);
    assert_eq!(nonlethal_after.owner_entity_index, 0);
    assert_eq!(nonlethal_after.last_sphere_source_entity_index, 0);
    assert!(!nonlethal.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            ..
        } if *source_entity_index == nonlethal_before.source_entity_index
    )));

    let mut allocation_failure = level_two_campaign_session();
    allocation_failure.runtime.set_level2_is_level_end(1);
    allocation_failure.step_outer_turn(PlayerCommand::default(), 0);
    // Complete ordinary following-pass reclamation before establishing the
    // failed-allocation prestate; clearing a free stack cannot occupy a row
    // that the next source prologue will lawfully release.
    allocation_failure.step_outer_turn(PlayerCommand::default(), 0);
    allocation_failure
        .economy_mut()
        .replacement_cursor_exclusions = (1_u16..1_000).collect();
    for _ in 1_u16..1_000 {
        if allocation_failure
            .allocate_level2_authored_class14(244)
            .is_none()
        {
            break;
        }
    }
    assert!(
        allocation_failure
            .allocate_level2_authored_class14(244)
            .is_none()
    );
    assert!(allocation_failure.economy().free_slots.is_empty());
    let allocation_before = allocation_failure
        .economy()
        .authored_trees()
        .iter()
        .find(|source| source.authored_index == 260)
        .copied()
        .expect("allocation-failure group-31 tree");
    let allocation_failure_owner = allocation_failure.state.entity_index;
    {
        let mut economy_view = allocation_failure.economy_mut();
        let tree = economy_view
            .authored_trees
            .iter_mut()
            .find(|source| source.source_entity_index == allocation_before.source_entity_index)
            .expect("mutable allocation-failure tree");
        tree.owner_entity_index = allocation_failure_owner;
        tree.pending_damage = tree.life + 1;
    }
    allocation_failure.step_outer_turn(PlayerCommand::default(), 0);
    let allocation_after = allocation_failure
        .economy()
        .authored_trees()
        .iter()
        .find(|source| source.source_entity_index == allocation_before.source_entity_index)
        .copied()
        .expect("allocation-failure tree remains live");
    assert_eq!(allocation_after.entity_rng, allocation_before.entity_rng);
    assert_eq!(allocation_after.action, 0);
    assert_eq!(allocation_after.owner_entity_index, 0);
    assert_eq!(allocation_after.last_sphere_source_entity_index, 0);
    assert_eq!(allocation_failure.economy().authored_trees().len(), 12);
    assert!(
        !allocation_failure
            .economy()
            .events()
            .iter()
            .any(|event| matches!(
                event,
                Level2EconomyEvent::ManaSphereSpawned {
                    source_entity_index,
                    ..
                } if *source_entity_index == allocation_before.source_entity_index
            ))
    );
}

#[test]
fn level_two_group31_reclaimed_lower_slot_defers_until_next_pass() {
    let mut session = level_two_campaign_session();
    let reset_free_slots = session.economy().free_slots.clone();
    let row_248 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 248)
        .copied()
        .expect("authored group-28 scroll row");
    let row_274 = session
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 274)
        .copied()
        .expect("group-31 row-274 switch");
    let reclaimed_source = session
        .allocate_level2_authored_class14(row_248.index)
        .expect("source-shaped class-14 allocation");
    let reclaimed_slot = reclaimed_source.source_entity_index;
    let reclaimed_generation = session.economy().slot_generation(reclaimed_slot);
    session.runtime.set_level2_is_level_end(1);
    session
        .set_level2_entity_position(111, reclaimed_source.position)
        .expect("move public player input onto the reclaim candidate");

    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden = session
        .economy()
        .authored_class14()
        .iter()
        .find(|source| source.source_entity_index == reclaimed_slot)
        .expect("collected lower-slot row remains through the callback pass");
    assert!(!hidden.drawing_enabled);
    assert!(session.economy().map_chains().is_linked(reclaimed_slot));
    assert!(!session.economy().free_slots.contains(&reclaimed_slot));
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
    )));
    let runtime_switch_slot = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.authored_index == 274)
        .map(|source| source.source_entity_index)
        .expect("row 274 runtime producer remains live");
    assert!(reclaimed_slot < runtime_switch_slot);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().free_slots.contains(&reclaimed_slot));
    assert!(!session.economy().map_chains().is_linked(reclaimed_slot));
    let switch_phase = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|source| source.source_entity_index == runtime_switch_slot)
        .map(|source| source.phase)
        .expect("row 274 remains live after the reclaim boundary");

    // The real sub_4A1E0 rebuild chooses the lowest current free row.
    // Earlier imported source reclamation may add a still-lower neighbor;
    // occupy only those actual candidates through the qualified protected
    // portal constructor so this fixture isolates reuse of its own scroll.
    session.economy_mut().rebuild_free_arena(&reset_free_slots);
    while session
        .economy()
        .free_slots
        .last()
        .is_some_and(|slot| *slot < reclaimed_slot)
    {
        let expected_slot = *session
            .economy()
            .free_slots
            .last()
            .expect("lower free source row");
        let occupied = session
            .allocate_level2_authored_class14(244)
            .expect("real protected carrier occupies the lower free neighbor");
        assert_eq!(occupied.source_entity_index, expected_slot);
    }
    assert_eq!(session.economy().free_slots.last(), Some(&reclaimed_slot));
    session
        .set_level2_entity_position(111, row_274.world_position(0))
        .expect("move public player input onto row 274");
    let turns_until_cadence = level_two_switch_turns_until_cadence(switch_phase);
    for _ in 0..turns_until_cadence.saturating_sub(1) {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated { group: 28, .. }
    )));
    session.step_outer_turn(PlayerCommand::default(), 0);

    let deferred_row = session
        .economy()
        .authored_class14()
        .iter()
        .find(|source| source.authored_index == 275)
        .copied()
        .expect("reclaimed lower-slot child remains for the next cursor pass");
    assert_eq!(deferred_row.source_entity_index, reclaimed_slot);
    assert_eq!(
        session.economy().slot_generation(reclaimed_slot),
        reclaimed_generation.wrapping_add(1),
        "NewEvent advances the real reclaimed occurrence exactly once"
    );
    assert!(deferred_row.source_entity_index < runtime_switch_slot);
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena { source_slot, .. }
            if *source_slot == reclaimed_slot
    )));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden_deferred_row = session
        .economy()
        .authored_class14()
        .iter()
        .find(|source| source.authored_index == 275)
        .expect("lower-slot birth is visited and hidden on the next pass");
    assert!(!hidden_deferred_row.drawing_enabled);
    assert!(session.economy().map_chains().is_linked(reclaimed_slot));
    assert!(!session.economy().free_slots.contains(&reclaimed_slot));
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .economy()
            .authored_class14()
            .iter()
            .any(|source| source.authored_index == 275)
    );
    assert!(session.economy().free_slots.contains(&reclaimed_slot));
}

#[test]
fn level_two_bee_row229_action17_moves_on_outer_cursor_and_reset_restores_hash() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    fire_level_two_model32_group(&mut session, 21);
    let row229_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(LEVEL2_BEE_OBJECTIVE_AUTHORED_INDEX))
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee");
    let before = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("row-229 Bee before action-17 turn");
    {
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == row229_slot)
            .expect("row-229 Bee source");
        source.action = 17;
        source.source_byte_3e = 1;
        source.source_byte_39 = 0;
        source.target_entity_index = None;
        source.control_flags = 8;
    }

    session.step_outer_turn(PlayerCommand::default(), 0);
    let after = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("row-229 Bee after action-17 turn");
    assert_eq!(after.action, 17);
    assert_eq!(after.source_byte_3e, 2);
    assert_ne!(
        (after.position.x, after.position.y),
        (before.position.x, before.position.y)
    );

    session.reset();
    assert!(session.economy().bee_creatures().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_death_runs_through_production_cursor() {
    let mut session = level_two_campaign_session();

    // Complete the real row-228 model-32 objective gate and materialize row
    // 229 in source order. Mutating only
    // the resulting typed row keeps the death callback on the production
    // dynamic cursor rather than calling a fixture-only helper.
    fire_level_two_model32_group(&mut session, 21);
    let row229_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(229))
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
            .expect("pre-kill row remains for the next source turn")
            .action,
        21
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    let phase_after_action21 = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .expect("phase-gated row remains live after action-21 callback")
        .source_byte_3e;
    assert_ne!(phase_after_action21 & 7, 0);
    assert!(!session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            ..
        } if *source_entity_index == row229_slot
    )));

    let mut reclaimed = false;
    let mut phase_zero_callback_seen = false;
    for _ in 0..10 {
        let before = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let after = session
            .economy()
            .bee_creatures()
            .iter()
            .find(|bee| bee.source_entity_index == row229_slot)
            .copied();
        let Some(after) = after else {
            assert!(phase_zero_callback_seen);
            reclaimed = true;
            break;
        };
        if before
            .expect("phase-gated row remains until the following boundary")
            .source_byte_3e
            & 7
            == 0
        {
            phase_zero_callback_seen = true;
            assert!(!after.active);
            assert!(!after.drawing_enabled);
        } else {
            assert!(after.active);
            assert!(after.drawing_enabled);
        }
    }
    assert!(reclaimed, "phase-zero callback reclaims the production row");
    assert!(phase_zero_callback_seen);
    assert!(session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            mana: 1500,
            ..
        } if *source_entity_index == row229_slot
    )));
}

#[test]
fn level_two_authored_bee_action23_selector10_rearms_through_production_cursor() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    // Materialize the authored row through the real group-21 producer,
    // then select only its non-L2-06 StageVar state. The next outer turn
    // still dispatches the callback through the live source cursor.
    fire_level_two_model32_group(&mut session, 21);
    let source_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(231))
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 231 Bee");
    let before = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == source_slot)
        .copied()
        .expect("live scripted Bee before selector-10 step");
    let mut economy_view = session.economy_mut();
    let bee = economy_view
        .bee_creatures
        .iter_mut()
        .find(|bee| bee.source_entity_index == source_slot)
        .expect("live scripted Bee source");
    bee.stage_var2 = 10;

    session.step_outer_turn(PlayerCommand::default(), 0);
    let after = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == source_slot)
        .copied()
        .expect("live scripted Bee after selector-10 step");
    assert_eq!(after.action, 18);
    assert_eq!(after.dword_0x10, 1);
    assert_eq!(after.entity_rng, before.entity_rng);
    assert_eq!(after.roll, before.roll);
    assert_eq!(after.source_byte_3e, before.source_byte_3e.wrapping_add(1));
    assert_eq!(after.phase, before.phase.wrapping_add(1));

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_actions17_to18_run_through_production_cursor() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    // Complete the real row-228 model-32 objective gate and materialize row
    // 229 through the authored activation
    // graph. Mutating only that live typed row selects a discriminating
    // action state; all callback execution below remains on the normal
    // outer-turn dynamic cursor. Reset the source cadence so action 17
    // performs its real two-draw awareness scan instead of relying on a
    // hand-seeded mailbox.
    fire_level_two_model32_group(&mut session, 21);
    let row229_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(229))
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee");
    let local_player_slot = session.state.entity_index;
    let initial = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production row 229 source");
    // Action 17 now runs the source mover before its awareness scan. Put
    // the live player one Bee step down the initial heading so the
    // source-order target remains inside the descriptor-73 cone after
    // that movement prelude.
    let mut target_after_move = initial.position;
    crate::move_entity(&mut target_after_move, initial.yaw, 0, initial.actual_speed);
    session.state.position = target_after_move;
    {
        let mut economy_view = session.economy_mut();
        let source = economy_view
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == row229_slot)
            .expect("production row 229 source");
        source.action = 17;
        source.source_byte_3e = 0;
        // The real SP prepass decrements byte39 before the callback.
        source.source_byte_39 = 2;
        source.target_entity_index = Some(local_player_slot);
    }

    session.step_outer_turn(PlayerCommand::default(), 0);
    let action17 = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("action-17 Bee remains live");
    let wander_first = level2_next_rng16(initial.entity_rng);
    let wander_second = level2_next_rng16(wander_first);
    let expected_rng_after_action17 = level2_next_rng16(wander_second);
    assert_eq!(action17.action, 18);
    assert_eq!(action17.entity_rng, expected_rng_after_action17);
    assert_eq!(
        action17.last_sound_id,
        (expected_rng_after_action17 & 1) + 12
    );
    assert_eq!(action17.dword_0x10, 1);
    assert_eq!(action17.source_byte_3e, 1);

    // The first action-18 visit advances the source cadence but is not a
    // contact window: the source value is 1 after action 17, so the
    // source's `% action_window == 0` gate suppresses the hit callback.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let action18 = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("action-18 Bee remains live");
    assert_eq!(action18.action, 18);
    assert_eq!(action18.entity_rng, expected_rng_after_action17);
    assert_eq!(
        action18.last_sound_id,
        (expected_rng_after_action17 & 1) + 12
    );
    assert_eq!(action18.actual_speed, 5 * action18.minimum_speed / 2);
    assert_eq!(action18.dword_0x10, 0);
    assert_eq!(
        action18.source_byte_3e,
        action17.source_byte_3e.wrapping_add(1)
    );

    // A second production-cursor visit with source byte 3e reset to the
    // contact window proves the strict 3-D `< 1024` callback path.
    let mut contact_seed = action18;
    contact_seed.source_byte_3e = 0;
    contact_seed.control_flags |= LEVEL2_BEE_MOVEMENT_SKIP_BIT;
    contact_seed.position = Position {
        x: session.state.position.x.wrapping_add(1023),
        y: session.state.position.y,
        z: session.state.position.z,
    };
    assert!(session.economy.replace_authored_bee_for_test(contact_seed));
    session.step_outer_turn(PlayerCommand::default(), 0);
    let action18_contact = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("contacting action-18 Bee remains live");
    let expected_rng_after_contact = level2_next_rng16(expected_rng_after_action17);
    assert_eq!(action18_contact.action, 18);
    assert_eq!(action18_contact.entity_rng, expected_rng_after_contact);
    assert_eq!(
        action18_contact.last_sound_id,
        (expected_rng_after_contact & 1) + 12
    );
    assert_eq!(
        action18_contact.actual_speed,
        -action18_contact.maximum_speed
    );
    assert_eq!(
        action18_contact.dword_0x10,
        3 * i32::from(action18_contact.action_window)
    );
    assert_eq!(action18_contact.source_byte_3e, 1);

    // Clearing the explicit word_0x96 mailbox exercises the source's
    // action-18 target-loss fallback. This test proves production
    // consumption of the mailbox, not natural AI target ownership.
    session
        .economy_mut()
        .set_authored_bee_target(row229_slot, None);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let target_lost = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("target-lost Bee remains live");
    assert_eq!(target_lost.action, 17);
    assert_eq!(target_lost.actual_speed, target_lost.minimum_speed);

    session.reset();
    assert!(session.economy().bee_creatures().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_authored_bee_actions16_and19_run_through_production_cursor() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();

    // Complete the real row-228/model-32 objective gate. Mutating the resulting live row only selects the
    // action branch; dispatch remains on the production dynamic cursor.
    fire_level_two_model32_group(&mut session, 21);
    let row229_slot = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.authored_index() == Some(229))
        .map(|bee| bee.source_entity_index)
        .expect("production group-21 row 229 Bee");
    let local_player_slot = session.state.entity_index;
    let initial = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("production row 229 source");
    session.state.position = initial.position;
    session
        .economy_mut()
        .set_authored_bee_target(row229_slot, Some(local_player_slot));

    // sub1BD90 does not select a class3 target from word96. Off cadence it
    // settles without scanning peers, RNG or a transition to attack.
    {
        let mut economy = session.economy_mut();
        let source = economy
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == row229_slot)
            .unwrap();
        source.action = 16;
        source.source_byte_3e = 1;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let action16 = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("action-16 Bee remains live");
    assert_eq!(
        action16.action, 16,
        "word96 alone cannot cause a stationary attack"
    );
    assert_eq!(action16.dword_0x10, initial.dword_0x10);
    assert_eq!(action16.entity_rng, initial.entity_rng);
    assert_eq!(action16.last_sound_id, initial.last_sound_id);
    assert_eq!(action16.source_byte_3e, 2);

    // On cadence the stationary helper selects an unlinked same-model peer
    // using its reverse cone and stores word32, independently of word96.
    let peer = session
        .allocate_level2_authored_bee(230)
        .expect("ordinary Day peer constructor");
    let peer_position = Position {
        x: action16.position.x.wrapping_sub(500),
        ..action16.position
    };
    {
        let mut economy = session.economy_mut();
        let peer_row = economy
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == peer.source_entity_index)
            .unwrap();
        peer_row.position = peer_position;
        peer_row.linked_peer_entity_index = 0;
        let source = economy
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == row229_slot)
            .unwrap();
        source.source_byte_3e = 0;
        source.yaw = heading_for_positions(peer_position, source.position) as i16;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let linked = *session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .unwrap();
    assert_eq!(linked.action, 19);
    assert_eq!(linked.linked_peer_entity_index, peer.source_entity_index);
    assert_eq!(linked.entity_rng, action16.entity_rng);

    // sub1C560 inherits the peer's actual attack relation at cadence, clears
    // word32 and lets sub1F800 set the attack latch without an RNG draw.
    {
        let mut economy = session.economy_mut();
        let peer_row = economy
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == peer.source_entity_index)
            .unwrap();
        peer_row.action = 18;
        peer_row.stage_var1 = 0;
        peer_row.stage_var2 = 0;
        peer_row.target_entity_index = Some(local_player_slot);
        let source = economy
            .bee_creatures
            .iter_mut()
            .find(|bee| bee.source_entity_index == row229_slot)
            .unwrap();
        source.source_byte_3e = 30;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let action19 = session
        .economy()
        .bee_creatures()
        .iter()
        .find(|bee| bee.source_entity_index == row229_slot)
        .copied()
        .expect("action-19 Bee remains live");
    assert_eq!(action19.action, 18);
    assert_eq!(action19.target_entity_index, Some(local_player_slot));
    assert_eq!(action19.linked_peer_entity_index, 0);
    assert_eq!(action19.dword_0x10, 1);
    assert_eq!(action19.entity_rng, action16.entity_rng);
    assert_eq!(action19.last_sound_id, action16.last_sound_id);
    assert_eq!(action19.source_byte_3e, 31);

    session.reset();
    assert!(session.economy().bee_creatures().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_model53_action58_runs_through_production_cursor() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored group-3 feature")
            .world_position(0)
    };

    // Reach row 439 through row 11 -> row 42 -> group 3. The first
    // action-58 callback is therefore an ordinary production visit, not a
    // direct callback or fixture setter.
    let local_player = session.state.entity_index;
    level_two_reach_row42(
        &mut session,
        local_player,
        feature_position(11),
        feature_position(42),
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
            .set_level2_entity_position(local_player, feature_position(42))
            .expect("public player-position input remains on row 42");
        session.step_outer_turn(PlayerCommand::default(), 0);
        if session
            .economy()
            .authored_model53()
            .iter()
            .any(|source| source.authored_index == 439)
        {
            break;
        }
    }

    let row439 = session
        .economy()
        .authored_model53()
        .iter()
        .find(|source| source.authored_index == 439)
        .copied()
        .expect("production route must materialize row 439");
    assert_eq!(
        (
            row439.source_class,
            row439.model,
            row439.action,
            row439.max_life,
            row439.life,
            row439.subspell_index,
            row439.control_flags,
            row439.last_sound_id,
        ),
        (10, 53, 58, 6, 5, 3000, 0x0001_0002, 30)
    );
    assert_eq!(
        (
            row439.actual_speed,
            row439.dword_0x10,
            row439.shift_rot_x,
            row439.shift_rot_y,
            row439.byte_0x46,
            row439.sprite_index,
            row439.water_spawn_requested,
        ),
        (16, 2, 0, 512, 0, 0, false)
    );
    assert_eq!(
        session
            .economy()
            .source_rng
            .get(&row439.source_entity_index),
        Some(&row439.entity_rng)
    );

    // sub_10130 accepts the first three shape-0 cells; the fourth cell
    // advances the source iterator to its terminal return and is not
    // processed by the source loop.
    let children = session
        .economy()
        .model17_spawned_children()
        .iter()
        .filter(|child| child.parent_entity_index == row439.source_entity_index)
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3);
    for child in children {
        assert_eq!(
            (
                child.parent_entity_id,
                child.model,
                child.action,
                child.subspell_index,
                child.max_life,
                child.life,
                child.control_flags,
                child.shift_rot_x,
                child.shift_rot_y,
                child.yaw,
            ),
            // The newly allocated higher slots run later in this same numeric
            // pass. Their first action-0 visit preserves the constructor
            // payload and adds source control bit 1 (0x02).
            (row439.entity_id, 0, 0, 400, 8, 7, 0x0003_0082, 512, 512, 0),
            "same-pass model-0 child state for {child:?}"
        );
    }
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

    // A negative pre-decrement return consumes no action-58 RNG and emits
    // no child, while ApplyEvents still advances the source phase.
    session.reset();
    let negative = session
        .economy_mut()
        .allocate_authored_model53(439, feature_position(439))
        .expect("direct negative-branch source allocation");
    let negative_slot = negative.source_entity_index;
    let negative_rng = negative.entity_rng;
    let negative_phase = negative.phase;
    session
        .economy_mut()
        .authored_model53
        .iter_mut()
        .find(|source| source.source_entity_index == negative_slot)
        .expect("negative-branch source")
        .life = -1;
    let child_count = session.economy().model17_spawned_children().len();
    assert!(
        session
            .economy_mut()
            .step_authored_model53_slot(negative_slot)
    );
    let negative_after = session
        .economy()
        .authored_model53()
        .iter()
        .find(|source| source.source_entity_index == negative_slot)
        .copied()
        .expect("negative-branch source remains live");
    assert_eq!(negative_after.life, -2);
    assert_eq!(negative_after.entity_rng, negative_rng);
    assert_eq!(negative_after.phase, negative_phase.wrapping_add(1));
    assert_eq!(
        session.economy().model17_spawned_children().len(),
        child_count
    );
    session.reset();
    assert!(session.economy().authored_model53().is_empty());
    assert!(session.economy().model17_spawned_children().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_outer_turn_reaches_family12_pickup_at_source_slot38() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.shared_spells().reset_hash();
    let pickup_position = session.shared_spells().pickups()[0].position;
    let player_entity_index = session.state.entity_index;
    assert_eq!(session.shared_spells().pickups()[0].index, 38);
    assert_eq!(session.shared_spells().pickups()[0].action, 37);

    session.state.position = pickup_position;
    let first = session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(first.len(), 1);
    assert_eq!(session.shared_spells().pickups()[0].phase, 40);
    assert_eq!(session.economy.entities()[38].phase, 40);
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::SpellAcquisition {
            source_slot: 38,
            result: Some(_),
            ..
        }
    )));
    let second = session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(second.len(), 1);
    assert_eq!(session.shared_spells().pickups()[0].action, 36);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                pass: 0,
                source_slot: 38,
                family: 12,
                result: Some(outcome),
            } if outcome.family == 12
                && outcome.source_slot == 38
                && outcome.parent_entity_index == player_entity_index
                && outcome.holder_action == 36
        )
    }));
    let stepped_hash = session.shared_spells().reset_hash();
    assert_ne!(stepped_hash, baseline_hash);

    session.reset();
    assert_eq!(session.shared_spells().reset_hash(), baseline_hash);
    assert_eq!(session.shared_spells().pickups()[0].action, 37);
    assert!(session.shared_spells().pickup_events().is_empty());
    session.state.position = pickup_position;
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.shared_spells().reset_hash(), stepped_hash);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::SpellAcquisition {
                source_slot: 38,
                family: 12,
                result: Some(_),
                ..
            }
        )
    }));
}

#[test]
fn level_two_outer_turn_dispatches_each_source_pass_in_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    for (speed_index, expected_passes) in [(0_u8, 1_u8), (1, 4), (2, 8)] {
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 source-pass session");
        let authority_index = terrain_index(4, 4);
        session.canonical_terrain_mut().planes.ground_heights[authority_index] = 77;
        let authority_hash = session.terrain_hash();
        session.step_outer_turn(PlayerCommand::default(), speed_index);
        assert_eq!(session.terrain_hash(), authority_hash);
        let passes = session
            .dispatch_events()
            .iter()
            .map(|event| match event {
                Mc2DispatchEvent::SharedPickup { pass, .. }
                | Mc2DispatchEvent::SpellAcquisition { pass, .. }
                | Mc2DispatchEvent::BadStone { pass, .. }
                | Mc2DispatchEvent::House { pass, .. }
                | Mc2DispatchEvent::Villager { pass, .. }
                | Mc2DispatchEvent::Dolmen { pass, .. }
                | Mc2DispatchEvent::Castle { pass, .. }
                | Mc2DispatchEvent::DynamicArena { pass, .. }
                | Mc2DispatchEvent::Economy { pass, .. }
                | Mc2DispatchEvent::Goat { pass, .. }
                | Mc2DispatchEvent::Error { pass, .. }
                | Mc2DispatchEvent::AuthoredGroupActivated { pass, .. }
                | Mc2DispatchEvent::LevelExitSequenceStarted { pass, .. }
                | Mc2DispatchEvent::LevelExitSequenceCompleted { pass, .. } => *pass,
            })
            .collect::<Vec<_>>();
        assert!(!passes.is_empty(), "source pass {speed_index} was empty");
        assert!(passes.windows(2).all(|window| window[0] <= window[1]));
        for pass in 0..expected_passes {
            assert!(
                passes.contains(&pass),
                "speed {speed_index} omitted source pass {pass}; dispatch={:#?}",
                session.dispatch_events(),
            );
        }
        assert_eq!(passes.iter().max().copied(), Some(expected_passes - 1));
        session.reset();
        assert_eq!(session.canonical_terrain(), session.canonical_terrain());
        assert_eq!(session.canonical_terrain(), session.canonical_terrain());
        assert_eq!(session.canonical_terrain(), session.canonical_terrain());
    }
}

#[test]
fn level_two_outer_turn_advances_global_rng_per_source_sweep() {
    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 RNG session");
    let initial = inspection.runtime().random_state;
    for (speed_index, passes) in [(0_u8, 1_u32), (1, 4), (2, 8)] {
        let mut session = inspection
            .level2_reset_session()
            .expect("isolated Level 2 RNG session");
        session.step_outer_turn(PlayerCommand::default(), speed_index);
        let mut expected = initial;
        for _ in 0..passes {
            expected = expected.wrapping_mul(9_377).wrapping_add(9_439);
        }
        assert_eq!(session.economy().global_rng, expected);
    }
}

#[test]
fn level_two_outer_turn_records_source_dispatch_errors_and_continues() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 error-channel session");
    session.shared_spells_mut().player_entity.index = 112;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::Error {
                pass: 0,
                source_slot: 38,
                ..
            }
        )
    }));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::BadStone { pass: 0, .. }
                | Mc2DispatchEvent::House { pass: 0, .. }
                | Mc2DispatchEvent::Villager { pass: 0, .. }
                | Mc2DispatchEvent::Dolmen { pass: 0, .. }
        )
    }));
}

#[test]
fn level_two_outer_turn_scans_live_dynamic_arena_rows() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 dynamic arena session");
    let carrier_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("carrier allocation");
    let sphere_slot = u16::try_from(
        session
            .economy_mut()
            .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 10, 111)
            .expect("sphere allocation"),
    )
    .expect("sphere source slot fits u16");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let dynamic = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                pass: 0,
                source_slot,
                kind,
            } => Some((*source_slot, *kind)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let first_dynamic_slots = dynamic
        .iter()
        .map(|(slot, _)| *slot)
        .filter(|slot| seen.insert(*slot))
        .collect::<Vec<_>>();
    assert!(
        first_dynamic_slots
            .windows(2)
            .all(|window| window[0] < window[1])
    );
    assert!(dynamic.contains(&(carrier_slot, Level2DynamicArenaKind::ManaCarrier)));
    assert!(dynamic.contains(&(
        u16::try_from(sphere_slot).expect("sphere slot"),
        Level2DynamicArenaKind::ManaSphere,
    )));

    session.economy_mut().begin_turn();
    assert!(session.economy_mut().absorb_sphere_into_carrier(
        u16::try_from(sphere_slot).expect("sphere slot"),
        carrier_slot,
    ));
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(!session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaSphere,
                ..
            } if *source_slot == u16::try_from(sphere_slot).expect("sphere slot")
        )
    }));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::ManaCarrier,
                ..
            } if *source_slot == carrier_slot
        )
    }));
}

#[test]
fn level_two_scheduler_global_rng_draws_follow_source_boundaries() {
    let declared_ids = LEVEL2_SCHEDULER_RNG_ROWS
        .iter()
        .map(|(id, _, _)| *id)
        .collect::<Vec<_>>();
    assert_eq!(declared_ids.len(), 11);
    assert_eq!(
        declared_ids,
        vec![
            "RNG-L2-0464",
            "RNG-L2-0465",
            "RNG-L2-0477",
            "RNG-L2-0478",
            "RNG-L2-0479",
            "RNG-L2-0480",
            "RNG-L2-0489",
            "RNG-L2-0490",
            "RNG-L2-0491",
            "RNG-L2-0492",
            "RNG-L2-0493",
        ]
    );
    let unique_ids = declared_ids.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(unique_ids.len(), declared_ids.len());
    assert_eq!(
        LEVEL2_SCHEDULER_RNG_ROWS
            .iter()
            .map(|(_, token, _)| *token)
            .collect::<Vec<_>>(),
        vec![
            "remc2/engine/EventsFunctions.cpp:62474",
            "remc2/engine/EventsFunctions.cpp:62475",
            "remc2/engine/Events.cpp:420",
            "remc2/engine/Events.cpp:578",
            "remc2/engine/Events.cpp:604",
            "remc2/engine/Terrain.cpp:22",
            "remc2/engine/Basic.cpp:2997",
            "remc2/engine/Basic.cpp:3035",
            "remc2/engine/Basic.cpp:3190",
            "remc2/engine/Basic.cpp:3228",
            "remc2/engine/Sound.cpp:6333",
        ]
    );

    let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
        .expect("qualified Level 2 scheduler/global RNG package");
    let initial_global = inspection.runtime().random_state;
    let mut session = level_two_campaign_session();
    let baseline = (
        session.campaign_state_hash(),
        session.dispatch_observability_hash(),
        session.economy().reset_hash(),
        session.economy().global_rng,
        session.economy.entities.clone(),
        session.economy().free_slots.clone(),
    );

    // GenerateLevelMap writes the package runtime seed and the two
    // file-scope Basic.cpp copies carry that same global/entity projection
    // into and back out of the session reset. No secondary RNG carrier is
    // allowed to appear at this boundary (RNG-L2-0480, 0489..0492).
    assert_eq!(baseline.3, initial_global);
    assert_eq!(
        session
            .economy
            .entities
            .iter()
            .map(|entity| (entity.index, entity.entity_rng))
            .collect::<Vec<_>>(),
        inspection
            .entities()
            .iter()
            .map(|entity| (entity.index, entity.entity_rng))
            .collect::<Vec<_>>()
    );

    // ApplyEvents advances the full-width session stream exactly once
    // before its ascending source walk (RNG-L2-0477). The qualified idle
    // pass has no tree row, no dynamic row, and no sound-46 emission, so
    // no later branch is permitted to perturb this transition.
    session.step_outer_turn(PlayerCommand::default(), 0);
    let pass_one_global = level2_next_rng32(initial_global);
    assert_eq!(session.economy().global_rng, pass_one_global);

    // NewEvent's ordinary free-stack branch seeds a carrier with the
    // allocated slot plus the post-ApplyEvents global low word
    // (RNG-L2-0478). The sphere constructor uses the same NewEvent seed
    // before its two source-local draws (RNG-L2-0479).
    let carrier_slot = session
        .economy()
        .free_slots
        .last()
        .copied()
        .expect("ordinary NewEvent free slot");
    let carrier_id = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("ordinary NewEvent carrier allocation");
    assert_eq!(carrier_id, carrier_slot);
    let carrier = session
        .economy()
        .carriers()
        .iter()
        .find(|carrier| carrier.source_entity_index == carrier_slot)
        .expect("allocated carrier row");
    assert_eq!(
        carrier.entity_rng,
        carrier_slot.wrapping_add(pass_one_global as u16)
    );

    let sphere_slot = session
        .economy()
        .free_slots
        .last()
        .copied()
        .expect("second ordinary NewEvent free slot");
    let sphere_id = session
        .economy_mut()
        .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x3456, 10, 111)
        .expect("ordinary NewEvent sphere allocation");
    assert_eq!(sphere_id, u32::from(sphere_slot));
    let sphere_seed = sphere_slot.wrapping_add(pass_one_global as u16);
    let sphere = session
        .economy()
        .spheres()
        .iter()
        .find(|sphere| sphere.source_entity_index == sphere_slot)
        .expect("allocated sphere row");
    assert_eq!(
        sphere.entity_rng,
        level2_next_rng16(level2_next_rng16(sphere_seed))
    );

    // Both rows are visible in the same ordinary source pass in ascending
    // source-slot order. Absorbing the sphere disables it after that
    // callback; the following pass owns reclaim/index rebuild, not the
    // same-pass producer (UpdateEntities_57730 boundary).
    session.step_outer_turn(PlayerCommand::default(), 0);
    let mut dynamic_slots = session
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                pass: 0,
                source_slot,
                kind: Level2DynamicArenaKind::ManaCarrier | Level2DynamicArenaKind::ManaSphere,
            } if *source_slot == carrier_slot || *source_slot == sphere_slot => Some(*source_slot),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_dynamic_slots = vec![carrier_slot, sphere_slot];
    expected_dynamic_slots.sort_unstable();
    assert_eq!(dynamic_slots.len(), 2);
    dynamic_slots.sort_unstable();
    assert_eq!(dynamic_slots, expected_dynamic_slots);
    assert_eq!(
        session.economy().global_rng,
        level2_next_rng32(pass_one_global)
    );

    session.economy_mut().begin_turn();
    assert!(
        session
            .economy_mut()
            .absorb_sphere_into_carrier(sphere_slot, carrier_slot)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().free_slots.contains(&sphere_slot));
    assert!(
        !session
            .economy()
            .spheres()
            .iter()
            .any(|sphere| sphere.source_entity_index == sphere_slot)
    );
    assert!(!session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot: slot,
                kind: Level2DynamicArenaKind::ManaSphere,
                ..
            } if *slot == sphere_slot
        )
    }));

    // Force NewEvent's replaceable-row fallback after the ordinary free
    // stack is exhausted. The dying burst is the only eligible fallback;
    // the replacement must overwrite its source slot with the same
    // post-pass global seed (RNG-L2-0479's secondary arena branch).
    let fallback_slot = session
        .economy_mut()
        .spawn_death_burst_from_death_with_id(902, 111, Position::default())
        .expect("replaceable NewEvent row");
    let fallback_global = session.economy().global_rng;
    session.economy_mut().free_slots.clear();
    let replacement_slot = session
        .economy_mut()
        .spawn_mana_carrier(Position::default(), 111, 0)
        .expect("replaceable NewEvent fallback allocation");
    assert_eq!(replacement_slot, fallback_slot);
    let replacement = session
        .economy()
        .carriers()
        .iter()
        .find(|carrier| carrier.source_entity_index == replacement_slot)
        .expect("replacement carrier row");
    assert_eq!(
        replacement.entity_rng,
        replacement_slot.wrapping_add(fallback_global as u16)
    );

    // The qualified LevelEnd route is the ordinary producer boundary for
    // RNG-L2-0464/0465. Reset before entering it so the global scheduler
    // draw can be compared with the package seed independently of the
    // carrier/sphere rows above. The first pass materializes and dispatches
    // the twelve trees, but the empty owner mailbox keeps the forbidden
    // tree callback branch unreachable in that same pass.
    session.reset();
    session.runtime.set_level2_is_level_end(1);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let tree_before = session
        .economy()
        .authored_trees()
        .iter()
        .find(|tree| tree.authored_index == 260)
        .copied()
        .expect("qualified group-31 tree source");
    assert_eq!(
        session.economy().global_rng,
        level2_next_rng32(initial_global),
        "group-31 activation consumes only the scheduler pass draw"
    );
    assert_eq!(tree_before.owner_entity_index, 0);
    assert_eq!(tree_before.action, 0);
    assert!(!session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            ..
        } if *source_entity_index == tree_before.source_entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::AuthoredGroupActivated {
            pass: 0,
            group: 31,
            authored_indices,
            ..
        } if authored_indices.as_ref() == LEVEL2_GROUP31_AUTHORED_INDICES
    )));

    // The damage mailbox is written between source passes. The following
    // pass reaches the same tree slot's owner callback, consumes exactly
    // one source-local LCG step (0464), and derives the new life from that
    // result (0465); the sphere event is downstream of that same pass.
    let tree_rng_before = tree_before.entity_rng;
    let owner_entity_index = session.state.entity_index;
    {
        let mut economy_view = session.economy_mut();
        let tree = economy_view
            .authored_trees
            .iter_mut()
            .find(|tree| tree.source_entity_index == tree_before.source_entity_index)
            .expect("mutable qualified group-31 tree source");
        tree.owner_entity_index = owner_entity_index;
        tree.pending_damage = tree.life + 1;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let tree_after = session
        .economy()
        .authored_trees()
        .iter()
        .find(|tree| tree.source_entity_index == tree_before.source_entity_index)
        .copied()
        .expect("qualified group-31 tree after owner callback");
    let tree_rng_after = level2_next_rng16(tree_rng_before);
    assert_eq!(
        session.economy().global_rng,
        level2_next_rng32(level2_next_rng32(initial_global)),
        "following tree callback consumes only its scheduler pass draw"
    );
    assert_eq!(tree_after.entity_rng, tree_rng_after);
    assert_eq!(tree_after.life, i32::from(tree_rng_after % 0x3c) + 130);
    assert_eq!(tree_after.action, 1);
    assert_eq!(tree_after.owner_entity_index, 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            pass: 0,
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredTree,
        } if *source_slot == tree_before.source_entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::Economy {
            pass: 0,
            source_slot,
            event: Level2EconomyEvent::ManaSphereSpawned { .. },
        } if *source_slot == tree_before.source_entity_index
    )));

    // The three sound-46 events are downstream of the same source pass.
    // PrepareEventSound computes `9377 * global + 9439` for its play-rate
    // variation (RNG-L2-0493) but does not write D41A0.rand_0x8 back; the
    // scheduler stream therefore remains exactly one draw per pass.
    session.reset();
    configure_level2_scheduler_audio_rows(&mut session);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(level2_scheduler_bleat_slots(&session), [39, 40, 42]);
    assert_eq!(
        session.economy().global_rng,
        level2_next_rng32(initial_global),
        "sound-46's RNG-L2-0493 derived value is not a second global-state advance"
    );
    let prepare_sound_rng = level2_next_rng32(session.economy().global_rng);
    assert_eq!(
        prepare_sound_rng,
        session
            .economy()
            .global_rng
            .wrapping_mul(9_377)
            .wrapping_add(9_439)
    );
    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter_map(|event| match event {
                Mc2DispatchEvent::Goat {
                    source_slot,
                    events,
                    ..
                } if events.iter().any(|event| {
                    matches!(
                        event,
                        GoatStepEvent::Bleat {
                            sound_id: 46,
                            action: 9 | 14,
                            ..
                        }
                    )
                }) =>
                    Some(*source_slot),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![39, 40, 42]
    );

    // Reset restores the package seed, all copied entity RNG words, the
    // arena/free-list/index projection, the event journal, and both
    // observable hashes in one transition.
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline.0);
    assert_eq!(session.dispatch_observability_hash(), baseline.1);
    assert_eq!(session.economy().reset_hash(), baseline.2);
    assert_eq!(session.economy().global_rng, baseline.3);
    assert_eq!(session.economy.entities, baseline.4);
    assert_eq!(session.economy().free_slots, baseline.5);
    assert!(session.dispatch_events().is_empty());
}

#[test]
fn level_two_scheduler_global_rng_rejections_are_exact_no_ops() {
    let (level_one_package, _) = fixture_paths();
    assert!(NativeWorld::inspect_level2(level_one_package).is_err());

    let package = level_two_fixture_path();
    let players = fs::read(package.join("players.bin")).expect("Level 2 players member");
    assert!(
        parse_players(
            &players[..players.len() - 1],
            OwnerPackageProfile::Level2,
            OwnerPackageSchema::V5
        )
        .is_err()
    );

    let inspection = NativeWorld::inspect_level2(&package)
        .expect("qualified Level 2 negative scheduler package");
    assert!(
        inspection
            .entities()
            .iter()
            .all(|entity| { !(entity.active && entity.class == 2 && entity.model == 0) })
    );
    let dolmen = inspection
        .entities()
        .iter()
        .find(|entity| entity.active && entity.class == 2 && entity.model == 2)
        .expect("qualified dolmen row");
    assert_eq!(dolmen.action, 6);
    assert_ne!((dolmen.class, dolmen.model), (2, 0));

    // Exhausting the ordinary free list with no replaceable live row
    // rejects NewEvent before it can seed a source row or publish an
    // event/index/hash mutation.
    let mut exhausted = inspection
        .level2_reset_session()
        .expect("free-list negative session");
    exhausted.economy_mut().free_slots.clear();
    let exhausted_before = (
        exhausted.campaign_state_hash(),
        exhausted.dispatch_observability_hash(),
        exhausted.economy().reset_hash(),
        exhausted.economy().global_rng,
        exhausted.economy().source_rng.clone(),
        exhausted.economy().events.clone(),
    );
    assert!(
        exhausted
            .economy_mut()
            .spawn_mana_carrier(Position::default(), 111, 0)
            .is_none()
    );
    assert_eq!(exhausted.campaign_state_hash(), exhausted_before.0);
    assert_eq!(exhausted.dispatch_observability_hash(), exhausted_before.1);
    assert_eq!(exhausted.economy().reset_hash(), exhausted_before.2);
    assert_eq!(exhausted.economy().global_rng, exhausted_before.3);
    assert_eq!(exhausted.economy().source_rng, exhausted_before.4);
    assert_eq!(exhausted.economy().events, exhausted_before.5);
    assert!(exhausted.dispatch_events().is_empty());

    // A rejected event and a stale cursor are no-ops before the source
    // scheduler can reach any handler draw. The tree callback is likewise
    // absent from this qualified reset; the dolmen row above must not be
    // coerced into AddTree02_00_64E20.
    let mut rejected = inspection
        .level2_reset_session()
        .expect("rejected-event negative session");
    let rejected_before = (
        rejected.campaign_state_hash(),
        rejected.dispatch_observability_hash(),
        rejected.economy().reset_hash(),
        rejected.economy().global_rng,
        rejected.economy().events.clone(),
    );
    assert!(
        rejected
            .economy_mut()
            .allocate_spell_pickup(0, 0, 0, Position::default())
            .is_none()
    );
    assert!(!rejected.economy_mut().step_mana_sphere_slot(u16::MAX));
    assert_eq!(rejected.campaign_state_hash(), rejected_before.0);
    assert_eq!(rejected.dispatch_observability_hash(), rejected_before.1);
    assert_eq!(rejected.economy().reset_hash(), rejected_before.2);
    assert_eq!(rejected.economy().global_rng, rejected_before.3);
    assert_eq!(rejected.economy().events, rejected_before.4);
    assert!(rejected.dispatch_events().is_empty());

    // A real outer pass is not a no-op: its ApplyEvents global draw is
    // expected. Reset, however, must remove that draw and every resulting
    // source/index/event/hash change.
    let mut reset = inspection
        .level2_reset_session()
        .expect("reset negative session");
    let baseline_hash = reset.campaign_state_hash();
    let baseline_economy_hash = reset.economy().reset_hash();
    let baseline_global = reset.economy().global_rng;
    reset.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        reset.economy().global_rng,
        level2_next_rng32(baseline_global)
    );
    assert_ne!(reset.campaign_state_hash(), baseline_hash);
    assert_ne!(reset.economy().reset_hash(), baseline_economy_hash);
    reset.reset();
    assert_eq!(reset.campaign_state_hash(), baseline_hash);
    assert_eq!(reset.economy().reset_hash(), baseline_economy_hash);
    assert_eq!(reset.economy().global_rng, baseline_global);
    assert!(reset.dispatch_events().is_empty());
}

#[test]
fn level_two_same_slot_replacement_defers_until_following_ascending_pass() {
    // Two overlapping live spheres make the first source handler retire
    // one arena occupant; the later persistent pickup then reuses that
    // numeric slot in the same outer turn. The ascending source pointer has
    // already visited that numeric slot, so the new generation must wait for
    // the following outer turn.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 runagain slot-reuse session");
    let baseline_campaign_hash = session.campaign_state_hash();
    let free_slots = session
        .economy()
        .free_slots
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>();
    let prefix_allocations = free_slots
        .iter()
        .enumerate()
        .find_map(|(index, _)| {
            (index + 2 < free_slots.len() && free_slots[index + 2] % 4 == 0).then_some(index)
        })
        .expect("free slot whose later persistent replacement cadence is immediately collectable");
    for _ in 0..prefix_allocations {
        let _ = session
            .economy_mut()
            .spawn_mana_carrier(Position::default(), 111, 0)
            .expect("prefix carrier allocation");
    }
    let target_slot = u16::try_from(
        session
            .economy_mut()
            .spawn_mana_sphere_from_death(901, Position::default(), 0, 0x1111, 100, 111)
            .expect("target sphere allocation"),
    )
    .expect("target sphere slot fits");
    let second_sphere_slot = u16::try_from(
        session
            .economy_mut()
            .spawn_mana_sphere_from_death(902, Position::default(), 0, 0x2222, 50, 111)
            .expect("source sphere allocation"),
    )
    .expect("source sphere slot fits");
    let persistent = session
        .allocate_level2_spell_replacement(11, 38, Position::default())
        .expect("persistent replacement allocation");
    session.state.position = persistent.position;
    assert_eq!(target_slot, free_slots[prefix_allocations]);
    assert_eq!(second_sphere_slot, free_slots[prefix_allocations + 1]);
    assert_eq!(
        persistent.source_entity_index,
        free_slots[prefix_allocations + 2]
    );
    let reused_slot = target_slot;
    assert_eq!(
        session.economy().free_slot_count(),
        free_slots.len() - prefix_allocations - 3
    );
    for slot in [target_slot, second_sphere_slot] {
        let mut economy_view = session.economy_mut();
        let sphere = economy_view
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == slot)
            .expect("test sphere remains live before source dispatch");
        sphere.position = Position::default();
        sphere.velocity_x = 0;
        sphere.velocity_y = 0;
        sphere.vertical_velocity = 0;
    }
    assert_eq!(
        session
            .economy()
            .spell_replacements()
            .iter()
            .find(|replacement| replacement.source_entity_index == persistent.source_entity_index)
            .expect("initial persistent source")
            .action,
        35
    );
    assert_eq!(
        session
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == reused_slot)
            .expect("initial cursor row")
            .generation,
        1
    );

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::ManaSphere,
            ..
        } if *source_slot == target_slot
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::SpellAcquisition {
            family: 11,
            source_slot,
            result: Some(_),
            ..
        } if *source_slot == persistent.source_entity_index
    )));
    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter(|event| matches!(
                event,
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::SpellReplacement,
                    ..
                } if *source_slot == reused_slot
            ))
            .count(),
        0,
        "the ascending source pointer cannot revisit a same-slot replacement"
    );
    let deferred_phase = {
        let deferred_replacement = session
            .economy()
            .spell_replacements()
            .iter()
            .find(|replacement| replacement.source_entity_index == reused_slot)
            .expect("replacement reused reclaimed slot");
        assert_eq!(deferred_replacement.family, 11);
        assert_eq!(deferred_replacement.action, 35);
        assert_eq!(deferred_replacement.phase, reused_slot as u8);
        deferred_replacement.phase
    };
    assert_eq!(
        session
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == reused_slot)
            .expect("replacement cursor row")
            .generation,
        2
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter(|event| matches!(
                event,
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::SpellReplacement,
                    ..
                } if *source_slot == reused_slot
            ))
            .count(),
        1,
        "the following ascending pass visits the replacement exactly once"
    );
    assert_eq!(
        session
            .economy()
            .spell_replacements()
            .iter()
            .find(|replacement| replacement.source_entity_index == reused_slot)
            .expect("replacement remains live after its following-pass visit")
            .phase,
        deferred_phase.wrapping_add(1)
    );
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
}

#[test]
fn production_speed_sweeps_preserve_the_complete_quiet_reset_rng_schedule() {
    use crate::level1_mission::GlobalRngSite;

    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    for (speed_index, expected_passes) in [(0_u8, 1_u16), (1, 4), (2, 8)] {
        let mut session = world.session();
        session.mission.clear_global_rng_trace();
        let evidence = session.step_with_speed_index(PlayerCommand::default(), speed_index);
        let trace = session.mission.global_rng_trace();
        assert_eq!(evidence.len(), usize::from(expected_passes));
        assert_eq!(trace.len(), usize::from(expected_passes) * 10);
        for pass_index in 0..expected_passes {
            let pass = &trace[usize::from(pass_index) * 10..usize::from(pass_index + 1) * 10];
            assert_eq!(
                pass.iter()
                    .map(|entry| entry.pass_index)
                    .collect::<Vec<_>>(),
                vec![pass_index; 10],
                "no draw may escape or enter scheduled pass {pass_index}"
            );
            assert_eq!(
                pass.iter()
                    .map(|entry| entry.source_slot)
                    .collect::<Vec<_>>(),
                [0_u16, 113, 114, 115, 116, 117, 118, 119, 120, 121]
            );
            assert_eq!(pass[0].site, GlobalRngSite::UpdateEntities);
            assert!(
                pass[1..]
                    .iter()
                    .all(|entry| entry.site == GlobalRngSite::Model60Smoke)
            );
        }
        assert_eq!(trace[0].before, 0xad6e_67d9);
        assert_eq!(
            trace[..10]
                .iter()
                .map(|entry| entry.after)
                .collect::<Vec<_>>(),
            [
                0x9909_f858,
                0xa631_b437,
                0x829a_3776,
                0xd2c9_9e15,
                0xeb07_8414,
                0xde4d_0573,
                0xa334_be32,
                0x0eea_ca51,
                0x661c_c5d0,
                0x43e9_ccaf,
            ]
        );
        for (index, entry) in trace.iter().enumerate() {
            assert_eq!(
                entry.after,
                entry.before.wrapping_mul(9_377).wrapping_add(9_439)
            );
            if let Some(next) = trace.get(index + 1) {
                assert_eq!(next.before, entry.after);
            }
        }
        assert_eq!(
            (
                session.mission.objective_presentation_palette_mod,
                session.mission.objective_presentation_phase,
                session.mission.objective_textbox_ticks,
            ),
            (2, 1, 0),
            "speed index {speed_index} advances presentation once, not once per entity pass"
        );
        session.step_with_speed_index(PlayerCommand::default(), speed_index);
        assert_eq!(
            (
                session.mission.objective_presentation_palette_mod,
                session.mission.objective_presentation_phase,
                session.mission.objective_textbox_ticks,
            ),
            (3, 0, 200),
            "speed index {speed_index} services initial objective once on the second outer frame"
        );
    }
}
