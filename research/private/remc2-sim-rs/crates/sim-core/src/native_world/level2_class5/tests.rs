use super::super::tests::level_two_fixture_path;
use super::super::*;

#[test]
fn level_two_model45_group22_materializes_authored_class5_rows_in_source_order() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 group-22 authored class-5 session");
    let baseline_hash = session.campaign_state_hash();
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");

    session.step_outer_turn(PlayerCommand::default(), 0);

    let sources = session.economy().authored_class5().to_vec();
    assert_eq!(
        sources
            .iter()
            .map(|source| source.authored_index)
            .collect::<Vec<_>>(),
        vec![238, 239, 240, 241, 242, 243, 245]
    );
    assert_eq!(
        sources
            .iter()
            .map(|source| source.model)
            .collect::<Vec<_>>(),
        vec![3, 9, 9, 9, 9, 9, 9]
    );
    assert_eq!(
        sources
            .iter()
            .map(|source| source.action)
            .collect::<Vec<_>>(),
        vec![217, 72, 72, 72, 72, 72, 72]
    );
    assert!(sources.iter().all(|source| source.active));
    assert!(sources.iter().all(|source| source.drawing_enabled));
    assert_eq!(sources[0].max_life, 9_000);
    assert_eq!(sources[0].mana, 2_250);
    assert_eq!(sources[0].max_mana, 4_500);
    assert_eq!(sources[1].max_life, 1_000);
    assert_eq!(sources[1].mana, 500);
    assert_eq!(sources[1].max_mana, 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::AuthoredGroupActivated {
            group: 22,
            authored_indices,
            ..
        } if authored_indices.as_ref() == [238, 239, 240, 241, 242, 243, 245]
    )));

    let row245_before = sources
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 materialized");
    // The same outer turn that allocates group 22 also reaches the dynamic
    // arena once, so the captured production row has consumed callback one.
    assert_eq!(row245_before.dword_0x10, 15);

    // `sub_20370` owns action 72 and only decrements the countdown. The
    // zero test uses the pre-decrement value, so the remaining fifteen
    // callbacks stay stationary and the following callback changes to action
    // 73 without moving.
    for _ in 0..15 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let row245_after_countdown = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 remains live after countdown callbacks");
    assert_eq!(row245_after_countdown.action, 72);
    assert_eq!(row245_after_countdown.dword_0x10, 0);
    assert_eq!(row245_after_countdown.position, row245_before.position);
    assert_eq!(
        row245_after_countdown.phase,
        row245_before.phase.wrapping_add(15)
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let row245_after_transition = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 remains live after action-73 transition");
    assert_eq!(row245_after_transition.action, 73);
    assert_eq!(row245_after_transition.dword_0x10, 400);
    assert_eq!(row245_after_transition.particle_index, 201);
    assert_eq!(row245_after_transition.position, row245_before.position);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredClass5,
            ..
        } if *source_slot == row245_after_transition.source_entity_index
    )));

    // Action 73 owns the movement/collision body. The qualified witness
    // crosses a tile boundary after seven action-73 callbacks, distinguishing
    // its accepted candidate from both the stationary countdown and a
    // phase-only placeholder.
    for _ in 0..7 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let mut expected_row245_after_seven = row245_before.position;
    for _ in 0..7 {
        crate::move_entity(
            &mut expected_row245_after_seven,
            row245_before.yaw,
            0,
            row245_before.actual_speed,
        );
        expected_row245_after_seven.z = session.terrain.ground_height(expected_row245_after_seven);
    }
    let row245_after_seven_steps = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 245)
        .copied()
        .expect("model-9 row 245 remains live after free callbacks");
    assert_eq!(
        row245_after_seven_steps.position,
        expected_row245_after_seven
    );
    assert_eq!(
        row245_after_seven_steps.phase,
        row245_before.phase.wrapping_add(23)
    );
    assert_ne!(
        row245_after_seven_steps.position.y >> 8,
        row245_before.position.y >> 8,
        "free branch must commit a cross-tile model-9 movement"
    );

    // Row 238 is the model-3/action-25 root.  This bounded fragment of
    // its `sub_29710` callback is not a phase-only no-op:
    // `sub_2AF10(a1, 0)` re-snaps the root to terrain even when movement
    // is disabled.  Tamper only the live source height, then exercise the
    // ordinary outer cursor and require the source-backed correction,
    // target retention, sprite selector, and typed dispatch event.
    let row238_before = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after model-9 callbacks");
    let row238_ground = session.terrain.ground_height(row238_before.position);
    let fragment_z = row238_ground.wrapping_add(17);
    let tracked_target = 111_u16;
    let tracked_target_position = Position {
        x: row238_before.position.x.wrapping_add(1_024),
        y: row238_before.position.y,
        z: row238_ground,
    };
    session
        .set_level2_entity_position(tracked_target, tracked_target_position)
        .expect("live imported target row accepts a position update");
    let tracked_target_heading =
        heading_for_positions(row238_before.position, tracked_target_position);
    let row238_mut = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for terrain fragment");
    row238_mut.action = 25;
    row238_mut.word_0x96_150 = tracked_target;
    row238_mut.byte_0x46_70 = 0;
    row238_mut.source_byte_3e = 0;
    row238_mut.yaw = i16::try_from(tracked_target_heading).expect("target heading fits i16");
    row238_mut.position.z = fragment_z;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after terrain-resnap fragment");
    assert_eq!(row238_after.phase, row238_before.phase.wrapping_add(1));
    assert_eq!(row238_after.action, 25);
    assert_eq!(
        (row238_after.position.x, row238_after.position.y),
        (row238_before.position.x, row238_before.position.y)
    );
    assert_eq!(row238_after.position.z, row238_ground);
    assert_eq!(row238_after.word_0x96_150, tracked_target);
    assert_eq!(row238_after.byte_0x46_70, 1);
    assert_eq!(row238_after.particle_index, 337);
    assert_eq!(row238_after.roll, tracked_target_heading as i16);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredClass5,
            ..
        } if *source_slot == row238_after.source_entity_index
    )));

    // Moving only the live target beyond the descriptor range must take
    // the source target-loss branch and clear both relation and sprite
    // selector rather than retaining stale engagement state.
    let out_of_range_target_position = Position {
        x: row238_after.position.x.wrapping_add(6_000),
        y: row238_after.position.y,
        z: row238_ground,
    };
    session
        .set_level2_entity_position(tracked_target, out_of_range_target_position)
        .expect("live imported target row accepts the out-of-range position");
    let row238_before_out_of_range = row238_after;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after_non_cadence = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live before target-loss cadence");
    assert_eq!(
        row238_after_non_cadence.phase,
        row238_before_out_of_range.phase.wrapping_add(1)
    );
    assert_eq!(row238_after_non_cadence.action, 25);
    assert_eq!(row238_after_non_cadence.word_0x96_150, tracked_target);
    assert_eq!(row238_after_non_cadence.byte_0x46_70, 1);
    assert_eq!(row238_after_non_cadence.particle_index, 337);
    assert_eq!(row238_after_non_cadence.source_byte_3e, 2);
    // The source callback keeps the stale engagement through the
    // non-cadence visits. Advance the ordinary cursor to the next
    // 0x1f-gated callback without changing the live target again.
    for _ in 0..30 {
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    let row238_before_cadence = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live at target-loss cadence");
    assert_eq!(row238_before_cadence.action, 25);
    assert_eq!(row238_before_cadence.source_byte_3e, 32);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after_out_of_range = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after target loss");
    assert_eq!(
        row238_after_out_of_range.phase,
        row238_before_cadence.phase.wrapping_add(1)
    );
    assert_eq!(
        row238_after_out_of_range.source_byte_3e,
        row238_before_cadence.source_byte_3e.wrapping_add(1)
    );
    assert_eq!(
        row238_after_out_of_range.action, 217,
        "target loss is committed on the source 0x1f cadence"
    );
    assert_eq!(row238_after_out_of_range.word_0x96_150, 0);
    assert_eq!(row238_after_out_of_range.byte_0x46_70, 0);
    assert_eq!(row238_after_out_of_range.particle_index, 315);

    // The nonzero byte-0x3B branch must promote the pending mailbox word
    // into the target relation before the wrapper runs.  This prevents a
    // cleared preexisting relation from taking the action-217 fallback in
    // place of the source's tracked-target path.
    let handoff_target = tracked_target;
    session
        .set_level2_entity_position(tracked_target, tracked_target_position)
        .expect("tracked target returns to the in-range position");
    let row238_before_handoff = row238_after_out_of_range;
    let row238_mut = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for target handoff");
    row238_mut.pending_word_0x62 = handoff_target;
    row238_mut.byte_0x3b = 1;
    row238_mut.action = 25;
    row238_mut.byte_0x46_70 = 0;
    row238_mut.word_0x96_150 = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after_handoff = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after target handoff");
    assert_eq!(
        row238_after_handoff.phase,
        row238_before_handoff.phase.wrapping_add(1)
    );
    assert_eq!(row238_after_handoff.action, 25);
    assert_eq!(row238_after_handoff.pending_word_0x62, 0);
    assert_eq!(row238_after_handoff.word_0x26, handoff_target);
    assert_eq!(row238_after_handoff.word_0x96_150, handoff_target);
    assert_eq!(row238_after_handoff.life, 1_000_000);

    // The preceding production activation had no target relation, so the
    // source wrapper took its action-217 target-loss fallback.  The
    // fragment proof above restores a nonzero relation only to isolate
    // the terrain branch without importing the child graph.
    // Isolate the preceding `sub_2A6B0` mailbox transfer on the same
    // production row.  With byte 0x3B clear, the source selects action
    // 220, copies the pending word into both target/fallback links, and
    // clears the mailbox before returning to the cursor.
    let pending_target = 0x3456_u16;
    let row238_before_fallback = row238_after_handoff;
    let fallback_z = row238_ground.wrapping_add(19);
    session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for pre-pass")
        .pending_word_0x62 = pending_target;
    let row238_mut = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 pre-pass source");
    row238_mut.byte_0x3b = 0;
    row238_mut.position.z = fallback_z;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after_fallback = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after pre-pass");
    assert_eq!(
        row238_after_fallback.phase,
        row238_before_fallback.phase.wrapping_add(1)
    );
    assert_eq!(row238_after_fallback.action, 220);
    assert_eq!(row238_after_fallback.pending_word_0x62, 0);
    assert_eq!(row238_after_fallback.word_0x26, pending_target);
    assert_eq!(row238_after_fallback.word_0x24, pending_target);
    assert_eq!(
        row238_after_fallback.position.z, fallback_z,
        "action-220 fallback must skip the later terrain-resnap fragment"
    );

    session.reset();
    assert!(session.economy().authored_class5().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_model3_actions_25_216_220_are_source_complete() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-3 action session");
    let baseline_hash = session.campaign_state_hash();
    let authored_features = session.mission().authored_features().to_vec();
    let feature_position = |index: u16| {
        authored_features
            .iter()
            .copied()
            .find(|feature| feature.index == index)
            .expect("authored model-3 feature")
            .world_position(0)
    };

    // Group 22 is the direct authored production witness for row 238.  The
    // constructor must expose one root sidecar and the complete sixteen-row
    // linked chain through the ordinary source cursor.
    session
        .set_level2_selected_objective_stage_state(2)
        .expect("source objective completion sentinel");
    session.step_outer_turn(PlayerCommand::default(), 0);
    let root_slot = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .map(|source| source.source_entity_index)
        .expect("group-22 model-3 root");
    let root_state = session
        .economy()
        .authored_class5_model3()
        .iter()
        .find(|state| state.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root sidecar");
    assert_eq!(root_state.descriptor_index, 74);
    assert_eq!(
        session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .filter(|child| child.parent_entity_index == root_slot)
            .count(),
        16
    );
    let chain = session
        .economy()
        .authored_class5_model3_children()
        .iter()
        .filter(|child| child.parent_entity_index == root_slot)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(chain[0].word_0x32, 0);
    for pair in chain.windows(2) {
        assert_eq!(pair[0].word_0x34, pair[1].source_entity_index);
        assert_eq!(pair[1].word_0x32, pair[0].source_entity_index);
    }
    assert_eq!(chain.last().expect("model-3 chain tail").word_0x34, 0);
    assert!(
        session
            .economy()
            .live_arena_slots()
            .iter()
            .filter(|(_, kind)| *kind == Level2DynamicArenaKind::AuthoredClass5Model3Child)
            .count()
            >= 16
    );

    // Action 25's target-loss branch is source-visible before the action-217
    // idle path.  An inactive imported target must clear both relations.
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root remains live");
        root.action = 25;
        root.source_byte_3e = 0;
        root.word_0x96_150 = 999;
        root.byte_0x46_70 = 0;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_action25 = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 action-25 root");
    assert_eq!(after_action25.action, 217);
    assert_eq!(after_action25.word_0x96_150, 0);
    assert_eq!(after_action25.particle_index, 315);

    // Action 216 consumes the two ordered RNG draws at counter 9 and emits
    // the source audio sink. Action 219 then enters action 217 in the same
    // source cursor pass.
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for action 216");
        root.action = 216;
        root.dword_0x10 = 9;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            event: Level2EconomyEvent::AuthoredClass5Model3Sound {
                source_entity_index,
                sound_id: 22,
            },
            ..
        } if *source_entity_index == root_slot
    )));
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for action 219");
        root.action = 219;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 action-219 root")
            .action,
        217
    );

    // A child mailbox is transferred through the root's source state, caps
    // damage at 76, and produces the ordinary economy damage event.
    let child_slot = chain[0].source_entity_index;
    let child_life_before = chain[0].life;
    {
        let child = session
            .economy_mut()
            .authored_class5_model3_children
            .iter_mut()
            .find(|child| child.source_entity_index == child_slot)
            .expect("model-3 child mailbox");
        child.pending_word_0x62 = 111;
        child.damage_dword_0x5e_94 = 80;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let child_after_damage = session
        .economy()
        .authored_class5_model3_children()
        .iter()
        .find(|child| child.source_entity_index == child_slot)
        .copied()
        .expect("model-3 child after mailbox");
    assert_eq!(child_after_damage.life, child_life_before - 76);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            event: Level2EconomyEvent::DamageApplied {
                target_entity_index,
                damage: 76,
                ..
            },
            ..
        } if *target_entity_index == child_slot
    )));

    // Action 218 rejects the same inactive target relation, while action 220
    // marks the root and every linked child for the source pre-kill state.
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for action 218");
        root.action = 218;
        root.source_byte_3e = 0;
        root.word_0x96_150 = 999;
        root.pending_word_0x62 = 0;
        root.byte_0x3b = 1;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 action-218 root")
            .action,
        217
    );
    session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == root_slot)
        .expect("model-3 root for action 220")
        .action = 220;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let killed_root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 action-220 root");
    assert_eq!((killed_root.action, killed_root.life), (221, -1));
    assert!(!killed_root.drawing_enabled);
    assert!(
        session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .filter(|child| child.parent_entity_index == root_slot)
            .all(|child| !child.drawing_enabled)
    );

    // The other authored constructor rows are reached by ordinary group-1
    // production, while rows 164/165 are checked through their group-keyed
    // constructor inputs. All six rows must share the same sixteen-child
    // model-3 carrier and no alternate Level-N API.
    session.reset();
    let local_player = session.state.entity_index;
    session
        .set_level2_entity_position(local_player, feature_position(11))
        .expect("row-11 production position");
    session.step_outer_turn(PlayerCommand::default(), 0);
    for authored_index in [167_u16, 168, 169] {
        let source = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.authored_index == authored_index)
            .copied()
            .expect("group-1 model-3 source");
        assert!(
            session
                .economy()
                .authored_class5_model3()
                .iter()
                .any(|state| state.source_entity_index == source.source_entity_index)
        );
        assert_eq!(
            session
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| child.parent_entity_index == source.source_entity_index)
                .count(),
            16
        );
    }
    for authored_index in [164_u16, 165] {
        let source = session
            .economy_mut()
            .allocate_authored_class5(authored_index, 3, feature_position(authored_index))
            .expect("group-keyed model-3 constructor row");
        assert_eq!(source.authored_index, authored_index);
        assert_eq!(
            session
                .economy()
                .authored_class5_model3_children()
                .iter()
                .filter(|child| child.parent_entity_index == source.source_entity_index)
                .count(),
            16
        );
    }

    // A full arena rejects the root-plus-sixteen constructor transactionally.
    session.reset();
    session.economy_mut().free_slots.clear();
    let economy_before_failure = session.economy().clone();
    assert!(
        session
            .economy_mut()
            .allocate_authored_class5(238, 3, feature_position(238))
            .is_none()
    );
    assert_eq!(session.economy(), &economy_before_failure);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}
