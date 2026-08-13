use super::super::tests::level_two_fixture_path;
use super::super::*;
use super::archer::{
    ARCHER_ACTION_ATTACK, ARCHER_DESCRIPTOR_CADENCE, ARCHER_MODEL, ArcherChildRequest,
};
use std::collections::BTreeSet;

fn activate_group20_archers() -> (Level2ResetSession, u64) {
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

    session.state.position = feature_position(11);
    session.runtime.objectives[0].stage_selectors[1] = 2;
    session.runtime.objectives[0].stage_selectors[2] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.runtime.objectives[0].stage_selectors[3] = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    session.state.position = feature_position(214);

    let expected = [
        216_u16, 217, 218, 219, 220, 221, 222, 223, 232, 233, 234, 235,
    ];
    let activated = (0..8).any(|_| {
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::AuthoredGroupActivated {
                    group: 20,
                    authored_indices,
                    ..
                } if authored_indices.as_ref() == expected
            )
        })
    });
    assert!(activated, "row 214 must activate group 20 in production");

    let authored_rows = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.active && source.model == ARCHER_MODEL)
        .map(|source| source.authored_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(authored_rows, BTreeSet::from([232, 233, 234, 235]));
    (session, baseline_hash)
}

fn produce_archer_child(session: &mut Level2ResetSession) -> (u16, u16, i16) {
    let root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("group-20 row 232 Archer root");
    let player_entity = session.state.entity_index;
    session.state.position = root.position;
    session
        .set_level2_entity_position(player_entity, root.position)
        .expect("local player accepts Archer target position");
    let parent_roll = root.roll;
    let source = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == root.source_entity_index)
        .expect("Archer root remains mutable");
    source.action = ARCHER_ACTION_ATTACK;
    source.source_byte_3e = ARCHER_DESCRIPTOR_CADENCE;
    source.word_0x96_150 = player_entity;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let child = session
        .economy()
        .authored_class5_archer_children()
        .iter()
        .find(|child| child.parent_entity_index == root.source_entity_index)
        .copied()
        .expect("ordinary Archer cursor retains a positive child");
    (
        root.source_entity_index,
        child.source_entity_index,
        parent_roll,
    )
}

#[test]
fn level_two_archers_complete_constructor_action_and_child_chain() {
    let (mut session, baseline_hash) = activate_group20_archers();
    let (root_slot, child_slot, parent_roll) = produce_archer_child(&mut session);

    let roots = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.active && source.model == ARCHER_MODEL)
        .map(|source| (source.authored_index, source.source_entity_index))
        .collect::<Vec<_>>();
    assert_eq!(
        roots.iter().map(|(row, _)| *row).collect::<BTreeSet<_>>(),
        BTreeSet::from([232, 233, 234, 235])
    );
    assert_eq!(
        roots
            .iter()
            .map(|(_, slot)| *slot)
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    let root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("row-232 root remains live");
    let archer_state = session
        .economy()
        .authored_class5_archer()
        .iter()
        .find(|state| state.source_entity_index == root_slot)
        .copied()
        .expect("row-232 typed Archer state");
    let child = session
        .economy()
        .authored_class5_archer_children()
        .iter()
        .find(|child| child.source_entity_index == child_slot)
        .copied()
        .expect("retained Archer child");
    assert_eq!(archer_state.word_0x34, child_slot);
    assert_eq!(child.parent_entity_index, root_slot);
    assert_eq!(child.word_0x96, root.word_0x96_150);
    assert_eq!(child.roll, parent_roll);
    assert!(child.slot_generation > 0);
    assert!(
        child.phase > 0,
        "child was visited by the live source cursor"
    );
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .iter()
            .any(|slot| *slot == root_slot)
    );
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .iter()
            .any(|slot| *slot == child_slot)
    );
    assert!(session.economy().live_arena_slots().contains(&(
        child_slot,
        Level2DynamicArenaKind::AuthoredClass5ArcherChild
    )));
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Level2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredClass5ArcherChild,
                ..
            } if *source_slot == child_slot
        )
    }));

    let first_archers = session
        .economy()
        .authored_class5()
        .iter()
        .filter(|source| source.model == ARCHER_MODEL)
        .copied()
        .collect::<Vec<_>>();
    let first_states = session.economy().authored_class5_archer().to_vec();
    let first_children = session.economy().authored_class5_archer_children().to_vec();
    let first_hash = session.campaign_state_hash();

    session.reset();
    assert!(session.economy().authored_class5_archer().is_empty());
    assert!(
        session
            .economy()
            .authored_class5_archer_children()
            .is_empty()
    );
    assert!(
        session
            .economy()
            .authored_class5()
            .iter()
            .all(|source| !matches!(source.authored_index, 232 | 233 | 234 | 235))
    );
    assert_eq!(session.campaign_state_hash(), baseline_hash);

    let (mut replay, replay_baseline_hash) = activate_group20_archers();
    assert_eq!(replay_baseline_hash, baseline_hash);
    produce_archer_child(&mut replay);
    assert_eq!(
        replay
            .economy()
            .authored_class5()
            .iter()
            .filter(|source| source.model == ARCHER_MODEL)
            .copied()
            .collect::<Vec<_>>(),
        first_archers
    );
    assert_eq!(replay.economy().authored_class5_archer(), first_states);
    assert_eq!(
        replay.economy().authored_class5_archer_children(),
        first_children
    );
    assert_eq!(replay.campaign_state_hash(), first_hash);
}

#[test]
fn archer_root_and_child_allocation_failures_are_atomic() {
    let (mut session, _) = activate_group20_archers();
    session.economy_mut().free_slots.clear();
    let before_no_slot = session.economy().clone();
    let hash_before_no_slot = session.campaign_state_hash();
    assert!(
        session
            .economy_mut()
            .allocate_authored_archer(235, Position { x: 0, y: 0, z: 0 })
            .is_none()
    );
    assert_eq!(session.economy(), &before_no_slot);
    assert_eq!(session.campaign_state_hash(), hash_before_no_slot);

    session.economy_mut().free_slots = vec![u16::MAX];
    let before_map_failure = session.economy().clone();
    let hash_before_map_failure = session.campaign_state_hash();
    assert!(
        session
            .economy_mut()
            .allocate_authored_archer(235, Position { x: 0, y: 0, z: 0 })
            .is_none()
    );
    assert_eq!(session.economy(), &before_map_failure);
    assert_eq!(session.campaign_state_hash(), hash_before_map_failure);

    let root = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 232)
        .copied()
        .expect("row-232 root for child rollback");
    let request = ArcherChildRequest {
        parent_entity_index: root.source_entity_index,
        parent_entity_id: root.source_entity_index,
        parent_word_0x96: root.word_0x96_150,
        parent_roll: root.roll,
        parent_position: root.position,
        parent_fov: root.fov,
        target_entity_index: session.state.entity_index,
        target_class: 3,
        target_model: 0,
        target_position: root.position,
    };
    session.economy_mut().free_slots = vec![u16::MAX];
    let before_child_map_failure = session.economy().clone();
    let hash_before_child_map_failure = session.campaign_state_hash();
    assert!(
        session
            .economy_mut()
            .allocate_archer_child_packet(request)
            .is_none()
    );
    assert_eq!(session.economy(), &before_child_map_failure);
    assert_eq!(session.campaign_state_hash(), hash_before_child_map_failure);
}

#[test]
fn archer_hit_and_kill_propagate_through_the_retained_child_chain() {
    let (mut session, _) = activate_group20_archers();
    let (root_slot, child_slot, _) = produce_archer_child(&mut session);
    let root = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == root_slot)
        .expect("Archer root for hit transition");
    root.action = super::archer::ARCHER_ACTION_HIT;
    root.dword_0x10 = 0;

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("root after pre-kill")
            .action,
        super::archer::ARCHER_ACTION_KILL
    );
    assert_eq!(
        session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .expect("child after pre-kill")
            .action,
        super::archer::ARCHER_ACTION_KILL
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .expect("root after terminal kill")
            .drawing_enabled
    );
    assert!(
        !session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .expect("child after terminal kill")
            .drawing_enabled
    );

    session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.source_entity_index == root_slot)
        .expect("root for reclaim")
        .active = false;
    session.economy_mut().reclaim_inactive_slots();
    assert!(
        session
            .economy()
            .authored_class5_archer_children()
            .iter()
            .all(|child| child.source_entity_index != child_slot)
    );
    assert!(
        session
            .economy()
            .live_arena_slots()
            .iter()
            .all(|(slot, _)| *slot != child_slot && *slot != root_slot)
    );
}

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
        vec![25, 72, 72, 72, 72, 72, 72]
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

    // Row 238 is the model-3/action-25 root.  Exercise the real production
    // target projection at the source cadence rather than the obsolete
    // action-25 target-loss wrapper.
    let row238_before = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after model-9 callbacks");
    let row238_ground = session.terrain.ground_height(row238_before.position);
    let player_entity = session.state.entity_index;
    for entity in &mut session.entities {
        if entity.class == 3 {
            entity.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }
    session
        .set_level2_entity_position(player_entity, row238_before.position)
        .expect("production player target position");
    session
        .entities
        .get_mut(usize::from(player_entity))
        .expect("production player target")
        .control_flags &= !ENTITY_DRAW_DISABLED_BIT;
    let row238_mut = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for terrain fragment");
    row238_mut.action = 25;
    row238_mut.source_byte_3e = 0;
    row238_mut.source_byte_39 = 1;
    row238_mut.actual_speed = 0;
    row238_mut.yaw = 0;
    row238_mut.word_0x96_150 = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after terrain-resnap fragment");
    assert_eq!(row238_after.phase, row238_before.phase.wrapping_add(1));
    assert_eq!(row238_after.action, 30);
    assert_eq!(
        (row238_after.position.x, row238_after.position.y),
        (row238_before.position.x, row238_before.position.y)
    );
    assert_eq!(row238_after.position.z, row238_ground);
    assert_eq!(row238_after.word_0x96_150, player_entity);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredClass5,
            ..
        } if *source_slot == row238_after.source_entity_index
    )));

    // A qualified class-3 target that moves out of range is rejected by the
    // source acquisition predicate, while the cadence still consumes the
    // two LCG draws.  The old wrapper incorrectly waited for action 217 and
    // cleared a stale relation here.
    for entity in &mut session.entities {
        if entity.class == 3 {
            entity.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }
    session
        .set_level2_entity_position(
            player_entity,
            Position {
                x: row238_after.position.x.wrapping_add(0x2000),
                y: row238_after.position.y,
                z: row238_ground,
            },
        )
        .expect("production out-of-range target position");
    let row238_mut = session
        .economy_mut()
        .authored_class5
        .iter_mut()
        .find(|source| source.authored_index == 238)
        .expect("model-3 row 238 remains mutable for source rejection");
    row238_mut.action = 25;
    row238_mut.source_byte_3e = 0;
    row238_mut.source_byte_39 = 1;
    row238_mut.actual_speed = 0;
    row238_mut.yaw = 0;
    row238_mut.entity_rng = 0x1357;
    row238_mut.roll = 0;
    row238_mut.word_0x96_150 = 0;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row238_after_rejection = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.authored_index == 238)
        .copied()
        .expect("model-3 row 238 remains live after source rejection");
    assert_ne!(row238_after_rejection.action, 30);
    assert_eq!(row238_after_rejection.word_0x96_150, 0);
    assert_eq!(
        row238_after_rejection.entity_rng,
        level2_next_rng16(level2_next_rng16(0x1357))
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

    let expected_roll = |initial_rng: u16, initial_roll: i16| {
        let first = level2_next_rng16(initial_rng);
        let second = level2_next_rng16(first);
        let direction = 2 * (i32::from(first % 0x009d) / 79) - 1;
        let delta = (i32::from(second & 0x00ff) + 85) * direction;
        (i32::from(initial_roll).wrapping_add(delta) & 0x07ff) as i16
    };

    let player_slot = session.state.entity_index;
    let root_position = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .expect("model-3 root remains live")
        .position;

    // The real class-3 projection is reduced to one qualified target.  At
    // the descriptor cadence, source action 25 consumes two draws, writes
    // word 0x96, and enters action 30 because the row-74 flags include 8.
    for entity in &mut session.entities {
        if entity.class == 3 {
            entity.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }
    session
        .set_level2_entity_position(player_slot, root_position)
        .expect("qualified player target position");
    session
        .entities
        .get_mut(usize::from(player_slot))
        .expect("qualified player target")
        .control_flags &= !ENTITY_DRAW_DISABLED_BIT;
    let initial_rng = 0x2468;
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for qualified target");
        root.action = 25;
        root.source_byte_3e = 0;
        root.source_byte_39 = 1;
        root.actual_speed = 0;
        root.yaw = 0;
        root.entity_rng = initial_rng;
        root.roll = 0;
        root.life = root.max_life;
        root.pending_word_0x62 = 0;
        root.word_0x96_150 = 0;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_qualified_target = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after qualified target");
    assert_eq!(after_qualified_target.action, 30);
    assert_eq!(after_qualified_target.word_0x96_150, player_slot);
    assert_eq!(
        after_qualified_target.entity_rng,
        level2_next_rng16(level2_next_rng16(initial_rng))
    );
    assert_eq!(after_qualified_target.roll, expected_roll(initial_rng, 0));
    let action30_phase = after_qualified_target.phase;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_action30 = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after action-30 handoff");
    assert_eq!(after_action30.action, 30);
    assert_eq!(after_action30.phase, action30_phase.wrapping_add(1));

    // A mailbox hit followed by a lower-life linked child transfers the
    // child's attacker relation to the root and takes the same action-30
    // handoff without consuming the cadence RNG.
    let child_slot = chain[0].source_entity_index;
    let child_attacker = 0x4242;
    let child_life = 8_900;
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for child transfer");
        root.action = 25;
        root.source_byte_3e = 0;
        root.entity_rng = initial_rng;
        root.life = root.max_life;
        root.pending_word_0x62 = 0x1111;
        root.word_0x96_150 = 0;
    }
    session
        .economy_mut()
        .authored_class5_model3
        .iter_mut()
        .find(|state| state.source_entity_index == root_slot)
        .expect("model-3 sidecar for child transfer")
        .damage_dword_0x5e_94 = 7;
    session
        .economy_mut()
        .authored_class5_model3_children
        .iter_mut()
        .find(|child| child.source_entity_index == child_slot)
        .expect("model-3 child for lower-life transfer")
        .life = child_life;
    session
        .economy_mut()
        .authored_class5_model3_children
        .iter_mut()
        .find(|child| child.source_entity_index == child_slot)
        .expect("model-3 child attacker relation")
        .word_0x26 = child_attacker;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_child_transfer = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after child transfer");
    assert_eq!(after_child_transfer.life, child_life);
    assert_eq!(after_child_transfer.word_0x26, child_attacker);
    assert_eq!(after_child_transfer.word_0x96_150, child_attacker);
    assert_eq!(after_child_transfer.action, 30);
    assert_eq!(after_child_transfer.entity_rng, initial_rng);
    assert_eq!(
        session
            .economy()
            .authored_class5_model3()
            .iter()
            .find(|state| state.source_entity_index == root_slot)
            .expect("model-3 sidecar after child transfer")
            .damage_dword_0x5e_94,
        0
    );

    // A negative root life records word 0x24 and selects the source
    // pre-kill handoff without movement or RNG draws.
    let death_attacker = 0x5151;
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for negative-life handoff");
        root.action = 25;
        root.source_byte_3e = 0;
        root.entity_rng = initial_rng;
        root.roll = 0;
        root.life = 1;
        root.pending_word_0x62 = death_attacker;
    }
    session
        .economy_mut()
        .authored_class5_model3
        .iter_mut()
        .find(|state| state.source_entity_index == root_slot)
        .expect("model-3 sidecar for negative-life handoff")
        .damage_dword_0x5e_94 = 2;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_negative_life = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after negative-life handoff");
    assert_eq!(after_negative_life.life, -1);
    assert_eq!(after_negative_life.word_0x24, death_attacker);
    assert_eq!(after_negative_life.action, 28);
    assert_eq!(after_negative_life.entity_rng, initial_rng);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_action28 = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after action-28 handoff");
    assert_eq!(after_action28.action, 29);
    assert!(
        session
            .economy()
            .authored_class5_model3_children()
            .iter()
            .filter(|child| child.parent_entity_index == root_slot)
            .all(|child| child.action == 29)
    );

    // With the class-3 projection rejected, the same-model source chain
    // selects the nearest eligible model-3 peer and enters action 27.
    for entity in &mut session.entities {
        if entity.class == 3 {
            entity.control_flags |= ENTITY_DRAW_DISABLED_BIT;
        }
    }
    let peer = session
        .economy_mut()
        .allocate_authored_class5(164, 3, root_position)
        .expect("model-3 peer constructor");
    let peer_slot = peer.source_entity_index;
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for peer selection");
        root.action = 25;
        root.source_byte_3e = 0;
        root.source_byte_39 = 1;
        root.actual_speed = 0;
        root.yaw = 0;
        root.entity_rng = initial_rng;
        root.roll = 0;
        root.life = root.max_life;
        root.pending_word_0x62 = 0;
        root.word_0x96_150 = 0;
    }
    let root_max_life = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .expect("model-3 root life for peer selection")
        .max_life;
    for child in &mut session.economy_mut().authored_class5_model3_children {
        if child.parent_entity_index == root_slot {
            child.life = root_max_life;
            child.word_0x26 = 0;
        }
    }
    assert!(session.economy_mut().step_authored_class5_slot(root_slot));
    let after_peer_selection = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after peer selection");
    assert_eq!(after_peer_selection.action, 27);
    assert_eq!(
        session
            .economy()
            .authored_class5_model3()
            .iter()
            .find(|state| state.source_entity_index == root_slot)
            .expect("model-3 sidecar after peer selection")
            .word_0x32,
        peer_slot
    );

    // Out-of-range, disabled, and out-of-FOV class-3 candidates never win,
    // but each cadence still consumes exactly the two source LCG draws and
    // applies the masked roll update.
    let rejected_target_cases = [
        (
            "out-of-range",
            Position {
                x: root_position.x.wrapping_add(0x2000),
                y: root_position.y,
                z: root_position.z,
            },
            false,
        ),
        ("disabled", root_position, true),
        (
            "out-of-fov",
            Position {
                x: root_position.x,
                y: root_position.y.wrapping_add(1_024),
                z: root_position.z,
            },
            false,
        ),
    ];
    for (label, target_position, disabled) in rejected_target_cases {
        for entity in &mut session.entities {
            if entity.class == 3 {
                entity.control_flags |= ENTITY_DRAW_DISABLED_BIT;
            }
        }
        session
            .set_level2_entity_position(player_slot, target_position)
            .expect("rejected class-3 target position");
        if !disabled {
            session
                .entities
                .get_mut(usize::from(player_slot))
                .expect("rejected class-3 target")
                .control_flags &= !ENTITY_DRAW_DISABLED_BIT;
        }
        let target_heading = heading_for_positions(root_position, target_position);
        let initial_rng = 0x1357;
        {
            let root = session
                .economy_mut()
                .authored_class5
                .iter_mut()
                .find(|source| source.source_entity_index == root_slot)
                .expect("model-3 root for rejected target");
            root.action = 25;
            root.source_byte_3e = 0;
            root.source_byte_39 = 1;
            root.actual_speed = 0;
            root.yaw = if label == "out-of-fov" {
                i16::try_from((target_heading + 0x0400) & 0x07ff)
                    .expect("rejected target heading fits")
            } else {
                0
            };
            root.entity_rng = initial_rng;
            root.roll = 0;
            root.life = root.max_life;
            root.pending_word_0x62 = 0;
            root.word_0x96_150 = 0;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        let after_rejection = session
            .economy()
            .authored_class5()
            .iter()
            .find(|source| source.source_entity_index == root_slot)
            .copied()
            .expect("model-3 root after rejected target");
        assert_ne!(after_rejection.action, 30, "{label} target must not win");
        assert_eq!(after_rejection.word_0x96_150, 0, "{label} target relation");
        assert_eq!(
            after_rejection.entity_rng,
            level2_next_rng16(level2_next_rng16(initial_rng)),
            "{label} RNG draw count"
        );
        assert_eq!(
            after_rejection.roll,
            expected_roll(initial_rng, 0),
            "{label} roll update"
        );
    }

    // The acquired target's action-26 hit callback remains reachable through
    // the native dynamic dispatcher and invokes the source sound-8 helper.
    session
        .set_level2_entity_position(player_slot, root_position)
        .expect("action-26 target position");
    session
        .entities
        .get_mut(usize::from(player_slot))
        .expect("action-26 target")
        .control_flags &= !ENTITY_DRAW_DISABLED_BIT;
    {
        let root = session
            .economy_mut()
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root_slot)
            .expect("model-3 root for action 26");
        root.action = 26;
        root.source_byte_3e = 0;
        root.actual_speed = 0;
        root.yaw = 0;
        root.life = root.max_life;
        root.word_0x96_150 = player_slot;
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after_action26 = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root_slot)
        .copied()
        .expect("model-3 root after action-26 hit");
    assert_eq!(after_action26.action, 26);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Level2DispatchEvent::Economy {
            event: Level2EconomyEvent::AuthoredClass5Model3Sound {
                source_entity_index,
                sound_id: 8,
            },
            ..
        } if *source_entity_index == root_slot
    )));

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
