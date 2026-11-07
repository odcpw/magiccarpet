use crate::{PlayerCommand, Position};

use super::super::super::{Mc2DispatchEvent, NativeWorld, tests::level_two_fixture_path};

#[test]
fn real_worm_impact_replacement_reaches_root_and_body_kill_births() {
    use crate::native_world::{FireballProjectileOrigin, Level2EconomyEvent, source_target};
    let session = crate::native_world::tests::level_two_campaign_session();
    let particles = &session.session.definition.presentation.particles;
    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let player = session.state.entity_index;
    let root = super::dynamic::construct_authored(
        &mut arena,
        &mut terrain,
        238,
        session.state.position,
        9000,
        particles,
    )
    .unwrap();
    let root_slot = root.source_entity_index;
    let body_slot = arena
        .economy
        .authored_class5_model3
        .iter()
        .find(|row| row.source_entity_index == root_slot)
        .unwrap()
        .word_0x34;
    let context = source_target::Context {
        particles: Some(particles),
        spells: Some(&session.session.state.live.shared_spells),
        ..source_target::Context::from_ordinary(
            &session.session.state.live.goats,
            &session.session.state.live.ordinary,
        )
    };
    // Use the real source1CC20 constructor and its ordinary9/0 consumer.
    // No flyer life, RNG, geometry or eligibility flag is edited. The current
    // player above the grounded Worm supplies the actual constructor target.
    let projectile =
        super::projectile::launch(&mut arena, &mut terrain, root_slot, player, context)
            .unwrap()
            .unwrap();
    let flyer = arena
        .economy
        .fireball_projectiles()
        .iter()
        .find(|row| row.active && row.source_entity_index == projectile)
        .unwrap();
    assert_eq!(
        (
            flyer.origin,
            flyer.source_class,
            flyer.source_model,
            flyer.payload
        ),
        (FireballProjectileOrigin::Worm, 9, 0, 500)
    );
    let mut impact = None;
    for _ in 0..22 {
        let step = crate::native_world::effects::step_projectile(
            &mut arena,
            &mut terrain,
            particles,
            projectile,
            context,
            &[],
            5120,
            None,
        )
        .unwrap();
        if let Some(slot) = step.impact_slot {
            impact = Some(slot);
            break;
        }
        assert!(
            step.live,
            "ordinary Worm flyer must yield its actual terminal product"
        );
    }
    let impact = impact.expect("the real21-life flyer naturally produces its terminal effect");
    let impact_handle = arena.handle(impact).unwrap();
    assert!(impact != root_slot && impact != body_slot);
    let product = &arena.entities[usize::from(impact)];
    assert_eq!((product.class, product.model, product.action), (10, 0, 0));
    assert_ne!(
        product.control_flags & 0x20000,
        0,
        "ordinary Impact construction supplies real source replacement eligibility"
    );
    assert!(arena.economy.live_arena_kind(impact).is_none());

    // A separate declared constructor-capacity control retains four genuine
    // addresses from the current free stack plus this actual raw candidate.
    // No address is invented, allocated in advance or made self-replaceable.
    {
        let mut limited = arena.clone();
        let mut limited_terrain = terrain.clone();
        let existing_free_stack = limited.economy.free_slots.clone();
        limited.economy.exhaust_new_event_capacity_for_test();
        // The existing helper isolates typed competitors as well as capacity.
        // Restore exactly the observed ordinary pool before retaining four.
        limited.economy.free_slots = existing_free_stack;
        for row in &mut limited.entities {
            if row.index != impact {
                row.set_control_flags(row.control_flags & !0x20000);
            }
        }
        limited.rebuild_free_arena(&[]);
        let ordinary = limited
            .economy
            .free_source_slots()
            .iter()
            .rev()
            .take(4)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(ordinary.len(), 4);
        assert!(ordinary.iter().all(|slot| limited.handle(*slot).is_none()));
        limited
            .economy
            .free_slots
            .retain(|slot| ordinary.contains(slot));
        let mut typed_candidates = Vec::new();
        limited
            .economy
            .for_each_live_replaceable_arena_row(|slot, kind| typed_candidates.push((slot, kind)));
        assert!(typed_candidates.is_empty());
        assert_eq!(
            limited
                .entities
                .iter()
                .filter(|row| row.active && row.class != 0 && row.control_flags & 0x20000 != 0)
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            vec![impact]
        );

        let position = root.position;
        let hash_before = limited.reset_hash_with_terrain(&limited_terrain);
        assert!(
            super::dynamic::construct_authored(
                &mut limited,
                &mut limited_terrain,
                239,
                position,
                9000,
                &particles[..104],
            )
            .is_none(),
            "all seventeen source particle geometries preflight before NewEvent"
        );
        assert_eq!(
            limited.reset_hash_with_terrain(&limited_terrain),
            hash_before
        );
        assert_eq!(limited.handle(impact), Some(impact_handle));

        let mut no_candidate = limited.clone();
        let mut no_candidate_terrain = limited_terrain.clone();
        no_candidate.economy.exhaust_new_event_capacity_for_test();
        no_candidate.finish_source_group();
        let failure_hash = no_candidate.reset_hash_with_terrain(&no_candidate_terrain);
        assert!(
            super::dynamic::construct_authored(
                &mut no_candidate,
                &mut no_candidate_terrain,
                239,
                position,
                9000,
                particles,
            )
            .is_none(),
            "zero ordinary capacity and no frozen candidate cannot publish a root"
        );
        assert_eq!(
            no_candidate.reset_hash_with_terrain(&no_candidate_terrain),
            failure_hash
        );

        let root_count_before = limited.rebuilt_class5_model_slots()[3].len();
        let partial = super::dynamic::construct_authored(
            &mut limited,
            &mut limited_terrain,
            239,
            position,
            9000,
            particles,
        )
        .expect("four actual ordinary slots and one raw candidate retain the allocated prefix");
        assert_eq!(partial.source_entity_index, ordinary[0]);
        assert_eq!(
            (partial.action, partial.max_life, partial.life, partial.mana),
            (25, 9000, 9000, 2250)
        );
        let mut expected_children = ordinary[1..].to_vec();
        expected_children.push(impact);
        let state = limited
            .economy
            .authored_class5_model3
            .iter()
            .find(|row| row.source_entity_index == partial.source_entity_index)
            .unwrap();
        assert_eq!(state.word_0x34, expected_children[0]);
        assert_eq!(state.control_flags & 0x20000, 0);
        for (index, &slot) in expected_children.iter().enumerate() {
            let child = limited
                .economy
                .authored_class5_model3_children
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .unwrap();
            assert_eq!(
                (
                    child.parent_entity_index,
                    child.action,
                    child.particle_index,
                    child.mana
                ),
                (partial.source_entity_index, 232, 89 + index as u16, 140)
            );
            assert_eq!(
                child.entity_rng, partial.entity_rng,
                "source qmemcpy follows each child NewEvent"
            );
            assert_eq!(
                child.word_0x32,
                if index == 0 {
                    partial.source_entity_index
                } else {
                    expected_children[index - 1]
                }
            );
            assert_eq!(
                child.word_0x34,
                expected_children.get(index + 1).copied().unwrap_or(0)
            );
            assert!(limited.economy.map_chains.is_linked(slot));
        }
        assert_eq!(
            limited
                .economy
                .authored_class5_model3_children
                .iter()
                .filter(|row| row.active && row.parent_entity_index == partial.source_entity_index)
                .count(),
            4
        );
        assert_eq!(
            limited.rebuilt_class5_model_slots()[3].len(),
            root_count_before + 1,
            "a partial four-body prefix contributes exactly one living root"
        );
        assert_eq!(limited.economy.free_slot_count(), 0);
        assert!(limited.resolve(impact_handle).is_none());
        assert_eq!(
            limited.slot_generation(impact),
            impact_handle.generation.wrapping_add(1)
        );
        let tile = limited
            .economy
            .map_chains
            .tile_of(partial.source_entity_index);
        let mut expected_map = vec![partial.source_entity_index];
        expected_map.extend(expected_children.iter().rev().copied());
        assert_eq!(
            limited
                .economy
                .map_chains
                .entity_chain((tile & 255) as u8, (tile >> 8) as u8)
                .into_iter()
                .take(5)
                .collect::<Vec<_>>(),
            expected_map,
            "sub4B6F0 links the root last after the sequential body prefix"
        );
    }

    // Explicit configured pressure at the action29 boundary. The actual
    // constructor phases and RNG are retained; only entry action, free-stack
    // capacity and competing eligibility are controlled. These are not claims
    // that ordinary gameplay exhausts every slot or makes the Worm replaceable.
    for dying_slot in [root_slot, body_slot] {
        for case in 0..4 {
            // 0=ordinary free capacity; 1=only the actual raw impact;
            // 2=no eligible fallback; 3=zero mana/nonzero owner, same impact.
            let mut current = arena.clone();
            let mut current_terrain = terrain.clone();
            let zero_mana = case == 3;
            let (position, yaw, id, mana, rng, phase) = if dying_slot == root_slot {
                let row = current
                    .economy
                    .authored_class5
                    .iter_mut()
                    .find(|row| row.source_entity_index == dying_slot)
                    .unwrap();
                row.action = 29;
                if zero_mana {
                    row.mana = 0;
                }
                let values = (
                    row.position,
                    row.yaw,
                    root_slot,
                    row.mana,
                    row.entity_rng,
                    row.source_byte_3e,
                );
                if zero_mana {
                    current.economy.source_owner.insert(dying_slot, 42);
                }
                values
            } else {
                let row = current
                    .economy
                    .authored_class5_model3_children
                    .iter_mut()
                    .find(|row| row.active && row.source_entity_index == dying_slot)
                    .unwrap();
                row.action = 29;
                if zero_mana {
                    row.mana = 0;
                    row.player_owner_entity_index = 42;
                }
                (
                    row.position,
                    row.yaw,
                    row.parent_entity_index,
                    row.mana,
                    row.entity_rng,
                    row.source_byte_3e,
                )
            };
            assert_eq!(
                phase & 7,
                0,
                "real root/body0 constructor phase reaches the death gate"
            );
            let before = source_target::read(&current, context, dying_slot)
                .unwrap()
                .unwrap();
            assert_eq!(
                before.control_flags & 0x20000,
                0,
                "ordinary Worm flags never admit source self replacement"
            );
            let handle = current.handle(dying_slot).unwrap();
            if case != 0 {
                current.economy.exhaust_new_event_capacity_for_test();
                for row in &mut current.entities {
                    if row.index != impact {
                        row.set_control_flags(row.control_flags & !0x20000);
                    }
                }
                current.rebuild_free_arena(&[]);
                // The source rebuild may reclaim the already terminal flyer.
                // Keep the declared capacity restriction after that real
                // maintenance; the captured raw fallback address is retained.
                current.economy.exhaust_new_event_capacity_for_test();
                assert_eq!(current.economy.free_slot_count(), 0);
                let mut typed_candidates = Vec::new();
                current
                    .economy
                    .for_each_live_replaceable_arena_row(|slot, kind| {
                        typed_candidates.push((slot, kind))
                    });
                assert!(
                    typed_candidates.is_empty(),
                    "declared candidate isolation: {typed_candidates:?}"
                );
                assert_eq!(
                    current
                        .entities
                        .iter()
                        .filter(|row| row.active
                            && row.class != 0
                            && row.control_flags & 0x20000 != 0)
                        .map(|row| row.index)
                        .collect::<Vec<_>>(),
                    vec![impact]
                );
                if case == 2 {
                    current.finish_source_group();
                }
            }
            let event_start = current.economy.events().len();
            super::mc2::step_action29_slot(&mut current, &mut current_terrain, dying_slot, context)
                .unwrap();
            assert_eq!(
                current.handle(dying_slot),
                Some(handle),
                "hide retains the original generation in this pass"
            );
            let after = source_target::read(&current, context, dying_slot)
                .unwrap()
                .unwrap();
            assert_ne!(after.control_flags & 0x400, 0);
            assert_eq!((after.position, after.source_id), (position, id));
            assert!(
                !super::dynamic::presentation_candidates(&current).any(|row| row.0 == dying_slot)
            );
            let (after_rng, after_owner) = if dying_slot == root_slot {
                (
                    current
                        .economy
                        .authored_class5
                        .iter()
                        .find(|row| row.source_entity_index == dying_slot)
                        .unwrap()
                        .entity_rng,
                    current.economy.source_owner(dying_slot),
                )
            } else {
                let row = current
                    .economy
                    .authored_class5_model3_children
                    .iter()
                    .find(|row| row.source_entity_index == dying_slot)
                    .unwrap();
                (row.entity_rng, row.player_owner_entity_index)
            };
            assert_eq!(
                after_rng,
                if zero_mana {
                    rng
                } else {
                    crate::native_world::level2_next_rng16(rng)
                }
            );
            assert_eq!(after_owner, if zero_mana { 42 } else { 0 });
            let spheres = current.economy.events()[event_start..]
                .iter()
                .filter_map(|event| {
                    if let Level2EconomyEvent::ManaSphereSpawned {
                        sphere_id,
                        source_entity_index,
                        mana,
                    } = event
                        && *source_entity_index == dying_slot
                    {
                        Some((*sphere_id, *mana))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let bursts = current
                .effects
                .death_burst_origins()
                .iter()
                .filter_map(|(&slot, &(generation, parent))| {
                    (parent == dying_slot && current.slot_generation(slot) == generation)
                        .then_some(slot)
                })
                .collect::<Vec<_>>();
            assert_eq!(spheres.len(), usize::from(matches!(case, 0 | 1)));
            assert_eq!(bursts.len(), usize::from(matches!(case, 0 | 3)));
            if let Some(&(sphere_id, amount)) = spheres.first() {
                assert_eq!(amount, mana);
                let sphere = current
                    .economy
                    .spheres()
                    .iter()
                    .find(|row| row.active && row.id == sphere_id)
                    .unwrap();
                assert_eq!(
                    (sphere.position, sphere.mana, sphere.player_entity_index),
                    (position, mana, 0)
                );
                assert!((sphere.yaw.wrapping_sub(yaw).wrapping_add(56) & 2047) < 113);
                if case == 1 {
                    assert_eq!(sphere.source_entity_index, impact);
                }
            }
            if let Some(&burst) = bursts.first() {
                let row = &current.entities[usize::from(burst)];
                assert_eq!(
                    (row.class, row.model, row.source_id, row.position),
                    (10, 1, id, position)
                );
                if case == 3 {
                    assert_eq!(burst, impact);
                }
            }
            if matches!(case, 1 | 3) {
                assert!(
                    current.resolve(impact_handle).is_none(),
                    "actual raw candidate generation was consumed"
                );
                assert_eq!(
                    current.slot_generation(impact),
                    impact_handle.generation.wrapping_add(1)
                );
            } else {
                assert_eq!(current.handle(impact), Some(impact_handle));
            }
            // Run the real next source boundary, rather than directly marking
            // the hidden owner inactive or releasing its slot in the fixture.
            let mut following = session.clone();
            following.session.state.live.economy = current;
            following.session.state.live.terrain = current_terrain;
            following.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                following
                    .dispatch_events()
                    .iter()
                    .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "following reclaim: {:?}",
                following.dispatch_events()
            );
            assert!(
                following.source_arena().resolve(handle).is_none(),
                "source boundary naturally reclaims the original hidden root/body generation"
            );
        }
    }
}

#[test]
fn alliance_impact_drives_typed_worm_selected_attack_and_expiry() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let parent = session.state.entity_index;
    let position = session.state.position;
    let source = super::dynamic::construct_authored(&mut arena, &mut terrain, 238, position, 9000,
        &session.session.definition.presentation.particles).expect("ordinary typed Worm producer");
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
    let target = super::dynamic::construct_authored(&mut arena, &mut terrain, 239, position, 9000,
        &session.session.definition.presentation.particles).unwrap().source_entity_index;
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
    // This pre-existing no-fallback fixture must also clear the shared raw
    // replacement cursor now that the constructor uses canonical NewEvent.
    session.session.state.live.economy.finish_source_group();
    let root = {
        let live = &mut session.session.state.live;
        super::dynamic::construct_authored(
            &mut live.economy, &mut live.terrain, 238,
            Position {
                x: 0x4000,
                y: 0x5000,
                z: 0,
            },
            9000, &session.session.definition.presentation.particles,
        )
        .expect("Worm root remains published after body exhaustion")
    };
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
    let root = {
        let live = &mut session.session.state.live;
        super::dynamic::construct_authored(
            &mut live.economy, &mut live.terrain, 900,
            Position {
                x: 10_000,
                y: 10_000,
                z: 0,
            },
            9000, &session.session.definition.presentation.particles,
        )
        .expect("first focused Worm root")
    };
    let peer = {
        let live = &mut session.session.state.live;
        super::dynamic::construct_authored(
            &mut live.economy, &mut live.terrain, 901,
            Position {
                x: 20_000,
                y: 10_000,
                z: 0,
            },
            9000, &session.session.definition.presentation.particles,
        )
        .expect("second focused Worm root")
    };
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
