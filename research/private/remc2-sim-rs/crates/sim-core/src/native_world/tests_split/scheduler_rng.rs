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
fn source31_authored_model28_scheduled_contact_and_expiry_reclaim() {
    let root = std::env::var_os("MC_SECRET_CAMPAIGN_PACKAGE_ROOT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated"),
        PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        root.join("mc-paj2-source-secret-31-campaign-package"),
        crate::campaign::spec::CampaignSelection::secret_destination(31).unwrap(),
    )
    .expect("real Source31 package opens")
    .simulation()
    .clone();
    let slot = session
        .entities()
        .iter()
        .find(|row| row.active && row.class == 5 && row.model == 28)
        .expect("Source31 constructs its authored model28 actor")
        .index;
    let handle = session
        .session
        .state
        .live
        .economy
        .handle(slot)
        .expect("authored actor generation");
    let player_slot = session.state.entity_index;
    let player_position = session.entities()[usize::from(player_slot)].position;

    // Controlled contact setup on the actual authored row: select the source
    // attack frame and a live player target without replacing either actor.
    let attacker = {
        let live = &mut session.session.state.live;
        let row = &mut live.economy.entities[usize::from(slot)];
        let attacker = row.source_id;
        row.set_position(player_position);
        row.set_source_action(226);
        row.set_source_byte_46(4);
        row.set_source_dword_10(8);
        row.set_subspell_index(37);
        row.set_phase(0);
        row.set_control_flags(row.control_flags | 0x800);
        // Configured attack countdown uses the source resident-frame word2C;
        // particle playback limit97 is a separate field, not this count.
        row.record[48..50].copy_from_slice(&16_u16.to_le_bytes());
        row.record[97] = 16;
        row.record[154..156].copy_from_slice(&player_slot.to_le_bytes());
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            slot,
            player_position,
            true,
            false,
        )
        .expect("authored actor remains linked in terrain");
        attacker
    };
    assert!(
        slot < player_slot,
        "the player consumes this earlier contact in the same pass"
    );
    let life_before = session.session.state.live.player_life;
    let mut protected = session.clone();
    assert!(protected.session.state.live.player_protection_ticks > 0);
    protected.step_level2_source_pass(0);
    assert_eq!(protected.session.state.live.player_life, life_before);
    assert_eq!(protected.session.state.live.player_pending_damage, 0);
    assert_eq!(protected.session.state.live.player_pending_attacker, 0);
    assert_eq!(
        &protected.entities()[usize::from(player_slot)].record[98..104],
        &[0; 6],
        "spawn protection clears the complete incoming contact mailbox"
    );
    assert!(!protected.economy().events.iter().any(|event| matches!(
        event,
        Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            ..
        } if *target_entity_index == player_slot && *attacker_entity_index == attacker
    )));

    // Same authored actor, attack frame and player geometry, after spawn
    // protection has expired. Source slot 1 precedes the player's slot 582:
    // the ordinary player callback consumes this hit within this pass.
    session.session.state.live.player_protection_ticks = 0;
    session.step_level2_source_pass(0);
    let row = &session.entities()[usize::from(slot)];
    assert_eq!(
        row.record[74], 5,
        "scheduled attack consumes its contact frame"
    );
    assert_eq!(
        row.phase, 1,
        "the common same-pass source tail advances once"
    );
    assert_eq!(session.session.state.live.player_life, life_before - 37);
    assert_eq!(session.session.state.live.player_pending_damage, 37);
    assert_eq!(session.session.state.live.player_pending_attacker, 0);
    assert!(session.economy().events.iter().any(|event| matches!(
        event,
        Level2EconomyEvent::DamageApplied {
            target_entity_index,
            attacker_entity_index,
            damage: 37,
            lethal: false,
        } if *target_entity_index == player_slot && *attacker_entity_index == attacker
    )));
    let player = &session.entities()[usize::from(player_slot)];
    assert_eq!(
        i32::from_le_bytes(player.record[98..102].try_into().unwrap()),
        37
    );
    assert_eq!(
        u16::from_le_bytes(player.record[102..104].try_into().unwrap()),
        0,
        "the player's same-pass nonlethal callback clears only the mailbox owner"
    );
    assert_eq!(
        u16::from_le_bytes(player.record[42..44].try_into().unwrap()),
        attacker
    );

    // Controlled source StageVar16 expiry on that same generation. The
    // callback hides this actor; only the following pass may reclaim it.
    {
        let row = &mut session.session.state.live.economy.entities[usize::from(slot)];
        row.set_source_action(231);
        row.record[77] = 16;
        row.record[44..46].fill(0);
        row.record[50..52].fill(0);
    }
    session.step_level2_source_pass(1);
    assert!(session.entities()[usize::from(slot)].drawing_disabled());
    assert!(
        session.session.state.live.economy.resolve(handle).is_some(),
        "hidden row survives its callback pass"
    );
    session.step_level2_source_pass(2);
    assert!(
        session.session.state.live.economy.resolve(handle).is_none(),
        "following pass reclaims the original generation"
    );
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
        let particle_index = if particle_rng & 1 != 0 { 84 } else { 83 };
        let particle = session.session.definition.presentation.particles[particle_index as usize];
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
                particle_index,
                (particle.rotation_speed / 2) as i16,
                (particle.speed / 2) as i16,
                (particle.speed / 2) as i16,
                (particle.rotation_speed / 2) as i16,
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
    let presentation = session
        .campaign_entity_presentations(0)
        .expect("actual campaign sprite projection")
        .into_iter()
        .find(|row| row.entity_index == tree_before.source_entity_index)
        .expect("authored Tree reaches the sprite consumer");
    assert_eq!(presentation.source_id, tree_before.entity_id);
    assert_eq!(
        presentation.render_parameter_index,
        tree_before.particle_index
    );
    assert_eq!(presentation.position, tree_before.position);
    assert_eq!(presentation.yaw, 0);
    let mut hidden = session.clone();
    assert!(crate::world::scenery::trees::hide_for_terrain(
        hidden.source_arena_mut(),
        tree_before.source_entity_index,
        tree_before.entity_id.wrapping_add(1),
    ));
    assert!(
        hidden
            .campaign_entity_presentations(0)
            .expect("hidden Tree sprite projection")
            .iter()
            .all(|row| row.entity_index != tree_before.source_entity_index)
    );
    assert!(!session.economy().events().iter().any(|event| matches!(
        event,
        Level2EconomyEvent::ManaSphereSpawned {
            source_entity_index,
            ..
        } if *source_entity_index == tree_before.source_entity_index
    )));

    // The actual shared area-damage producer writes the Tree mailbox between
    // source passes. The next pass reaches the ordinary Tree callback.
    let owner_entity_index = session.state.entity_index;
    assert!(
        crate::native_world::area_damage::apply(
            session.source_arena_mut(),
            None,
            None,
            crate::combat::area::AreaDamage {
                position: tree_before.position,
                shape: crate::combat::area::Shape {
                    half_width: 64,
                    half_depth: 64,
                    vertical_offset: tree_before.yaw,
                    half_height: tree_before.fov,
                },
                owner: owner_entity_index,
                payload: u16::try_from(tree_before.life + 1)
                    .expect("Tree life fits damage payload"),
                selector: (tree_before.source_class, tree_before.model),
                mode: 0,
            },
        )
        .expect("shared area damage visits Tree")
            > 0
    );
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
    let child = session
        .economy()
        .authored_model6()
        .iter()
        .find(|child| child.source_entity_index == tree_after.last_sphere_source_entity_index)
        .copied()
        .expect("tree callback model-6 child remains live");
    let owner_id = session
        .economy
        .entities
        .get(usize::from(owner_entity_index))
        .map_or(owner_entity_index, |entity| entity.source_id);
    assert_eq!(child.entity_id, owner_id);
    assert_eq!(
        (child.model, child.action, child.subspell_index),
        (6, 6, 50)
    );
    assert_eq!(child.height_offset, tree_after.last_sphere_word_0x2c);
    assert_eq!(i32::from(child.life), tree_after.life);

    // Continue the actual action-1 Tree from the successful source birth.
    // The neighboring life values exercise the strict source threshold.
    let mut lifecycle = session.clone();
    let particles = &lifecycle.session.definition.presentation.particles;
    let live = &mut lifecycle.session.state.live;
    let arena = &mut live.economy;
    let terrain = &mut live.terrain;
    let slot = tree_after.source_entity_index;
    let expected_particle = match tree_after.particle_index {
        83 => 226,
        84 => 227,
        other => panic!("real Tree constructor selected unexpected particle {other}"),
    };
    let binding = particles[expected_particle as usize];
    {
        let tree = arena.economy.authored_tree_mut(slot).unwrap();
        tree.life = 61;
        tree.owner_entity_index = owner_entity_index;
        tree.pending_damage = 123;
    }
    assert!(
        crate::world::scenery::trees::step(arena, terrain, Default::default(), particles, slot)
            .unwrap()
    );
    let tree = *arena.economy.authored_tree_mut(slot).unwrap();
    assert_eq!(
        (tree.life, tree.action, tree.particle_index),
        (60, 1, tree_after.particle_index)
    );
    assert_eq!(tree.phase, tree_after.phase.wrapping_add(1));
    assert_eq!(
        (tree.owner_entity_index, tree.pending_damage),
        (owner_entity_index, 123)
    );
    assert!(
        crate::world::scenery::trees::step(arena, terrain, Default::default(), particles, slot)
            .unwrap()
    );
    let tree = *arena.economy.authored_tree_mut(slot).unwrap();
    assert_eq!(
        (tree.life, tree.action, tree.particle_index),
        (59, 2, expected_particle)
    );
    assert_eq!(tree.phase, tree_after.phase.wrapping_add(2));
    assert_eq!(
        (tree.yaw, tree.fov),
        (
            (binding.rotation_speed / 2) as i16,
            (binding.rotation_speed / 2) as i16
        )
    );
    assert_eq!(
        (tree.pitch, tree.roll),
        ((binding.speed / 2) as i16, (binding.speed / 2) as i16)
    );
    assert_eq!(
        (tree.owner_entity_index, tree.pending_damage),
        (owner_entity_index, 123)
    );
    assert!(
        crate::world::scenery::trees::step(arena, terrain, Default::default(), particles, slot)
            .unwrap()
    );
    let tree = *arena.economy.authored_tree_mut(slot).unwrap();
    assert_eq!(
        (tree.life, tree.action, tree.particle_index),
        (59, 2, expected_particle)
    );
    assert_eq!(tree.phase, tree_after.phase.wrapping_add(3));
    assert_eq!(
        (tree.owner_entity_index, tree.pending_damage),
        (owner_entity_index, 123)
    );
    let tree = arena.economy.authored_tree_mut(slot).unwrap();
    tree.life = 60;
    tree.action = 1;
    tree.particle_index = tree_after.particle_index;
    tree.owner_entity_index = 0;
    lifecycle.step_outer_turn(PlayerCommand::default(), 0);
    let scheduled = lifecycle
        .economy()
        .authored_trees()
        .iter()
        .find(|tree| tree.source_entity_index == slot)
        .expect("source scheduler retains the post-child Tree");
    assert_eq!(
        (scheduled.life, scheduled.action, scheduled.particle_index),
        (59, 2, expected_particle)
    );
    let scheduled_child = lifecycle
        .economy()
        .authored_model6()
        .iter()
        .find(|row| row.source_entity_index == child.source_entity_index)
        .expect("Tree child reaches the ordinary model-6 consumer");
    assert_eq!(scheduled_child.life, child.life - 1);
    assert_eq!(scheduled_child.phase, child.phase.wrapping_add(1));
    assert!(lifecycle.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            source_slot,
            kind: Level2DynamicArenaKind::AuthoredModel6,
            ..
        } if *source_slot == child.source_entity_index
    )));

    // Fill the real arena, then consume earlier frozen replacement addresses
    // through the ordinary sphere constructor until a previously visited
    // Tree is the next fixed address. This is a controlled source boundary,
    // not an earned gameplay traversal.
    let mut alias = level_two_campaign_session();
    alias.runtime.set_level2_is_level_end(1);
    alias.step_outer_turn(PlayerCommand::default(), 0);
    alias.step_outer_turn(PlayerCommand::default(), 0);
    let owner = alias.state.entity_index;
    while alias.economy().free_slot_count() > 0 {
        alias
            .allocate_level2_authored_class14(244)
            .expect("real class14 constructor fills free arena");
    }
    let particles = &alias.session.definition.presentation.particles;
    let live = &mut alias.session.state.live;
    let arena = &mut live.economy;
    let terrain = &mut live.terrain;
    let tree_slot = arena
        .economy
        .authored_trees()
        .iter()
        .filter(|tree| tree.active && tree.control_flags & SOURCE_REPLACEABLE_FLAG != 0)
        .map(|tree| tree.source_entity_index)
        .min()
        .expect("ordinary Tree callback published a replaceable row");
    let generation = arena.economy.slot_generation(tree_slot);
    let rng_before_failure;
    {
        let tree = arena.economy.authored_tree_mut(tree_slot).unwrap();
        // A fresh callback flag cannot enter an already-cleared source list.
        tree.control_flags &= !SOURCE_REPLACEABLE_FLAG;
        tree.owner_entity_index = owner;
        tree.pending_damage = tree.life + 1;
        rng_before_failure = tree.entity_rng;
    }
    arena.finish_source_group();
    assert!(
        crate::world::scenery::trees::step(
            arena,
            terrain,
            Default::default(),
            particles,
            tree_slot
        )
        .unwrap()
    );
    {
        let tree = arena
            .economy
            .authored_tree_mut(tree_slot)
            .expect("empty frozen list retains Tree");
        assert!(tree.life < 0);
        assert_eq!(tree.owner_entity_index, 0);
        assert_eq!(tree.entity_rng, rng_before_failure);
        assert_ne!(tree.control_flags & SOURCE_REPLACEABLE_FLAG, 0);
        assert_eq!(tree.last_sphere_source_entity_index, 0);
        assert!(tree.drawing_enabled, "real selected Tree remains on land");
        tree.life = tree.max_life;
        tree.owner_entity_index = owner;
        tree.pending_damage = tree.life + 1;
    }
    arena.rebuild_free_arena(&[]);
    assert_eq!(arena.economy.free_slot_count(), 0);
    let dynamic = arena
        .economy
        .live_arena_slots()
        .into_iter()
        .map(|(slot, _)| slot)
        .collect::<std::collections::BTreeSet<_>>();
    let mut candidates = arena
        .entities
        .iter()
        .filter(|row| {
            row.index != 0
                && row.active
                && row.class != 0
                && !dynamic.contains(&row.index)
                && row.control_flags & SOURCE_REPLACEABLE_FLAG != 0
        })
        .map(|row| row.index)
        .collect::<Vec<_>>();
    arena
        .economy
        .for_each_live_replaceable_arena_row(|slot, _| candidates.push(slot));
    candidates.sort_unstable();
    candidates.dedup();
    assert!(candidates.contains(&tree_slot));
    for expected in candidates.into_iter().take_while(|slot| *slot < tree_slot) {
        let slot = arena
            .allocate_source_slot_with_terrain(terrain)
            .expect("earlier frozen address");
        assert_eq!(slot, expected);
        let position = Position { x: 0, y: 0, z: 0 };
        let sphere = arena
            .economy
            .initialize_authored_model39(slot, tree_slot, position)
            .expect("ordinary sphere constructor at frozen address");
        crate::world::effects::sync_position(arena, terrain, slot, sphere.position, true, false)
            .unwrap();
    }
    let expected_rng = level2_next_rng16(arena.economy.new_event_rng(tree_slot));
    // An obsolete raw mirror must not outlive the current typed occupant.
    arena.entities[usize::from(tree_slot)].active = true;
    arena.entities[usize::from(tree_slot)].class = 2;
    assert!(
        crate::world::scenery::trees::step(
            arena,
            terrain,
            Default::default(),
            particles,
            tree_slot
        )
        .unwrap()
    );
    assert_eq!(
        arena.economy.slot_generation(tree_slot),
        generation.wrapping_add(1)
    );
    assert!(
        arena
            .economy
            .authored_trees()
            .iter()
            .all(|tree| tree.source_entity_index != tree_slot)
    );
    let replacement = arena
        .economy
        .authored_model6()
        .iter()
        .find(|child| child.active && child.source_entity_index == tree_slot)
        .expect("source Tree address now owns its actual model-6 child");
    assert_eq!((replacement.position.x, replacement.position.y), (0, 0));
    assert_eq!(replacement.action, 1);
    assert_eq!(replacement.entity_rng, expected_rng);
    assert_eq!(replacement.life, (expected_rng % 60 + 130) as i16);
    assert_eq!(replacement.height_offset, 1152);
    assert_eq!((replacement.model, replacement.subspell_index), (6, 50));
    assert_eq!(replacement.phase, (tree_slot as u8).wrapping_add(1));
    assert_ne!(replacement.control_flags & SOURCE_REPLACEABLE_FLAG, 0);
    assert_eq!(
        crate::FlightWorld::material_mask(terrain, replacement.position),
        crate::WATER_AMBIENCE_MASK
    );
    assert_ne!(
        replacement.control_flags & crate::level1_mission::SOURCE_DISABLED_FLAG,
        0,
        "the Tree water tail hides its current replacement in the same pass"
    );
    assert!(!arena.entities[usize::from(tree_slot)].active);
    assert_eq!(
        terrain.planes().entity_indices[0] as u16,
        arena.economy.map_chains.tile_head(0)
    );
    assert!(
        alias
            .campaign_entity_presentations(0)
            .expect("same-pass water-hidden replacement projection")
            .iter()
            .all(|row| row.entity_index != tree_slot)
    );
    // The source water tail disabled this very address. The ordinary next
    // pass must reclaim it before action dispatch, rather than running the
    // current action on a hidden occupant.
    alias.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !alias.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::AuthoredModel6,
                ..
            } if *source_slot == tree_slot
        )),
        "hidden replacement must not receive an action visit"
    );
    assert!(!alias.entities()[usize::from(tree_slot)].active);
    assert!(
        alias
            .economy()
            .authored_model6()
            .iter()
            .all(|child| child.source_entity_index != tree_slot)
    );
    assert!(
        !alias
            .economy()
            .map_chains
            .entity_chain(0, 0)
            .contains(&tree_slot)
    );
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
    let nonlethal_children = nonlethal
        .economy()
        .authored_model6()
        .iter()
        .filter(|child| child.authored_index == nonlethal_before.source_entity_index)
        .count();
    assert!(
        crate::native_world::area_damage::apply(
            nonlethal.source_arena_mut(),
            None,
            None,
            crate::combat::area::AreaDamage {
                position: nonlethal_before.position,
                shape: crate::combat::area::Shape {
                    half_width: 64,
                    half_depth: 64,
                    vertical_offset: nonlethal_before.yaw,
                    half_height: nonlethal_before.fov,
                },
                owner: nonlethal_owner,
                payload: 0,
                selector: (nonlethal_before.source_class, nonlethal_before.model),
                mode: 0,
            },
        )
        .expect("shared nonlethal area damage visits Tree")
            > 0
    );
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
    assert_eq!(
        nonlethal
            .economy()
            .authored_model6()
            .iter()
            .filter(|child| child.authored_index == nonlethal_before.source_entity_index)
            .count(),
        nonlethal_children
    );
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
    let allocation_children = allocation_failure
        .economy()
        .authored_model6()
        .iter()
        .filter(|child| child.authored_index == allocation_before.source_entity_index)
        .count();
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
    // A source group termination clears the frozen replacement cursor. The
    // old economy-only exclusion no longer models this allocator path.
    allocation_failure.source_arena_mut().finish_source_group();
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
    assert_eq!(
        allocation_failure
            .economy()
            .authored_model6()
            .iter()
            .filter(|child| child.authored_index == allocation_before.source_entity_index)
            .count(),
        allocation_children
    );
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
fn level_three_imported_tree_damage_reaches_effect_and_following_actions() {
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE_LEVEL3_V1").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf14-campaign-package-v1-a/qualified-level3")
        },
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL3,
    )
    .expect("real Level 3 campaign package")
    .simulation()
    .clone();
    session.runtime.set_level2_is_level_end(1);
    let tree = session.entities()[95].clone();
    assert!(tree.active);
    assert_eq!((tree.class, tree.model, tree.action), (2, 0, 0));
    let owner = session.state.entity_index;
    let owner_id = session.entities()[usize::from(owner)].source_id;
    let request = crate::combat::area::AreaDamage {
        position: tree.position,
        shape: crate::combat::area::Shape {
            half_width: 64,
            half_depth: 64,
            vertical_offset: tree.array_yaw(),
            half_height: tree.array_fov(),
        },
        owner,
        payload: u16::try_from(tree.life + 1).expect("Tree life fits damage payload"),
        selector: (2, 0),
        mode: 0,
    };
    let mut nonlethal = session.clone();
    assert!(
        crate::native_world::area_damage::apply(
            nonlethal.source_arena_mut(),
            None,
            None,
            crate::combat::area::AreaDamage {
                payload: 0,
                ..request
            },
        )
        .expect("nonlethal source area visit")
            > 0
    );
    nonlethal.step_outer_turn(PlayerCommand::default(), 0);
    let unchanged = &nonlethal.entities()[95];
    assert_eq!(
        (unchanged.action, unchanged.life, unchanged.entity_rng),
        (0, tree.life, tree.entity_rng)
    );
    assert_eq!(&unchanged.record[102..104], &[0, 0]);

    assert!(
        crate::native_world::area_damage::apply(session.source_arena_mut(), None, None, request,)
            .expect("lethal source area visit")
            > 0
    );
    let old_children = session
        .entities()
        .iter()
        .filter(|row| row.active && (row.class, row.model) == (10, 6))
        .map(|row| row.index)
        .collect::<std::collections::BTreeSet<_>>();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let after = &session.entities()[95];
    assert_eq!((after.class, after.model, after.action), (2, 0, 1));
    assert_eq!(after.entity_rng, level2_next_rng16(tree.entity_rng));
    assert_eq!(&after.record[102..104], &[0, 0]);
    let child = session
        .entities()
        .iter()
        .find(|row| {
            row.active
                && (row.class, row.model) == (10, 6)
                && !old_children.contains(&row.index)
                && row.source_id == owner_id
                && (row.position.x, row.position.y) == (tree.position.x, tree.position.y)
        })
        .expect("actual imported Tree produces its source GroundFire child");
    assert_eq!(
        child.source_word_2c(),
        (i32::from(tree.array_fov()) * 3 / 4) as i16
    );
    assert!(
        session
            .campaign_entity_presentations(0)
            .expect("campaign sprite consumer")
            .iter()
            .any(|row| row.entity_index == child.index
                && row.render_parameter_index == child.particle_parameter_index.unwrap())
    );
    let life = after.life;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let continued = &session.entities()[95];
    assert_eq!(
        (
            continued.class,
            continued.model,
            continued.action,
            continued.life
        ),
        (2, 0, 1, life - 1)
    );
    let next_particle = match continued.particle_parameter_index {
        Some(83) => 226,
        Some(84) => 227,
        other => panic!("real imported Tree particle is unexpected: {other:?}"),
    };
    // Exercise the strict action-1 threshold on this same real Tree; the
    // normal scheduler, particle binding and sprite consumer own the result.
    session.source_arena_mut().entities[95].set_life(60);
    session.step_outer_turn(PlayerCommand::default(), 0);
    let changed = &session.entities()[95];
    assert_eq!(
        (
            changed.action,
            changed.life,
            changed.particle_parameter_index
        ),
        (2, 59, Some(next_particle))
    );
    let binding = session.session.definition.presentation.particles[usize::from(next_particle)];
    assert_eq!(changed.array_yaw(), (binding.rotation_speed / 2) as i16);
    assert_eq!(changed.array_pitch(), (binding.speed / 2) as i16);
    assert_eq!(changed.array_fov(), (binding.rotation_speed / 2) as i16);
    assert!(
        session
            .campaign_entity_presentations(0)
            .expect("changed Tree sprite projection")
            .iter()
            .any(|row| row.entity_index == 95 && row.render_parameter_index == next_particle)
    );
    let phase = changed.phase;
    session.step_outer_turn(PlayerCommand::default(), 0);
    let settled = &session.entities()[95];
    assert_eq!((settled.action, settled.life), (2, 59));
    assert_eq!(settled.phase, phase.wrapping_add(1));
}

#[test]
fn level_seven_imported_tree_self_alias_runs_current_effect_on_land() {
    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level7");
    let mut session = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(7).unwrap(),
    )
    .expect("real Level 7 campaign package")
    .simulation()
    .clone();
    session.runtime.set_level2_is_level_end(1);
    let tree = session.entities()[108].clone();
    assert!(tree.active);
    assert_eq!((tree.class, tree.model, tree.action), (2, 0, 0));
    let owner = session.state.entity_index;
    assert!(
        crate::native_world::area_damage::apply(
            session.source_arena_mut(),
            None,
            None,
            crate::combat::area::AreaDamage {
                position: tree.position,
                shape: crate::combat::area::Shape {
                    half_width: 64,
                    half_depth: 64,
                    vertical_offset: tree.array_yaw(),
                    half_height: tree.array_fov(),
                },
                owner,
                payload: u16::try_from(tree.life + 1).unwrap(),
                selector: (2, 0),
                mode: 0,
            },
        )
        .expect("actual source area damage")
            > 0
    );

    // Fill the existing shared pool with an actual Level 7 authored class14
    // constructor, then consume the earlier frozen replacement addresses.
    while session.economy().free_slot_count() > 0 {
        session
            .allocate_level2_authored_class14(427)
            .expect("real Level 7 class14 constructor fills a free slot");
    }
    let particles = &session.session.definition.presentation.particles;
    let live = &mut session.session.state.live;
    let arena = &mut live.economy;
    let terrain = &mut live.terrain;
    assert_ne!(
        crate::FlightWorld::material_mask(terrain, Position::default()),
        crate::WATER_AMBIENCE_MASK
    );
    arena.rebuild_free_arena(&[]);
    let dynamic = arena
        .economy
        .live_arena_slots()
        .into_iter()
        .map(|(slot, _)| slot)
        .collect::<std::collections::BTreeSet<_>>();
    let mut candidates = arena
        .entities
        .iter()
        .filter(|row| {
            row.index != 0
                && row.active
                && row.class != 0
                && !dynamic.contains(&row.index)
                && row.control_flags & SOURCE_REPLACEABLE_FLAG != 0
        })
        .map(|row| row.index)
        .collect::<Vec<_>>();
    arena
        .economy
        .for_each_live_replaceable_arena_row(|slot, _| candidates.push(slot));
    candidates.sort_unstable();
    candidates.dedup();
    assert!(candidates.contains(&tree.index));
    for expected in candidates.into_iter().take_while(|slot| *slot < tree.index) {
        let slot = arena
            .allocate_source_slot_with_terrain(terrain)
            .expect("frozen replacement");
        assert_eq!(slot, expected);
        let sphere = arena
            .economy
            .initialize_authored_model39(slot, tree.index, Position::default())
            .expect("source sphere constructor");
        crate::world::effects::sync_position(arena, terrain, slot, sphere.position, true, false)
            .unwrap();
    }
    let generation = arena.economy.slot_generation(tree.index);
    let expected_rng = level2_next_rng16(arena.economy.new_event_rng(tree.index));
    crate::world::scenery::trees::step_imported(
        arena,
        terrain,
        Default::default(),
        particles,
        tree.index,
    )
    .expect("real imported Tree callback at its selected address");
    let child = &arena.entities[usize::from(tree.index)];
    assert_eq!(
        arena.economy.slot_generation(tree.index),
        generation.wrapping_add(1)
    );
    assert_eq!((child.class, child.model, child.action), (10, 6, 1));
    assert_eq!((child.position.x, child.position.y), (0, 0));
    assert_eq!(child.entity_rng, expected_rng);
    assert_eq!(child.life, i32::from(expected_rng % 60) + 130);
    assert_eq!(child.source_word_2c(), 1152);
    assert_eq!(
        child.control_flags & crate::level1_mission::SOURCE_DISABLED_FLAG,
        0
    );
    let life = child.life;
    let phase = child.phase;
    assert!(
        session
            .campaign_entity_presentations(0)
            .expect("land replacement sprite projection")
            .iter()
            .any(|row| row.entity_index == tree.index && row.render_parameter_index == 228)
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    let continued = &session.entities()[usize::from(tree.index)];
    assert_eq!(
        (continued.class, continued.model, continued.action),
        (10, 6, 1),
        "dispatch events {:?}",
        session.dispatch_events()
    );
    assert_eq!(continued.life, life - 1);
    assert_eq!(continued.phase, phase.wrapping_add(1));
    assert_ne!(continued.control_flags & 2, 0);
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
            // pass. NewAdd0A00 retains AddEventToMap's bit0x04; model53 adds
            // 0x10080 to its constructor's0x20000, then action0 adds0x02.
            (row439.entity_id, 0, 0, 400, 8, 7, 0x0003_0086, 512, 512, 0),
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

    let source_slot = row439.source_entity_index;
    let handle = session.source_arena().handle(source_slot).unwrap();
    // Finish the actual constructor's remaining life, including one visit
    // whose old life is negative. Expiry neither hides nor reclaims model53.
    for _ in 0..row439.life + 2 {
        session.step_level2_source_pass(0);
    }
    let expired = *session.economy().authored_model53().iter()
        .find(|source| source.source_entity_index == source_slot).unwrap();
    assert_eq!(expired.life, -2);
    assert!(expired.active && expired.drawing_enabled);
    assert_eq!(session.source_arena().handle(source_slot), Some(handle));

    // Declared arithmetic boundary on that same ordinarily produced carrier:
    // the old i16 counter wrapped positive here and resumed damage/spawning.
    session.economy_mut().authored_model53.iter_mut()
        .find(|source| source.source_entity_index == source_slot).unwrap().life = -32768;
    let prior_children = session.economy().model17_spawned_children().iter()
        .filter(|child| child.active && child.parent_entity_index == source_slot)
        .map(|child| (child.source_entity_index,
            session.economy().slot_generation(child.source_entity_index)))
        .collect::<BTreeSet<_>>();
    for visit in 1..=2 {
        session.step_level2_source_pass(0);
        assert!(session.dispatch_events().iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })));
        let current = session.economy().authored_model53().iter()
            .find(|source| source.source_entity_index == source_slot).unwrap();
        assert_eq!(current.life, -32768 - visit);
        assert_eq!(current.phase, expired.phase.wrapping_add(visit as u8));
        assert_eq!(current.entity_rng, expired.entity_rng);
        assert_eq!(current.dword_0x10, expired.dword_0x10);
        assert!(current.active && current.drawing_enabled);
        assert_eq!(session.source_arena().handle(source_slot), Some(handle));
        let view = crate::native_world::source_target::read(
            session.source_arena(),
            crate::native_world::source_target::Context::default(),
            source_slot,
        ).unwrap().unwrap();
        assert_eq!(view.life, current.life, "current packet retains the full source counter");
        assert_eq!((view.active, view.class, view.model), (true, 10, 53));
        assert!(session.economy().model17_spawned_children().iter()
            .filter(|child| child.active && child.parent_entity_index == source_slot)
            .all(|child| prior_children.contains(&(child.source_entity_index,
                session.economy().slot_generation(child.source_entity_index)))),
            "a terminal model53 visit must not construct another child");
    }

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

#[test]
fn ymbul_source_groups_earned_mana_and_live_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::GameSession;
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_SECRET31_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("mc-paj2-source-secret-31-campaign-package"));
    let cache = std::env::var_os("MC_SECRET31_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/source31"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2231,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    // Declared incoming carry for this standalone revisit.
    game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
        .apply_carried_spell_families(&[1], Some(1)).unwrap();
    assert_eq!(complete_ymbul_source31_for_test(&mut game), 0x0a);
}

/// Complete the real source31 mission for the caller's declared arriving
/// secret session, returning its actual ordinary or retained-parent exit byte.
pub(crate) fn complete_ymbul_source31_for_test(
    mut game: &mut crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>,
) -> u8 {
    use crate::campaign::backend::Mc2CampaignBackend;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
    use crate::FlightWorld;
    use std::collections::BTreeSet;

    type Game = GameSession<Mc2CampaignBackend>;
    #[derive(Default)]
    struct Receipts {
        group1: usize,
        group2: usize,
        group3: usize,
        group6: usize,
        wave: Vec<SourceEntityHandle>,
    }
    fn sim(game: &Game) -> &Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut Mc2Simulation {
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
    }
    fn advance(game: &mut Game, command: GameCommand, receipts: &mut Receipts) {
        game.advance(game.identity().guard(), command).expect("Ymbul source advance");
        let state = sim(game);
        for event in state.dispatch_events() {
            match event {
                Mc2DispatchEvent::Error { .. } => panic!("Ymbul source dispatch: {event:?}"),
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 1, authored_indices, .. }
                    if (162..=170).contains(source_slot) && !authored_indices.is_empty() => {
                    let expected = state.mission().authored_features().iter()
                        .filter(|row| row.index != 0 && row.kind != 0 && row.activation_group == 1)
                        .map(|row| row.index).collect::<Vec<_>>();
                    assert_eq!(expected.len(), 43);
                    assert_eq!(authored_indices.as_ref(), expected);
                    receipts.group1 += 1;
                    for kind in [CreatureKind::Skeleton, CreatureKind::Archer] {
                        receipts.wave.extend(state.creature_runtime_census().roots_of_kind(kind)
                            .filter_map(|root| match root.origin {
                                CreatureRootOrigin::AuthoredRecord { authored_record }
                                    if expected.contains(&authored_record) => Some(root.handle),
                                _ => None,
                            }));
                    }
                    assert_eq!(receipts.wave.len(), 41, "all actual group1 creature generations allocate");
                }
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 2, authored_indices, .. } => {
                    let switch = state.economy().authored_switch_producers().iter()
                        .find(|row| row.authored_index == 630).expect("actual group1 Skeleton absence switch");
                    assert_eq!(*source_slot, switch.source_entity_index);
                    assert_eq!(authored_indices.as_ref(), [631, 632]);
                    receipts.group2 += 1;
                }
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 3, authored_indices, .. } => {
                    let switch = state.economy().authored_switch_producers().iter()
                        .find(|row| row.authored_index == 633).expect("actual group1 Archer absence switch");
                    assert_eq!(*source_slot, switch.source_entity_index);
                    assert_eq!(authored_indices.as_ref(), [634, 635, 636, 637]);
                    receipts.group3 += 1;
                }
                Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 116, group: 6, authored_indices, .. } => {
                    assert!(authored_indices.contains(&455) && authored_indices.contains(&511));
                    receipts.group6 += 1;
                }
                _ => {}
            }
        }
    }
    fn percent(game: &Game, player: u16) -> i32 {
        let economy = sim(game).economy();
        let total = economy.owner_mana_global();
        assert!(total > 0);
        economy.castle_for_owner(player).map_or(0, |castle|
            100 * economy.owner_mana_aggregate(player).saturating_add(castle.mana) / total)
    }
    let goals = game.definition().level_definition().unwrap().objectives();
    assert_eq!(goals.len(), 1);
    assert!(matches!(goals[0].condition(), ObjectiveCondition::CastleManaPercent(30)));
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    let spawn = sim(&game).state.position;
    let gate = &sim(&game).entities()[116];
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 4, 4, 6));
    let gate_handle = sim(&game).source_arena().handle(116).unwrap();
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1);
    // The caller declares incoming Possession; its holder pays for the cast.
    assert_ne!(sim(&game).shared_spells().local_player().enabled_spell_holders[1], 0);
    let mut receipts = Receipts::default();
    advance(&mut game, GameCommand {
        select_spell: Some(crate::game_definition::SemanticId::parse("spell.possession").unwrap()),
        ..GameCommand::default()
    }, &mut receipts);
    let initial_transports = sim(&game).entities().iter()
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 34, 36))
        .filter_map(|row| sim(&game).source_arena().handle(row.index))
        .map(|h| (h.source_slot, h.generation)).collect::<BTreeSet<_>>();
    // Enter one actual reset area; all duplicate switches retain source scheduling.
    let trigger = &sim(&game).entities()[162];
    assert_eq!((trigger.class, trigger.model, trigger.action, trigger.source_id), (11, 0, 0, 1));
    let mut position = trigger.position;
    position.z = sim(&game).canonical_terrain().ground_height(position)
        .wrapping_add(sim(&game).profile().ground_clearance);
    sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
    for _ in 0..64 {
        advance(&mut game, GameCommand::default(), &mut receipts);
        if receipts.group1 > 0 { break; }
    }
    assert_eq!(receipts.group1, 1, "the real area producer must activate its complete group");
    sim_mut(&mut game).set_level2_entity_position(player, spawn).unwrap();
    let survivor9 = sim(&game).source_arena().rebuilt_class5_model_slots()[9][0];
    let survivor4 = sim(&game).source_arena().rebuilt_class5_model_slots()[4][0];
    let survivor9 = sim(&game).source_arena().handle(survivor9).unwrap();
    let survivor4 = sim(&game).source_arena().handle(survivor4).unwrap();
    // Witness each actual absence callback with a live family member in the
    // source prepass. A later natural enemy hit may kill that member; neither
    // its health nor AI is held for the rest of the encounter.
    let mut blocked = [false; 2];
    for _ in 0..32 {
        let slots = sim(&game).source_arena().rebuilt_class5_model_slots();
        let before = [630_u16, 633].map(|authored| *sim(&game).economy()
            .authored_switch_producers().iter().find(|row| row.active && row.authored_index == authored)
            .expect("real group1 absence switch"));
        let alive = [slots[9].contains(&survivor9.source_slot), slots[4].contains(&survivor4.source_slot)];
        advance(&mut game, GameCommand::default(), &mut receipts);
        let state = sim(&game);
        for index in 0..2 {
            let survivor = [survivor9, survivor4][index];
            let model = [9_usize, 4][index];
            let present_after = state.source_arena().handle(survivor.source_slot) == Some(survivor)
                && state.source_arena().rebuilt_class5_model_slots()[model].contains(&survivor.source_slot);
            let callback = state.cf10_production_schedule_receipts().into_iter().any(|receipt| matches!(receipt,
                crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                    if slot == before[index].source_entity_index));
            let after = state.economy().authored_switch_producers().iter()
                .find(|row| row.source_entity_index == before[index].source_entity_index);
            if alive[index] && present_after && callback {
                assert!(after.is_some_and(|row| row.active && !row.fired
                    && row.dword_0x10 == before[index].dword_0x10),
                    "the actual absence callback must freeze its timer while the selected family lives");
                blocked[index] = true;
            }
        }
        if blocked == [true, true] { break; }
    }
    assert_eq!(blocked, [true, true], "both living-family negative callbacks must be observed");
    assert_eq!((receipts.group2, receipts.group3), (0, 0));
    let mut struck = BTreeSet::new();
    // Arrived contacts are explicit controls on actual current generations. All
    // deaths, source absence countdowns, later tails and reclaims stay scheduled.
    for _ in 0..2048 {
        let model_slots = sim(&game).source_arena().rebuilt_class5_model_slots();
        let roots = [9_usize, 4].into_iter().flat_map(|model|
            model_slots[model].iter().copied()
        ).filter_map(|slot| sim(&game).source_arena().handle(slot)).collect::<Vec<_>>();
        for handle in roots {
            if struck.insert((handle.source_slot, handle.generation)) {
                area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
            }
        }
        advance(&mut game, GameCommand::default(), &mut receipts);
        if receipts.group2 > 0 && receipts.group3 > 0
            && receipts.wave.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
    }
    assert_eq!((receipts.group2, receipts.group3), (1, 1), "both source absence chains publish");
    assert!(receipts.wave.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()),
        "all produced creature generations naturally reclaim");
    let transports = sim(&game).entities().iter()
        .filter(|row| row.active && (row.class, row.model, row.action) == (10, 34, 36))
        .filter_map(|row| sim(&game).source_arena().handle(row.index).map(|h| (h, row.position)))
        .filter(|(h, _)| !initial_transports.contains(&(h.source_slot, h.generation)))
        .collect::<Vec<_>>();
    assert_eq!(transports.len(), 6, "the two absence groups create six live source transports");
    assert!(sim(&game).canonical_player_life_for_test() >= 0);
    // Enter one source-created transport with the same forward-facing contact
    // used by the real field consumer; the destination and schedule are not
    // staged, and the actual player carrier must move to its authored target.
    let (transport, position) = transports[0];
    let source = &sim(&game).entities()[usize::from(transport.source_slot)];
    let destination = crate::world::fields::transport::destination(source);
    let contact = Position { y: position.y.wrapping_add(1), ..position };
    let heading = crate::heading_for_positions(contact, position) as i16;
    let auxiliary = sim(&game).shared_spells().beyond_sight_auxiliary();
    let yaw = heading.wrapping_add(auxiliary.next_entity);
    let pitch = auxiliary.entity_index2;
    let state = sim_mut(&mut game);
    state.set_level2_entity_position(player, contact).unwrap();
    state.state.actual_speed = 0;
    state.state.target_speed = 0;
    state.set_level2_player_aim_for_test(yaw, pitch);
    advance(&mut game, GameCommand::default(), &mut receipts);
    assert_eq!(sim(&game).source_arena().handle(transport.source_slot), Some(transport));
    let actual = sim(&game).state.position;
    let near = |a: u16, b: u16| i32::from(a.wrapping_sub(b) as i16).abs() < 256;
    assert!(near(actual.x, destination.x) && near(actual.y, destination.y),
        "the actual group-created transport must move the player: source={transport:?}, contact={contact:?}, destination={destination:?}, actual={actual:?}");
    advance(&mut game, GameCommand::default(), &mut receipts); // publish the last actual wave census

    // Select an actual settled neutral source sphere. No sphere or collector is moved.
    let sphere = sim(&game).economy().spheres().iter().find(|row|
        row.active && !row.pending_delete && row.player_entity_index == 0 && row.control_flags & 8 != 0
            && row.control_flags & 0x0020_0000 == 0 && row.vertical_velocity == 0 && row.mana > 0)
        .copied().expect("Ymbul retains an actual settled neutral mana sphere");
    let sphere_handle = sim(&game).source_arena().handle(sphere.source_entity_index).unwrap();
    // Declared prior Castle/bank isolates the final increment, strictly below
    // 30%; the bank is not claimed as earned campaign progress.
    let castle = {
        let state = sim_mut(&mut game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state.economy_mut().set_castle_stage_footprint(stage, footprint);
        }
        let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position { x: x * 2048, y: y * 2048, z: 0 }))
            .map(|mut p| { p.z = state.canonical_terrain().ground_height(p); p })
            .filter(|p| {
                let dx = i32::from(p.x.wrapping_sub(sphere.position.x) as i16).abs();
                let dy = i32::from(p.y.wrapping_sub(sphere.position.y) as i16).abs();
                dx.max(dy) >= 4096 && !state.economy().castle_projectile_site_blocked(*p, state.entities(), state.canonical_terrain())
            }).min_by_key(|p| crate::distance_3d_wrapped(*p, sphere.position))
            .expect("terrain admits the prior Castle outside sphere overlap");
        state.economy_mut().build_castle(site, player).unwrap()
    };
    advance(&mut game, GameCommand::default(), &mut receipts);
    assert_eq!(sim(&game).source_arena().handle(sphere_handle.source_slot), Some(sphere_handle));
    assert!(sim(&game).economy().spheres().iter().any(|row| row.id == sphere.id && row.active
        && !row.pending_delete && row.player_entity_index == 0 && row.mana == sphere.mana));
    let total = i64::from(sim(&game).economy().owner_mana_global());
    let aggregate = i64::from(sim(&game).economy().owner_mana_aggregate(player));
    let numerator = total.checked_mul(30).and_then(|n| aggregate.checked_mul(100).and_then(|a| n.checked_sub(a)))
        .expect("source 30-percent census arithmetic").max(0);
    let threshold = (numerator + 69) / 70;
    let bank = i32::try_from(threshold - i64::from(sphere.mana)).expect("prior bank fits i32");
    assert!(bank >= 0 && sphere.mana > 0);
    let capacity = i32::try_from(aggregate + i64::from(bank) + i64::from(sphere.mana)).unwrap();
    assert!(capacity <= i32::MAX / 100);
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, capacity));
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, bank));
    let castle_handle = sim(&game).source_arena().handle(castle).unwrap();
    advance(&mut game, GameCommand::default(), &mut receipts);
    assert_eq!(i64::from(sim(&game).economy().owner_mana_global()), total + i64::from(bank));
    assert!(percent(&game, player) < 30);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1);
    assert_eq!(receipts.group6, 0, "below-threshold source mana blocks the real exit gate");

    let target = sim(&game).possession_guidance_target_top_for_test(sphere.source_entity_index).unwrap();
    let state = sim_mut(&mut game);
    let fov = state.shared_spells().player_entity().array_fov();
    let mut hand = Position { x: target.x.wrapping_add(2048), y: target.y, z: target.z.checked_add(512).unwrap() };
    let yaw = crate::level1_mission::projectile_guidance_angles(hand, target).0;
    let hand_yaw = yaw.wrapping_sub(state.shared_spells().beyond_sight_auxiliary().next_entity).wrapping_sub(512);
    let mut origin = Position { z: hand.z.checked_sub(fov).unwrap(), ..hand };
    crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
    origin.z = origin.z.max(state.canonical_terrain().ground_height(origin).wrapping_add(state.profile().ground_clearance))
        .max(state.canonical_terrain().ground_height(hand).wrapping_add(crate::level1_mission::FIREBALL_COLLISION_HALF_HEIGHT).wrapping_sub(fov));
    hand.z = origin.z.wrapping_add(fov);
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(hand, target);
    state.set_level2_entity_position(player, origin).unwrap();
    state.state.actual_speed = 0;
    state.state.target_speed = 0;
    state.set_level2_player_aim_for_test(yaw, pitch);
    let holder = sim(&game).shared_spells().local_player().enabled_spell_holders[1];
    let holder_handle = sim(&game).source_arena().handle(holder).unwrap();
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    let cost = sim(&game).shared_spells().pickup(holder).unwrap().max_mana;
    assert!(cost > 0 && sim(&game).shared_spells().player_entity().mana >= cost);
    let mut cursor = sim(&game).economy().events().len();
    let (mut paid, mut debited, mut claimed, mut retreated, mut credited) = (false, false, false, false, false);
    let mut collected = None;
    let castle_position = sim(&game).economy().castle_at(castle).unwrap().position;
    let bound = 32 + 2 * (crate::distance_3d_wrapped(sphere.position, castle_position) as usize).div_ceil(48) + 256;
    for turn in 0..bound {
        if claimed && !retreated {
            assert!(sim(&game).canonical_player_life_for_test() >= 0);
            let mut position = spawn;
            let terrain = sim(&game).canonical_terrain();
            position.z = position.z.max(terrain.ground_height(position).wrapping_add(sim(&game).profile().ground_clearance));
            if let Some(ceiling) = terrain.ceiling_height(position) {
                position.z = position.z.min(ceiling.wrapping_sub(sim(&game).profile().ceiling_clearance));
            }
            sim_mut(&mut game).set_level2_entity_position(player, position).unwrap();
            retreated = true;
        }
        advance(&mut game, GameCommand { cast_selected: turn == 0, ..GameCommand::default() }, &mut receipts);
        let state = sim(&game);
        debited |= state.cf10_production_schedule_receipts().into_iter().any(|receipt| match receipt {
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
            } if slot == player => original.observed_player_resources().zip(selector.observed_player_resources())
                .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                    && before.2 <= -cost && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1.saturating_sub(after.1) >= cost),
            _ => false,
        });
        if turn == 0 {
            let launched = state.economy().possession_projectiles().iter().any(|row|
                row.source_id_entity_index == player && row.holder_entity_index == holder_handle.source_slot
                    && state.source_arena().handle(row.source_entity_index).is_some());
            let charged = state.cf10_production_schedule_receipts().into_iter().any(|receipt| match receipt {
                crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                    slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
                } if slot == holder => original.observed_player_resources().zip(selector.observed_player_resources())
                    .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                        && before.1 >= cost && after.2 == if before.2 >= 0 { -cost } else { before.2.wrapping_sub(cost) }),
                _ => false,
            });
            paid = launched && charged;
            assert!(paid, "public Possession must launch and queue its actual cost");
        }
        assert_eq!(state.source_arena().handle(castle), Some(castle_handle));
        for event in &state.economy().events()[cursor..] {
            match *event {
                Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                    if sphere_id == sphere.id && owner_entity_index == player => claimed = true,
                Level2EconomyEvent::CastleDefenderCollectedSphere { defender_slot, sphere_id, mana }
                    if sphere_id == sphere.id => {
                        assert!(claimed && mana == sphere.mana);
                        collected = Some((state.source_arena().handle(defender_slot).unwrap(), mana));
                    }
                Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
                    if castle_id == castle && collected.is_some_and(|(h, payload)| h.source_slot == defender_slot && payload == mana) => {
                        assert_eq!(state.source_arena().handle(defender_slot), Some(collected.unwrap().0));
                        assert!(percent(&game, player) >= 30,
                            "actual collector credit earns 30%: turn={turn}, total={total}, aggregate={aggregate}, bank={bank}, sphere={sphere:?}");
                        credited = true;
                    }
                _ => {}
            }
        }
        cursor = state.economy().events().len();
        if !credited {
            assert!(percent(&game, player) < 30);
            assert_eq!(state.runtime().objectives[owner].stage_selectors[0], 1);
        }
        if credited && receipts.group6 > 0 { break; }
    }
    assert!(paid && debited && claimed && retreated && collected.is_some() && credited && receipts.group6 == 1,
        "paid={paid}/{debited}, claim={claimed}, collected={collected:?}, credit={credited}, group6={}, bank={bank}, total={total}, sphere={sphere:?}", receipts.group6);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    for _ in 0..8 {
        if sim(&game).source_arena().handle(116) != Some(gate_handle) { break; }
        advance(&mut game, GameCommand::default(), &mut receipts);
    }
    assert_ne!(sim(&game).source_arena().handle(116), Some(gate_handle));
    let exit = *sim(&game).economy().authored_switch_producers().iter()
        .find(|row| row.active && row.authored_index == 455 && row.model == 12 && row.action == 12)
        .expect("actual group6 exit455");
    assert!(sim(&game).economy().authored_class14().iter().any(|row| row.active && row.authored_index == 511 && row.model == 3));
    assert!(sim(&game).canonical_player_life_for_test() >= 0, "a living player must enter the real exit");
    sim_mut(&mut game).set_level2_entity_position(player, exit.position).unwrap();
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        advance(&mut game, GameCommand::default(), &mut receipts);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    sim(&game).source_exit_result_byte()
}

#[test]
fn pav_durivium_cave_groups_earned_mana_and_live_exit() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::GameSession;
    let generated = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated");
    let package = std::env::var_os("MC_SECRET32_PACKAGE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("mc-paj2-source-secret-32-campaign-package"));
    let cache = std::env::var_os("MC_SECRET32_OWNER_CACHE").map(PathBuf::from)
        .unwrap_or_else(|| generated.join("campaign-original-environments/source32"));
    let definition = load_mc2_campaign(&package, &crate::game_definition::default_catalog().unwrap()).unwrap();
    let mut game = GameSession::open(&Mc2CampaignAdapter, std::sync::Arc::new(definition), 2232,
        Mc2CampaignInputs::with_owner_cache(&package, &cache)).unwrap();
    // Declared incoming carry for this standalone revisit.
    game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
        .apply_carried_spell_families(&[1], Some(1)).unwrap();
    assert_eq!(complete_pav_source32_for_test(&mut game), 0x0a);
}

/// Complete the real source32 mission for the caller's declared arriving
/// secret session, returning its actual ordinary or retained-parent exit byte.
pub(crate) fn complete_pav_source32_for_test(
    mut game: &mut crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>,
) -> u8 {
    use crate::campaign::backend::Mc2CampaignBackend;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::game_session::{GameCommand, GameCompletionStatus, GameSession};
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
    use crate::FlightWorld;
    use std::collections::{BTreeMap, BTreeSet};

    type Game = GameSession<Mc2CampaignBackend>;
    #[derive(Default)]
    struct Receipts {
        groups: BTreeMap<u16, usize>,
        wave: Vec<SourceEntityHandle>,
        close: Option<SourceEntityHandle>,
        fire_sphere: Option<SourceEntityHandle>,
        sphere_births: BTreeSet<(u32, u32)>,
        event_cursor: usize,
    }
    fn sim(game: &Game) -> &Mc2Simulation {
        game.backend().compatibility_campaign().simulation()
    }
    fn sim_mut(game: &mut Game) -> &mut Mc2Simulation {
        game.backend_mut().compatibility_campaign_mut_for_test().simulation_mut()
    }
    fn step(game: &mut Game, command: GameCommand, receipts: &mut Receipts) {
        let model76_before = sim(game).economy().authored_model76().iter()
            .filter_map(|row| sim(game).source_arena().handle(row.source_entity_index))
            .collect::<Vec<_>>();
        game.advance(game.identity().guard(), command).expect("Pav source advance");
        let state = sim(game);
        for event in state.dispatch_events() {
            match event {
                Mc2DispatchEvent::Error { .. } => panic!("Pav source dispatch: {event:?}"),
                Mc2DispatchEvent::AuthoredGroupActivated { group, authored_indices, .. }
                    if matches!(*group, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 9) && !authored_indices.is_empty() => {
                    let expected = state.mission().authored_features().iter()
                        .filter(|row| row.index != 0 && row.kind != 0 && row.activation_group == *group as i16)
                        .map(|row| row.index).collect::<Vec<_>>();
                    assert_eq!(authored_indices.as_ref(), expected, "real source group {group}");
                    *receipts.groups.entry(*group).or_default() += 1;
                    if *group == 6 {
                        let roots = state.creature_runtime_census().roots_of_kind(CreatureKind::Cymmerian)
                            .filter_map(|root| match root.origin {
                                CreatureRootOrigin::AuthoredRecord { authored_record }
                                    if expected.contains(&authored_record) => Some(root.handle),
                                _ => None,
                            }).collect::<Vec<_>>();
                        assert_eq!(roots.len(), 8, "all eight source group6 roots allocate");
                        receipts.wave.extend(roots);
                    }
                    if *group == 3 {
                        let source = state.economy().terrain_strip_close_triggers().iter()
                            .find(|row| row.authored_index == 346)
                            .expect("group3 constructs the actual terrain command");
                        receipts.close = state.source_arena().handle(source.source_entity_index);
                    }
                    if *group == 9 {
                        // The generic source constructor has no authored-index
                        // argument. Bind the unique new generation by this
                        // exact group's feature420 position and parent chain.
                        let feature = state.mission().authored_features().iter()
                            .find(|row| row.index == 420).unwrap();
                        let position = feature.world_position(state.canonical_terrain()
                            .ground_height(feature.world_position(0)));
                        let roots = state.economy().authored_model76().iter()
                            .filter(|row| row.active && row.position == position)
                            .filter_map(|row| state.source_arena().handle(row.source_entity_index)
                                .filter(|handle| !model76_before.contains(handle)).map(|handle| (*row, handle)))
                            .collect::<Vec<_>>();
                        assert_eq!(roots.len(), 1, "group9 must allocate exactly one new model76 generation at feature420");
                        let (root, handle) = roots[0];
                        assert_eq!(state.economy().model76_children().iter().filter(|child|
                            child.active && child.parent_entity_index == handle.source_slot
                                && child.parent_entity_id == root.entity_id).count(), 25,
                            "the actual root owns the complete source child chain");
                        receipts.fire_sphere = Some(handle);
                    }
                }
                _ => {}
            }
        }
        for event in &state.economy().events()[receipts.event_cursor..] {
            if let Level2EconomyEvent::ManaSphereSpawned { sphere_id, source_entity_index: 310 | 317, mana } = *event
                && mana > 0
            {
                let slot = u16::try_from(sphere_id).expect("authored sphere is a source slot");
                receipts.sphere_births.insert((sphere_id, state.economy().slot_generation(slot)));
            }
        }
        receipts.event_cursor = state.economy().events().len();
    }
    fn count(receipts: &Receipts, group: u16) -> usize {
        receipts.groups.get(&group).copied().unwrap_or(0)
    }
    fn percent(game: &Game, player: u16) -> i32 {
        let economy = sim(game).economy();
        let total = economy.owner_mana_global();
        assert!(total > 0);
        economy.castle_for_owner(player).map_or(0, |castle|
            100 * economy.owner_mana_aggregate(player).saturating_add(castle.mana) / total)
    }
    // Player-only geometry control: select an actual open cave position inside
    // the source switch overlap. The switch, cadence and source group stay real.
    fn place_at_switch(game: &mut Game, player: u16, slot: u16) {
        let state = sim(game);
        let switch = &state.entities()[usize::from(slot)];
        let body = &state.entities()[usize::from(player)];
        let extent = |row: &crate::native_world::EntitySnapshot, offset: usize|
            i16::from_le_bytes(row.record_bytes()[offset..offset + 2].try_into().unwrap());
        let offsets = [0_i16, -128, 128, -256, 256, -512, 512, -768, 768, -1024, 1024, -1536, 1536, -2048, 2048];
        let position = offsets.into_iter().flat_map(|dy| offsets.into_iter().map(move |dx| (dx, dy)))
            .filter_map(|(dx, dy)| {
                let mut p = Position { x: switch.position.x.wrapping_add_signed(dx),
                    y: switch.position.y.wrapping_add_signed(dy), z: 0 };
                if !crate::level1_mission::source_entities_overlap_xy(switch.position,
                    extent(switch, 88), extent(switch, 90), p, extent(body, 88), extent(body, 90)) {
                    return None;
                }
                let floor = state.canonical_terrain().ground_height(p);
                let ceiling = state.canonical_terrain().ceiling_height(p)?;
                let low = floor.checked_add(state.profile().ground_clearance)?;
                let high = ceiling.checked_sub(state.profile().ceiling_clearance)?;
                if low > high { return None; }
                p.z = ((i32::from(low) + i32::from(high)) / 2) as i16;
                (!state.canonical_terrain().cave_collision_blocked(p)).then_some(p)
            }).next().unwrap_or_else(|| panic!("actual switch {slot} has no valid reachable cave overlap"));
        sim_mut(game).set_level2_entity_position(player, position).unwrap();
    }
    fn trigger(game: &mut Game, player: u16, slot: u16, group: u16, receipts: &mut Receipts) {
        let source = &sim(game).entities()[usize::from(slot)];
        assert_eq!((source.class, source.source_id), (11, group));
        let before = count(receipts, group);
        place_at_switch(game, player, slot);
        for _ in 0..64 {
            step(game, GameCommand::default(), receipts);
            if count(receipts, group) > before { break; }
        }
        assert!(count(receipts, group) > before, "actual switch {slot} must publish group {group}");
        assert!(sim(game).canonical_player_life_for_test() >= 0);
    }

    let goals = game.definition().level_definition().unwrap().objectives();
    assert_eq!(goals.len(), 1);
    assert!(matches!(goals[0].condition(), ObjectiveCondition::CastleManaPercent(40)));
    let player = sim(&game).state.entity_index;
    let owner = usize::from(sim(&game).state.player_color_index);
    let spawn = sim(&game).state.position;
    let gate = &sim(&game).entities()[350];
    assert_eq!((gate.class, gate.model, gate.action, gate.source_id), (11, 4, 4, 7));
    let gate_handle = sim(&game).source_arena().handle(350).unwrap();
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1);
    assert_ne!(sim(&game).shared_spells().local_player().enabled_spell_holders[1], 0,
        "the caller declares incoming Possession");
    let mut receipts = Receipts::default();
    step(&mut game, GameCommand {
        select_spell: Some(crate::game_definition::SemanticId::parse("spell.possession").unwrap()),
        ..GameCommand::default()
    }, &mut receipts);
    assert_eq!(count(&receipts, 7), 0, "below-goal source gate stays closed");

    // Actual group6 contains eight large source25 roots. Their normal split,
    // vulnerable landing, death and following-pass reclaims remain scheduled.
    trigger(&mut game, player, 347, 6, &mut receipts);
    sim_mut(&mut game).set_level2_entity_position(player, spawn).unwrap();
    let mut seen = receipts.wave.clone();
    let mut struck = BTreeSet::new();
    let mut split = false;
    for _ in 0..1024 {
        split |= sim(&game).creature_runtime_census().roots_of_kind(CreatureKind::Cymmerian)
            .any(|row| matches!(row.origin, CreatureRootOrigin::CreatureConversion { .. }));
        let targets = sim(&game).entities().iter().filter(|row| row.active && row.class == 5 && row.model == 25)
            .map(|row| (sim(&game).source_arena().handle(row.index).unwrap(),
                row.life >= 0 && (row.action != 200 || row.record_bytes()[74] >= 3))).collect::<Vec<_>>();
        for (handle, vulnerable) in targets {
            if !seen.contains(&handle) { seen.push(handle); }
            if vulnerable && struck.insert((handle.source_slot, handle.generation)) {
                area_damage::contact(sim_mut(&mut game).source_arena_mut(), None, handle.source_slot, u16::MAX, player).unwrap();
            }
        }
        step(&mut game, GameCommand::default(), &mut receipts);
        if sim(&game).source_arena().rebuilt_class5_model_slots()[25].is_empty()
            && seen.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
    }
    assert!(split && seen.len() > receipts.wave.len(), "source25 actual split children must be observed");
    assert!(sim(&game).source_arena().rebuilt_class5_model_slots()[25].is_empty());
    assert!(seen.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()), "all source25 generations reclaim");
    assert!(sim(&game).canonical_player_life_for_test() >= 0);

    // Repeating area switch creates the real opening command and exit switch.
    // Their controller is the actual class14 row in the authored command cell.
    let corridor = sim(&game).entities().iter().find(|row| row.active && row.class == 14
        && matches!(row.model, 1 | 2) && row.position.x >> 8 == 178 && row.position.y >> 8 == 194)
        .map(|row| row.index).expect("the authored group3 cave controller exists");
    let corridor_handle = sim(&game).source_arena().handle(corridor).unwrap();
    let corridor_position = sim(&game).entities()[usize::from(corridor)].position;
    let closed_floor = sim(&game).canonical_terrain().ground_height(corridor_position);
    let closed_ceiling = sim(&game).canonical_terrain().ceiling_height(corridor_position).unwrap();
    assert!(sim(&game).canonical_terrain().cave_collision_blocked(corridor_position));
    trigger(&mut game, player, 324, 3, &mut receipts);
    let close = receipts.close.expect("actual model63 generation");
    for _ in 0..64 {
        if sim(&game).entities()[usize::from(corridor)].life == 2 { break; }
        step(&mut game, GameCommand::default(), &mut receipts);
    }
    assert_eq!(sim(&game).source_arena().handle(corridor), Some(corridor_handle));
    assert_eq!(sim(&game).entities()[usize::from(corridor)].life, 2, "real model63 opens the controller");
    assert!(sim(&game).source_arena().resolve(close).is_none()
        || sim(&game).economy().terrain_strip_close_triggers().iter().any(|row|
            row.source_entity_index == close.source_slot && !row.active));
    let mut open_callback = false;
    for _ in 0..256 {
        step(&mut game, GameCommand::default(), &mut receipts);
        open_callback |= sim(&game).cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. } if *slot == corridor));
        assert_eq!(sim(&game).source_arena().handle(corridor), Some(corridor_handle));
        if sim(&game).entities()[usize::from(corridor)].life == 4 { break; }
    }
    assert!(open_callback);
    assert_eq!(sim(&game).entities()[usize::from(corridor)].life, 4, "actual controller finishes restoring the corridor");
    let open_floor = sim(&game).canonical_terrain().ground_height(corridor_position);
    let open_ceiling = sim(&game).canonical_terrain().ceiling_height(corridor_position).unwrap();
    assert!(open_floor < closed_floor && open_ceiling > closed_ceiling);
    assert!(!sim(&game).canonical_terrain().cave_collision_blocked(corridor_position));
    // Leaving the actual inverse switch admits its group4 model64, which
    // re-seals the same controller without changing either terrain timer.
    sim_mut(&mut game).set_level2_entity_position(player, spawn).unwrap();
    for _ in 0..64 {
        step(&mut game, GameCommand::default(), &mut receipts);
        if count(&receipts, 4) > 0 && sim(&game).entities()[usize::from(corridor)].life == 1 { break; }
    }
    assert!(count(&receipts, 4) > 0, "actual inverse switch publishes group4");
    assert_eq!(sim(&game).source_arena().handle(corridor), Some(corridor_handle));
    assert_eq!(sim(&game).entities()[usize::from(corridor)].life, 1, "real model64 seals the controller");
    let mut seal_callback = false;
    for _ in 0..256 {
        step(&mut game, GameCommand::default(), &mut receipts);
        seal_callback |= sim(&game).cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. } if *slot == corridor));
        assert_eq!(sim(&game).source_arena().handle(corridor), Some(corridor_handle));
        if sim(&game).entities()[usize::from(corridor)].life == 3 { break; }
    }
    assert!(seal_callback);
    assert_eq!(sim(&game).entities()[usize::from(corridor)].life, 3, "actual controller finishes sealing the corridor");
    assert!(sim(&game).canonical_terrain().ground_height(corridor_position) > open_floor);
    assert!(sim(&game).canonical_terrain().ceiling_height(corridor_position).unwrap() < open_ceiling);
    assert!(sim(&game).canonical_terrain().cave_collision_blocked(corridor_position));

    // Activate both source mana waves before sampling the global denominator.
    trigger(&mut game, player, 310, 1, &mut receipts);
    trigger(&mut game, player, 317, 2, &mut receipts);
    assert_eq!(receipts.sphere_births.len(), 113, "both real sphere producers allocate their complete source waves");
    // The two neighboring cave area switches admit both distinct reward and
    // Fire Spheres groups; keep the entire source-authored lists in receipts.
    trigger(&mut game, player, 74, 5, &mut receipts);
    if count(&receipts, 9) == 0 { trigger(&mut game, player, 249, 9, &mut receipts); }
    let fire = receipts.fire_sphere.expect("group9 has an actual Fire Spheres root");
    assert!(sim(&game).economy().model76_children().iter().any(|child|
        child.parent_entity_index == fire.source_slot), "the actual Fire Spheres producer allocates children");
    let pickup = *sim(&game).economy().spell_pickups().iter().find(|row|
        row.authored_index == 366 && row.family == 23 && row.active)
        .expect("group5 constructs the real Magic Mine pickup");
    assert_eq!(pickup.action, 70);
    let pickup_handle = sim(&game).source_arena().handle(pickup.source_entity_index).unwrap();
    let ring_positions = sim(&game).mission().authored_features().iter()
        .filter(|row| row.kind == 10 && row.subtype == 11 && row.activation_group == 5)
        .map(|row| (row.tile_x, row.tile_y)).collect::<BTreeSet<_>>();
    assert_eq!(ring_positions.len(), 12);
    let rings = sim(&game).entities().iter().filter(|row| row.active
        && (row.class, row.model, row.action) == (10, 11, 11)
        && ring_positions.contains(&(row.position.x >> 8, row.position.y >> 8)))
        .filter_map(|row| sim(&game).source_arena().handle(row.index)).collect::<Vec<_>>();
    assert_eq!(rings.len(), 12, "the group5 source rings must all construct");
    // Authored366 starts inside solid Cave terrain. The group's twelve real
    // lowering rings must excavate it and the pickup's own scheduled callback
    // must relax it toward the changing floor before a player can overlap it.
    sim_mut(&mut game).set_level2_entity_position(player, spawn).unwrap();
    let mut pose = None;
    let mut fire_callback = false;
    let mut ring_callback = false;
    let mut pickup_callback = false;
    for _ in 0..512 {
        step(&mut game, GameCommand::default(), &mut receipts);
        let state = sim(&game);
        fire_callback |= state.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, selector, .. }
                if *slot == fire.source_slot && selector.observed_dynamic_cursor_row().is_some_and(|row|
                    row.source_slot == fire.source_slot && row.generation == fire.generation
                        && row.kind == crate::native_world::Level2DynamicArenaKind::AuthoredModel76)));
        ring_callback |= state.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                if rings.iter().any(|handle| handle.source_slot == *slot)));
        pickup_callback |= state.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                if *slot == pickup_handle.source_slot));
        assert_eq!(state.source_arena().handle(pickup_handle.source_slot), Some(pickup_handle), "actual pickup remains current while the cave opens");
        let jar = state.shared_spells().pickup(pickup_handle.source_slot).expect("actual current shared pickup");
        let terrain = state.canonical_terrain();
        let floor = terrain.ground_height(jar.position);
        let ceiling = terrain.ceiling_height(jar.position).expect("actual pickup is in a cave");
        let low = floor.saturating_add(state.profile().ground_clearance);
        let high = ceiling.saturating_sub(state.profile().ceiling_clearance);
        if low <= high && !terrain.cave_collision_blocked(jar.position) {
            let body = &state.entities()[usize::from(player)];
            let offset = |row: &crate::native_world::EntitySnapshot|
                i16::from_le_bytes(row.record_bytes()[86..88].try_into().unwrap());
            let center = i32::from(jar.position.z) + i32::from(offset(jar)) - i32::from(offset(body));
            let mut candidate = jar.position;
            candidate.z = center.clamp(i32::from(low), i32::from(high)) as i16;
            let mut probe = body.clone();
            probe.set_position(candidate);
            if crate::native_world::world_types::level2_source_intersects(jar, &probe) {
                pose = Some(candidate);
                break;
            }
        }
    }
    let pose = pose.unwrap_or_else(|| {
        let state = sim(&game);
        let jar = state.shared_spells().pickup(pickup_handle.source_slot);
        let terrain = state.canonical_terrain();
        panic!("group5 never admits its actual pickup: jar={jar:?}, floor={}, ceiling={:?}, blocked={}, ring_callback={ring_callback}, pickup_callback={pickup_callback}, live_rings={:?}",
            terrain.ground_height(pickup.position), terrain.ceiling_height(pickup.position),
            terrain.cave_collision_blocked(pickup.position), rings.iter().filter(|h| state.source_arena().resolve(**h).is_some()).collect::<Vec<_>>());
    });
    assert!(ring_callback && pickup_callback, "actual lowering rings and pickup must be scheduled");
    sim_mut(&mut game).set_level2_entity_position(player, pose).unwrap();
    let mut acquired = false;
    for _ in 0..16 {
        step(&mut game, GameCommand::default(), &mut receipts);
        let state = sim(&game);
        fire_callback |= state.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, selector, .. }
                if *slot == fire.source_slot && selector.observed_dynamic_cursor_row().is_some_and(|row|
                    row.source_slot == fire.source_slot && row.generation == fire.generation
                        && row.kind == crate::native_world::Level2DynamicArenaKind::AuthoredModel76)));
        acquired |= state.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::SpellAcquisition { source_slot, family: 23, result: Some(outcome), .. }
                if *source_slot == pickup.source_entity_index && outcome.parent_entity_index == player
                    && outcome.holder_action == 69));
        if acquired && fire_callback { break; }
    }
    assert!(acquired && fire_callback, "the real Magic Mine pickup and Fire Spheres callback must reach their consumers");
    assert!(sim(&game).canonical_player_life_for_test() >= 0);
    sim_mut(&mut game).set_level2_entity_position(player, spawn).unwrap();
    for _ in 0..512 {
        if rings.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()) { break; }
        step(&mut game, GameCommand::default(), &mut receipts);
    }
    assert!(rings.iter().all(|handle| sim(&game).source_arena().resolve(*handle).is_none()),
        "all group5 lowering-ring generations must finish and reclaim naturally");
    assert!(sim(&game).canonical_player_life_for_test() >= 0);
    // The real Magic Mine acquisition may have changed the selected hand.
    step(&mut game, GameCommand {
        select_spell: Some(crate::game_definition::SemanticId::parse("spell.possession").unwrap()),
        ..GameCommand::default()
    }, &mut receipts);
    // Select one genuine settled member of the two produced mana waves and a
    // declared cave-valid player firing pose. Spheres and collectors move only
    // through their ordinary source callbacks.
    let mut selection = None;
    for _ in 0..256 {
        let state = sim(&game);
        let fov = state.shared_spells().player_entity().array_fov();
        for sphere in state.economy().spheres().iter().filter(|row|
            receipts.sphere_births.contains(&(row.id, state.economy().slot_generation(row.source_entity_index)))
                && row.active && !row.pending_delete
                && row.player_entity_index == 0 && row.control_flags & 8 != 0
                && row.control_flags & 0x0020_0000 == 0 && row.vertical_velocity == 0 && row.mana > 0)
        {
            let Some(target) = state.possession_guidance_target_top_for_test(sphere.source_entity_index) else { continue };
            for dx in [2048_i16, -2048] {
                let mut hand = Position { x: target.x.wrapping_add_signed(dx), y: target.y,
                    z: target.z.saturating_add(512) };
                let yaw = crate::level1_mission::projectile_guidance_angles(hand, target).0;
                let hand_yaw = yaw.wrapping_sub(state.shared_spells().beyond_sight_auxiliary().next_entity).wrapping_sub(512);
                let mut origin = Position { z: hand.z.saturating_sub(fov), ..hand };
                crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
                let floor = state.canonical_terrain().ground_height(origin);
                let Some(ceiling) = state.canonical_terrain().ceiling_height(origin) else { continue };
                origin.z = origin.z.max(floor.saturating_add(state.profile().ground_clearance));
                if origin.z > ceiling.saturating_sub(state.profile().ceiling_clearance)
                    || state.canonical_terrain().cave_collision_blocked(origin) { continue; }
                hand.z = origin.z.saturating_add(fov);
                if hand.z > ceiling.saturating_sub(crate::level1_mission::POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT) { continue; }
                let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(hand, target);
                selection = Some((*sphere, origin, yaw, pitch));
                break;
            }
            if selection.is_some() { break; }
        }
        if selection.is_some() { break; }
        step(&mut game, GameCommand::default(), &mut receipts);
    }
    let (sphere, origin, yaw, pitch) = selection.expect("a real produced sphere has an admitted cave firing pose");
    let sphere_handle = sim(&game).source_arena().handle(sphere.source_entity_index).unwrap();
    // Declared prior Castle/bank; source arithmetic below guarantees that the
    // bank alone is below 40%. The final increment must be actually collected.
    let castle = {
        let state = sim_mut(&mut game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            state.economy_mut().set_castle_stage_footprint(stage, footprint);
        }
        let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position { x: x * 2048, y: y * 2048, z: 0 }))
            .map(|mut p| { p.z = state.canonical_terrain().ground_height(p); p })
            .filter(|p| {
                let dx = i32::from(p.x.wrapping_sub(sphere.position.x) as i16).abs();
                let dy = i32::from(p.y.wrapping_sub(sphere.position.y) as i16).abs();
                dx.max(dy) >= 4096 && !state.economy().castle_projectile_site_blocked(*p, state.entities(), state.canonical_terrain())
            }).min_by_key(|p| crate::distance_3d_wrapped(*p, sphere.position))
            .expect("source terrain admits the declared prior Castle outside the sphere");
        state.economy_mut().build_castle(site, player).unwrap()
    };
    step(&mut game, GameCommand::default(), &mut receipts);
    assert_eq!(sim(&game).source_arena().handle(sphere_handle.source_slot), Some(sphere_handle));
    assert!(sim(&game).economy().spheres().iter().any(|row| row.id == sphere.id && row.active
        && !row.pending_delete && row.player_entity_index == 0 && row.mana == sphere.mana
        && row.position == sphere.position), "selected actual sphere remains the same settled generation and payload");
    let total = i64::from(sim(&game).economy().owner_mana_global());
    let aggregate = i64::from(sim(&game).economy().owner_mana_aggregate(player));
    let numerator = total.checked_mul(40).and_then(|n| aggregate.checked_mul(100).and_then(|a| n.checked_sub(a)))
        .expect("source 40-percent census arithmetic").max(0);
    let threshold = (numerator + 59) / 60;
    let bank = i32::try_from(threshold - i64::from(sphere.mana)).expect("prior bank fits i32");
    assert!(bank >= 0 && sphere.mana > 0);
    // The source impact can claim multiple nearby spheres. Preserve enough
    // declared prior capacity for all still-live source-produced payloads;
    // capacity itself does not contribute to the source percentage.
    let eligible = sim(&game).economy().spheres().iter().filter_map(|row| {
        let handle = sim(&game).source_arena().handle(row.source_entity_index)?;
        (row.active && !row.pending_delete && row.player_entity_index == 0 && row.mana > 0
            && receipts.sphere_births.contains(&(row.id, handle.generation)))
            .then_some(((handle.source_slot, handle.generation), row.mana))
    }).collect::<BTreeMap<_, _>>();
    assert!(eligible.contains_key(&(sphere_handle.source_slot, sphere_handle.generation)));
    let eligible_total = eligible.values().try_fold(0_i64, |sum, mana| sum.checked_add(i64::from(*mana)))
        .expect("real candidate sphere payloads fit i64");
    let capacity = i32::try_from(aggregate + i64::from(bank) + eligible_total).unwrap();
    assert!(capacity <= i32::MAX / 100);
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, capacity));
    assert!(sim_mut(&mut game).economy_mut().set_castle_mana_prerequisite_for_test(castle, bank));
    let castle_handle = sim(&game).source_arena().handle(castle).unwrap();
    step(&mut game, GameCommand::default(), &mut receipts);
    assert_eq!(i64::from(sim(&game).economy().owner_mana_global()), total + i64::from(bank),
        "actual next source census includes the prior bank and both completed waves");
    assert!(percent(&game, player) < 40);
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 1);
    assert_eq!(count(&receipts, 7), 0, "the real gate remains closed below 40%");

    let state = sim_mut(&mut game);
    state.set_level2_entity_position(player, origin).unwrap();
    state.state.actual_speed = 0;
    state.state.target_speed = 0;
    state.set_level2_player_aim_for_test(yaw, pitch);
    let holder = sim(&game).shared_spells().local_player().enabled_spell_holders[1];
    let holder_handle = sim(&game).source_arena().handle(holder).unwrap();
    let player_handle = sim(&game).source_arena().handle(player).unwrap();
    let cost = sim(&game).shared_spells().pickup(holder).unwrap().max_mana;
    assert!(cost > 0 && sim(&game).shared_spells().player_entity().mana >= cost);
    let mut cursor = sim(&game).economy().events().len();
    let (mut paid, mut debited, mut claimed, mut retreated, mut credited) = (false, false, false, false, false);
    let mut mana_changes = std::collections::VecDeque::new();
    let mut mana_events = std::collections::VecDeque::new();
    let mut projectile_handle = None;
    let mut paid_effects = BTreeMap::new();
    let mut impact_targets = BTreeSet::new();
    let mut claimed_targets = BTreeSet::new();
    let mut possession_events = std::collections::VecDeque::new();
    let mut collected = BTreeMap::<(u16, u32), BTreeMap<(u16, u32), i32>>::new();
    crate::native_world::level2_dynamic_economy::start_possession_trace();
    let castle_position = sim(&game).economy().castle_at(castle).unwrap().position;
    let bound = 32 + 2 * (crate::distance_3d_wrapped(sphere.position, castle_position) as usize).div_ceil(48) + 256;
    for turn in 0..bound {
        if claimed && !retreated {
            assert!(sim(&game).canonical_player_life_for_test() >= 0);
            sim_mut(&mut game).set_level2_entity_position(player, spawn).unwrap();
            retreated = true;
        }
        let before = (sim(&game).economy().owner_mana_global(), sim(&game).economy().owner_mana_aggregate(player),
            sim(&game).economy().castle_at(castle).map(|row| (row.mana, row.mana_capacity)), percent(&game, player));
        let unprotected = sim(&game).economy().spheres().iter().filter_map(|row| {
            let handle = sim(&game).source_arena().handle(row.source_entity_index)?;
            (row.active && !row.pending_delete && row.control_flags & 0x0020_0000 == 0)
                .then_some((handle.source_slot, handle.generation))
        }).collect::<BTreeSet<_>>();
        step(&mut game, GameCommand { cast_selected: turn == 0, ..GameCommand::default() }, &mut receipts);
        let state = sim(&game);
        let after = (state.economy().owner_mana_global(), state.economy().owner_mana_aggregate(player),
            state.economy().castle_at(castle).map(|row| (row.mana, row.mana_capacity)), percent(&game, player));
        if before != after {
            if mana_changes.len() == 32 { mana_changes.pop_front(); }
            mana_changes.push_back((turn, before, after));
        }
        debited |= state.cf10_production_schedule_receipts().into_iter().any(|receipt| match receipt {
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
            } if slot == player => original.observed_player_resources().zip(selector.observed_player_resources())
                .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                    && before.2 <= -cost && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
                    && before.1.saturating_sub(after.1) >= cost),
            _ => false,
        });
        if turn == 0 {
            projectile_handle = state.economy().possession_projectiles().iter().find_map(|row|
                (row.source_id_entity_index == player && row.holder_entity_index == holder_handle.source_slot)
                    .then(|| state.source_arena().handle(row.source_entity_index)).flatten());
            let charged = state.cf10_production_schedule_receipts().into_iter().any(|receipt| match receipt {
                crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted {
                    slot, original, current: crate::campaign::schedule::CurrentOccupant::Known { selector, .. }, ..
                } if slot == holder => original.observed_player_resources().zip(selector.observed_player_resources())
                    .is_some_and(|(before, after)| before.0 == player_handle && after.0 == player_handle
                        && before.1 >= cost && after.2 == if before.2 >= 0 { -cost } else { before.2.wrapping_sub(cost) }),
                _ => false,
            });
            paid = projectile_handle.is_some() && charged;
            assert!(paid, "public Possession launches and queues its real cost");
        }
        let trace = crate::native_world::level2_dynamic_economy::take_possession_trace();
        crate::native_world::level2_dynamic_economy::start_possession_trace();
        for receipt in trace {
            if possession_events.len() == 32 { possession_events.pop_front(); }
            possession_events.push_back((turn, format!("{receipt:?}")));
            use crate::native_world::level2_dynamic_economy::PossessionSourceReceipt;
            match receipt {
                PossessionSourceReceipt::ImpactBirth { effect, projectile, holder, owner, packet }
                    if Some(projectile) == projectile_handle && holder == holder_handle && owner == player_handle => {
                        assert!(matches!(packet.source_model, 12 | 70));
                        paid_effects.insert((effect.source_slot, effect.generation), packet.source_model);
                    }
                PossessionSourceReceipt::MailboxWrite { effect, target, source_id, before_possessor, amount, possessor, .. }
                    if source_id == player && possessor == player && before_possessor != player => {
                        let key = (target.source_slot, target.generation);
                        if let Some(model) = paid_effects.get(&(effect.source_slot, effect.generation))
                            && amount == u32::from(*model == 70)
                            && (amount != 0 || unprotected.contains(&key))
                            && eligible.contains_key(&key)
                        {
                            impact_targets.insert(key);
                        }
                    }
                _ => {}
            }
        }
        assert_eq!(state.source_arena().handle(castle), Some(castle_handle));
        for event in &state.economy().events()[cursor..] {
            if matches!(event, Level2EconomyEvent::ManaSphereSpawned { .. }
                | Level2EconomyEvent::ManaSphereClaimed { .. }
                | Level2EconomyEvent::ManaSpheresMerged { .. }
                | Level2EconomyEvent::ManaSphereCollected { .. }
                | Level2EconomyEvent::CastleDefenderCollectedSphere { .. })
                || matches!(event, Level2EconomyEvent::CastleDefenderArrived { mana, .. } if *mana != 0)
            {
                if mana_events.len() == 32 { mana_events.pop_front(); }
                mana_events.push_back((turn, format!("{event:?}"), before, after));
            }
            match *event {
                Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                    if owner_entity_index == player => {
                        let slot = u16::try_from(sphere_id).unwrap();
                        let key = (slot, state.economy().slot_generation(slot));
                        if impact_targets.contains(&key) {
                            assert!(eligible.contains_key(&key));
                            claimed_targets.insert(key);
                            claimed = true;
                        }
                    }
                Level2EconomyEvent::CastleDefenderCollectedSphere { defender_slot, sphere_id, mana } => {
                    let slot = u16::try_from(sphere_id).unwrap();
                    let key = (slot, state.economy().slot_generation(slot));
                    if claimed_targets.contains(&key) {
                        assert_eq!(eligible.get(&key), Some(&mana));
                        let defender = state.source_arena().handle(defender_slot).unwrap();
                        collected.entry((defender.source_slot, defender.generation)).or_default().insert(key, mana);
                    }
                }
                Level2EconomyEvent::CastleDefenderArrived { defender_slot, castle_id, mana }
                    if castle_id == castle && mana > 0 => {
                        let defender = state.source_arena().handle(defender_slot).unwrap();
                        let Some(payloads) = collected.get(&(defender.source_slot, defender.generation)) else { continue };
                        let payload = payloads.values().try_fold(0_i32, |sum, amount| sum.checked_add(*amount)).unwrap();
                        assert_eq!(mana, payload, "the same current collector deposits exactly its traced source-sphere payload");
                        assert_eq!(after.2.unwrap().0.checked_sub(before.2.unwrap().0), Some(mana));
                        assert!(percent(&game, player) >= 40,
                            "actual collector credit earns 40%: turn={turn}, total={total}, aggregate={aggregate}, bank={bank}, sphere={sphere:?}");
                        credited = true;
                    }
                _ => {}
            }
        }
        cursor = state.economy().events().len();
        if !credited {
            assert!(percent(&game, player) < 40,
                "source mana crossed before traced credit: turn={turn}, initial=(total={total}, aggregate={aggregate}, bank={bank}, capacity={capacity}), aim={sphere:?}, aim_handle={sphere_handle:?}, current_handle={:?}, eligible={eligible:?}, effects={paid_effects:?}, impacts={impact_targets:?}, claims={claimed_targets:?}, collected={collected:?}, possession={possession_events:?}, changes={mana_changes:?}, events={mana_events:?}, objective={:?}",
                state.source_arena().handle(sphere.source_entity_index), state.runtime().objectives[owner]);
            assert_eq!(state.runtime().objectives[owner].stage_selectors[0], 1);
        }
        if credited && count(&receipts, 7) > 0 { break; }
    }
    let _ = crate::native_world::level2_dynamic_economy::take_possession_trace();
    assert!(paid && debited && claimed && retreated && !collected.is_empty() && credited && count(&receipts, 7) == 1,
        "paid={paid}/{debited}, claim={claimed}, impacts={impact_targets:?}, collected={collected:?}, credit={credited}, group7={}, bank={bank}, total={total}, sphere={sphere:?}", count(&receipts, 7));
    assert_eq!(sim(&game).runtime().objectives[owner].stage_selectors[0], 2);
    assert_eq!(sim(&game).runtime().objectives[owner].is_level_end, 1);
    for _ in 0..8 {
        if sim(&game).source_arena().handle(350) != Some(gate_handle) { break; }
        step(&mut game, GameCommand::default(), &mut receipts);
    }
    assert_ne!(sim(&game).source_arena().handle(350), Some(gate_handle));
    let exit = *sim(&game).economy().authored_switch_producers().iter()
        .find(|row| row.active && row.authored_index == 416 && row.model == 12 && row.action == 12)
        .expect("actual group7 exit416");
    assert!(sim(&game).economy().authored_class14().iter().any(|row| row.active && row.model == 3
        && row.authored_index == 418), "actual group7 visible portal418");
    assert!(sim(&game).canonical_player_life_for_test() >= 0, "a living player must enter the real exit");
    sim_mut(&mut game).set_level2_entity_position(player, exit.position).unwrap();
    for _ in 0..700 {
        if matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }) { break; }
        step(&mut game, GameCommand::default(), &mut receipts);
    }
    assert!(matches!(game.completion_status(game.identity().guard()).unwrap().value, GameCompletionStatus::Completed { .. }));
    sim(&game).source_exit_result_byte()
}
