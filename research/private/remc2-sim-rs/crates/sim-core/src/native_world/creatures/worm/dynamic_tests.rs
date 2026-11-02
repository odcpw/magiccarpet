use crate::{PlayerCommand, Position};

use super::super::super::{Mc2DispatchEvent, NativeWorld, tests::level_two_fixture_path};

#[test]
fn alliance_impact_drives_typed_worm_selected_attack_and_expiry() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let parent = session.state.entity_index;
    let position = session.state.position;
    let source = arena.economy.allocate_authored_class5(238, 3, position, &terrain).expect("ordinary typed Worm producer");
    let slot = source.source_entity_index;
    let enemy = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    let mut target = crate::native_world::cave_features::new_event(enemy, 5, position);
    target.class = 5; target.model = 21; target.record[67] = 5; target.record[68] = 21;
    target.set_life(1_000);
    arena.entities[usize::from(enemy)] = target;
    arena.entities[usize::from(parent)].record[154..156].copy_from_slice(&enemy.to_le_bytes());
    let impact = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
    let effect = &mut arena.entities[usize::from(impact)];
    effect.source_id = parent; effect.record[30..32].copy_from_slice(&parent.to_le_bytes());
    effect.record[74] = 32; effect.subspell_index = 40;
    effect.record[154..156].copy_from_slice(&slot.to_le_bytes());
    assert_eq!(crate::native_world::alliance_spell::step_impact(&mut arena, impact, &[slot]).unwrap().affected_entities, vec![slot]);
    let state = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap();
    assert_eq!((state.stage_var2, state.parent_entity_index), (14, parent));
    let mut mixed = arena.clone();
    let mut mixed_terrain = terrain.clone();
    mixed.entities[usize::from(parent)].record[154..156].fill(0);
    let peer = mixed.allocate_source_slot_with_terrain(&mut mixed_terrain).unwrap();
    let peer_position = Position { x: position.x.wrapping_add(32), ..position };
    let mut imported_peer = crate::native_world::cave_features::new_event(peer, 5, peer_position);
    imported_peer.class = 5; imported_peer.model = 3; imported_peer.record[67] = 5; imported_peer.record[68] = 3;
    imported_peer.set_life(1_000);
    mixed.entities[usize::from(peer)] = imported_peer;
    mixed.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap().source_byte_3e = 8;
    super::dynamic::step_alliance_stage_slot(&mut mixed, &mut mixed_terrain, slot, &[slot, peer], Default::default()).unwrap();
    let root = mixed.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
    assert_eq!(i32::from(root.roll), crate::heading_for_positions(peer_position, root.position),
        "the frozen mixed-carrier model3 list controls source separation");
    let feedback = super::dynamic::step_alliance_stage_slot(&mut arena, &mut terrain, slot, &[slot], Default::default()).unwrap();
    assert_eq!(feedback, vec![(parent, 24)]);
    let root = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
    assert_eq!((root.action, root.word_0x96_150), (26, enemy));
    let before = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap().stage_remaining;
    arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap().source_byte_3e = 0;
    let before_projectiles = arena.economy.fireball_projectiles().iter().filter(|row| row.active).count();
    super::dynamic::step_alliance_attack_slot(&mut arena, &mut terrain, slot, &[slot], Default::default()).unwrap();
    assert_eq!(arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap().stage_remaining, before - 1);
    assert_eq!(arena.economy.fireball_projectiles().iter().filter(|row| row.active).count(), before_projectiles + 1,
        "the selected target drives the real Worm Fireball constructor");
    let mut player_target = arena.clone();
    let mut player_target_terrain = terrain.clone();
    let other_player = player_target.allocate_source_slot_with_terrain(&mut player_target_terrain).unwrap();
    let mut other = crate::native_world::cave_features::new_event(other_player, 5, position);
    other.class = 3; other.model = 0; other.record[67] = 3; other.record[68] = 0;
    other.set_life(1_000);
    player_target.entities[usize::from(other_player)] = other;
    player_target.entities[usize::from(parent)].record[154..156].copy_from_slice(&other_player.to_le_bytes());
    let root = player_target.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
    root.action = 26; root.source_byte_3e = 0; root.word_0x96_150 = other_player;
    super::dynamic::step_alliance_attack_slot(&mut player_target, &mut player_target_terrain, slot, &[slot], Default::default()).unwrap();
    assert_eq!(i16::from_le_bytes(player_target.entities[usize::from(other_player)].record[58..60].try_into().unwrap()), 100,
        "a successful shot applies the source player-target alert");
    let root = arena.economy.authored_class5.iter_mut().find(|row| row.source_entity_index == slot).unwrap();
    root.action = 31;
    arena.economy.authored_class5_model3.iter_mut().find(|row| row.source_entity_index == slot).unwrap().stage_remaining = 1;
    super::dynamic::step_alliance_stage_slot(&mut arena, &mut terrain, slot, &[slot], Default::default()).unwrap();
    let state = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap();
    assert_eq!((state.stage_var2, state.parent_entity_index), (10, 0));
    assert!(arena.handle(slot).is_some());
    super::dynamic::prepare_alliance_stage_pass(&mut arena, &mut runtime, &[slot], Default::default()).unwrap();
    let root = arena.economy.authored_class5.iter().find(|row| row.source_entity_index == slot).unwrap();
    let state = arena.economy.authored_class5_model3.iter().find(|row| row.source_entity_index == slot).unwrap();
    assert_eq!((root.action, state.stage_var1, state.stage_var2), (25, 0, 0),
        "the next source prepass returns the same Worm to its ordinary route");
}

#[test]
fn imported_allied_worm_follows_and_attacks_current_typed_target() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let position = session.state.position;
    let parent = session.state.entity_index;
    let target = arena.economy.allocate_authored_class5(239, 3, position, &terrain).unwrap().source_entity_index;
    let slot = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    let mut root = crate::native_world::cave_features::new_event(slot, 5, position);
    root.class = 5; root.model = 3; root.record[67] = 5; root.record[68] = 3;
    root.set_life(1_000); root.set_source_action(31); root.set_phase(0);
    root.record[77] = 14; root.record[44..46].copy_from_slice(&parent.to_le_bytes());
    root.record[50..52].copy_from_slice(&40_i16.to_le_bytes());
    root.record[52..54].copy_from_slice(&40_i16.to_le_bytes());
    arena.entities[usize::from(slot)] = root;
    arena.entities[usize::from(parent)].record[154..156].copy_from_slice(&target.to_le_bytes());
    let (_, feedback) = super::imported::step_imported_alliance_stage(
        &mut arena, &terrain, slot, &[slot, target], Default::default(),
    ).unwrap();
    assert_eq!(feedback, vec![(parent, 24)]);
    assert_eq!((arena.entities[usize::from(slot)].action, arena.entities[usize::from(slot)].source_word_96()), (26, target));
    arena.entities[usize::from(slot)].set_phase(0);
    let attack = super::imported::step_imported_alliance_attack(
        &mut arena, &terrain, slot, &[slot, target], Default::default(),
    ).unwrap();
    let launch = attack.launch.unwrap();
    assert_eq!(launch.target_source_entity_index, target,
        "the real imported attack emits a projectile toward the current typed carrier");
    assert_eq!((launch.target_class, launch.target_model), (5, 3));
}

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
