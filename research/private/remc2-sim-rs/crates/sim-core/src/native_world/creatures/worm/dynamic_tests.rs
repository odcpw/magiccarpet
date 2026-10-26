use crate::{PlayerCommand, Position};

use super::super::super::{Mc2DispatchEvent, NativeWorld, tests::level_two_fixture_path};

#[test]
fn model3_constructor_retains_allocated_prefix_on_partial_exhaustion() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Worm constructor session");
    let baseline_hash = session.campaign_state_hash();

    {
        let economy = &mut session.economy_mut();
        economy.replacement_cursor_exclusions.extend(1_u16..=999);
        // allocate_slot pops the tail: root 700, then body rows 701..703.
        economy.free_slots = vec![703, 702, 701, 700];
    }
    let root = session
        .economy_mut()
        .allocate_authored_class5(
            238,
            3,
            Position {
                x: 0x4000,
                y: 0x5000,
                z: 0,
            },
        )
        .expect("Worm root remains published after body exhaustion");
    assert_eq!(root.source_entity_index, 700);
    assert_eq!(root.action, 25);
    assert_eq!(root.model, 3);
    assert_eq!(root.fov, 0);

    let state = session
        .economy()
        .authored_class5_model3()
        .iter()
        .find(|state| state.source_entity_index == root.source_entity_index)
        .copied()
        .expect("partial Worm retains its root sidecar");
    assert_eq!(state.word_0x34, 701);
    assert_eq!(
        (
            state.render_yaw,
            state.render_pitch,
            state.render_roll,
            state.render_fov
        ),
        (90, 414, 414, 108)
    );

    let children = session
        .economy()
        .authored_class5_model3_children()
        .iter()
        .filter(|child| child.parent_entity_index == root.source_entity_index)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3);
    assert_eq!(
        children
            .iter()
            .map(|child| child.source_entity_index)
            .collect::<Vec<_>>(),
        vec![701, 702, 703]
    );
    assert_eq!(children[0].word_0x32, 700);
    assert_eq!(children[0].word_0x34, 702);
    assert_eq!(children[1].word_0x32, 701);
    assert_eq!(children[1].word_0x34, 703);
    assert_eq!(children[2].word_0x32, 702);
    assert_eq!(children[2].word_0x34, 0);
    assert!(
        children
            .iter()
            .all(|child| child.entity_rng == root.entity_rng)
    );
    assert!(
        children
            .iter()
            .all(|child| child.source_byte_39 == root.source_byte_39)
    );
    assert_eq!(
        children
            .iter()
            .map(|child| child.source_byte_3e)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(children.iter().all(|child| child.yaw == root.yaw));
    assert_eq!(session.economy().source_owner(root.source_entity_index), 0);
    assert!(
        children
            .iter()
            .all(|child| child.player_owner_entity_index == 0)
    );
    let tile = session
        .economy()
        .map_chains
        .tile_of(root.source_entity_index);
    assert_eq!(
        session
            .economy()
            .map_chains
            .entity_chain((tile & 0xff) as u8, (tile >> 8) as u8)
            .into_iter()
            .take(4)
            .collect::<Vec<_>>(),
        vec![700, 703, 702, 701],
        "source links the root last, ahead of its reverse body allocation order"
    );
    for slot in 700_u16..=703 {
        assert!(session.economy().map_chains.is_linked(slot));
        assert!(!session.economy().free_slots.contains(&slot));
    }

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn model3_action27_uses_original_peer_speed_and_sentinel_chain_target() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 Worm peer session");
    let baseline_hash = session.campaign_state_hash();
    let root = session
        .economy_mut()
        .allocate_authored_class5(
            900,
            3,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
        )
        .expect("first focused Worm root");
    let peer = session
        .economy_mut()
        .allocate_authored_class5(
            901,
            3,
            Position {
                x: 20_000,
                y: 10_000,
                z: 0,
            },
        )
        .expect("second focused Worm root");
    let expected_speed = peer.maximum_speed.wrapping_add(peer.actual_speed);
    {
        let economy = &mut session.economy_mut();
        let source = economy
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == root.source_entity_index)
            .expect("first Worm root remains live");
        source.action = 27;
        source.source_byte_3e = 0;
        source.phase = 0;
        source.actual_speed = 0;
        let linked = economy
            .authored_class5
            .iter_mut()
            .find(|source| source.source_entity_index == peer.source_entity_index)
            .expect("second Worm root remains live");
        linked.action = 27;
        let source_state = economy
            .authored_class5_model3
            .iter_mut()
            .find(|state| state.source_entity_index == root.source_entity_index)
            .expect("first Worm peer sidecar");
        source_state.word_0x32 = peer.source_entity_index;
        let peer_state = economy
            .authored_class5_model3
            .iter_mut()
            .find(|state| state.source_entity_index == peer.source_entity_index)
            .expect("second Worm peer sidecar");
        peer_state.word_0x32 = 0;
    }

    session.step_outer_turn(PlayerCommand::default(), 0);

    let root_after = session
        .economy()
        .authored_class5()
        .iter()
        .find(|source| source.source_entity_index == root.source_entity_index)
        .expect("linked Worm remains after its ordinary scheduler visit");
    let state_after = session
        .economy()
        .authored_class5_model3()
        .iter()
        .find(|state| state.source_entity_index == root.source_entity_index)
        .expect("linked Worm sidecar remains");
    assert_eq!(root_after.action, 27);
    assert_eq!(state_after.word_0x32, 0);
    assert_eq!(root_after.actual_speed, expected_speed);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
    );

    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}
