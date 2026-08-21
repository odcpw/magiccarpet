    #[test]
    fn level_two_castle_root_positions_follow_source_grid_and_anchor_vector() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let terrain = SessionTerrain::from_inspection(&inspection);
        let footprint = CastleBuildFootprint::from_owner_cells(1, 8, 8, &[0xff; 128])
            .expect("source 8x8 BUILD vector");
        let projectile = Position {
            x: 0x13_45,
            y: 0x56_78,
            z: 1_234,
        };
        // Make the selector error observable: every candidate 4x4 and 8x8
        // perimeter cell is high, except `(16,82)`, which belongs only to
        // the consumer-effective 8x8 boundary. A second VGA-halving pass
        // therefore reports 200*32 instead of the source 0*32 anchor.
        let mut boundary_terrain = terrain.clone();
        for (width, height) in [(8_u8, 8_u8), (4, 4)] {
            let center_x = 20_u8;
            let center_y = 86_u8;
            let start_x = center_x - width / 2;
            let start_y = center_y - height / 2;
            for offset in 0..width {
                boundary_terrain.planes.ground_heights[terrain_index(start_x + offset, start_y)] =
                    200;
                boundary_terrain.planes.ground_heights
                    [terrain_index(start_x + offset, start_y + height)] = 200;
            }
            for offset in 0..height {
                boundary_terrain.planes.ground_heights[terrain_index(start_x, start_y + offset)] =
                    200;
                boundary_terrain.planes.ground_heights
                    [terrain_index(start_x + width, start_y + offset)] = 200;
            }
        }
        boundary_terrain.planes.ground_heights[terrain_index(16, 82)] = 0;
        let (boundary_root, boundary_anchor) =
            level2_castle_root_positions(&boundary_terrain, &footprint, projectile);
        assert_eq!((boundary_root.x, boundary_root.y), (0x14_00, 0x56_00));
        assert_eq!(boundary_anchor.z, 0);
        let (_, halved_anchor) = level2_castle_root_positions_with_resolution(
            &boundary_terrain,
            &footprint,
            projectile,
            true,
        );
        assert_eq!(halved_anchor.z, 200 * 32);
        let (root, anchor) = level2_castle_root_positions(&terrain, &footprint, projectile);
        assert_eq!((root.x, root.y), (0x14_00, 0x56_00));
        assert_eq!((anchor.x, anchor.y), (root.x, root.y));
        let center_x = (root.x >> 8) as u8;
        let center_y = (root.y >> 8) as u8;
        let width = footprint.width;
        let height = footprint.height;
        let start_x = center_x.wrapping_sub(width / 2);
        let start_y = center_y.wrapping_sub(height / 2);
        let mut minimum = 250_u8;
        for offset in 0..width {
            for (x, y) in [
                (start_x.wrapping_add(offset), start_y),
                (start_x.wrapping_add(offset), start_y.wrapping_add(height)),
            ] {
                minimum = minimum.min(
                    (terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    }) >> 5) as u8,
                );
            }
        }
        for offset in 0..height {
            for (x, y) in [
                (start_x.wrapping_add(width), start_y.wrapping_add(offset)),
                (start_x, start_y.wrapping_add(offset)),
            ] {
                minimum = minimum.min(
                    (terrain.ground_height(Position {
                        x: u16::from(x) << 8,
                        y: u16::from(y) << 8,
                        z: 0,
                    }) >> 5) as u8,
                );
            }
        }
        assert_eq!(anchor.z, i16::from(minimum) * 32);

        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 placement session");
        let castle_slot = session
            .economy_mut()
            .build_castle_with_footprint(projectile, 111, &footprint)
            .expect("source-derived Castle placement");
        let castle = session.economy().castle().expect("Castle");
        assert_eq!(castle.source_entity_index, castle_slot);
        assert_eq!(castle.position, root);
        assert_eq!(castle.anchor_position, anchor);
        assert_eq!(
            (
                castle.action,
                castle.build_state,
                castle.level,
                castle.life,
                castle.max_life,
                castle.mana_capacity
            ),
            (5, 0, 0, 40_000, 40_000, 0)
        );
        assert_eq!(session.economy().player_castle_entity_index(), 0);
        assert!(session.economy().map_chains().is_linked(castle_slot));
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let stage_controller = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("stage-one model-79 controller");
        assert_eq!(
            (stage_controller.position.x, stage_controller.position.y),
            (root.x, root.y)
        );
        assert_eq!(stage_controller.chain_previous_slot, castle_slot);
    }

    #[test]
    fn level_two_castle_controller_points_pin_stage_corner_vectors() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let vectors: &[(u8, &[(i8, i8)])] = &[
            (1, &[(0, 0)]),
            (2, &[(-7, -7), (7, -7), (-7, 7), (7, 7)]),
            (4, &[(-14, -14), (14, -14), (-14, 14), (14, 14)]),
            (
                6,
                &[
                    (-21, -21),
                    (0, -21),
                    (21, -21),
                    (-21, 0),
                    (21, 0),
                    (-21, 21),
                    (0, 21),
                    (21, 21),
                ],
            ),
        ];
        for &(level, expected_offsets) in vectors {
            let mut session = inspection
                .level2_reset_session()
                .expect("controller vector session");
            let root_position = Position {
                x: 0x20_00,
                y: 0x30_00,
                z: 0,
            };
            let castle_slot = session
                .economy_mut()
                .build_castle(root_position, 111)
                .expect("controller vector Castle");
            {
                let castle = session
                    .economy_mut()
                    .castle
                    .as_mut()
                    .expect("controller vector Castle");
                castle.action = 5;
                castle.build_state = 3;
                castle.level = level.saturating_sub(1);
            }
            assert!(session.economy_mut().step_castle_action(castle_slot));
            let castle = session
                .economy()
                .castle()
                .expect("controller vector Castle");
            assert_eq!(
                castle.controller_chain_count as usize,
                expected_offsets.len()
            );
            let slots = &castle.controller_chain_slots[..expected_offsets.len()];
            for (ordinal, (&slot, &(offset_x, offset_y))) in
                slots.iter().zip(expected_offsets).enumerate()
            {
                let child = session
                    .economy()
                    .castle_children()
                    .iter()
                    .find(|child| child.source_entity_index == slot)
                    .expect("controller child slot");
                let expected_x = root_position
                    .x
                    .wrapping_add_signed(i16::from(offset_x) * 256);
                let expected_y = root_position
                    .y
                    .wrapping_add_signed(i16::from(offset_y) * 256);
                assert_eq!(
                    (child.position.x, child.position.y),
                    (expected_x, expected_y)
                );
                assert_eq!(
                    child.chain_previous_slot,
                    if ordinal == 0 {
                        castle_slot
                    } else {
                        slots[ordinal - 1]
                    }
                );
                assert_eq!(
                    child.chain_next_slot,
                    slots.get(ordinal + 1).copied().unwrap_or_default()
                );
            }
        }
    }

    #[test]
    fn level_two_castle_controller_coefficients_back_down_to_nonzero_stage() {
        assert_eq!(level2_castle_shift_axes(1), (1_664, 1_664));
        assert_eq!(level2_castle_shift_axes(2), (3_328, 3_328));
        assert_eq!(level2_castle_shift_axes(4), (5_120, 5_120));
        assert_eq!(level2_castle_shift_axes(6), (6_784, 6_784));
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("controller fallback session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller fallback Castle");
        {
            let economy = session.economy_mut();
            assert!(economy.set_castle_controller_coefficient(2, 0x2a));
            assert!(economy.set_castle_controller_coefficient(3, 0));
            let castle = economy.castle.as_mut().expect("controller fallback Castle");
            castle.action = 5;
            castle.build_state = 3;
            castle.level = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let castle = session
            .economy()
            .castle()
            .expect("controller fallback Castle");
        assert_eq!(castle.level, 3);
        assert_eq!(castle.controller_chain_count, 4);
        for &slot in &castle.controller_chain_slots[..4] {
            let child = session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == slot)
                .expect("fallback model-79 child");
            assert_eq!((child.source_byte_43, child.source_word_4a), (0x2a, 2));
        }
    }

    #[test]
    fn level_two_castle_controller_selection_backs_down_zero_coefficients() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("controller coefficient session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("controller coefficient Castle");
        {
            let economy = session.economy_mut();
            assert!(economy.set_castle_controller_coefficient(1, 0x2a));
            assert!(economy.set_castle_controller_coefficient(2, 0));
            economy.free_slots.truncate(1);
            let castle = economy
                .castle
                .as_mut()
                .expect("controller coefficient Castle");
            castle.action = 6;
            castle.level = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let castle = session.economy().castle().expect("downgraded Castle");
        assert_eq!(castle.controller_chain_count, 1);
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.active && child.model == 79)
            .expect("backed-down controller");
        assert_eq!((child.source_byte_43, child.source_word_4a), (0x2a, 1));

        let mut no_controller = inspection
            .level2_reset_session()
            .expect("zero coefficient session");
        let zero_slot = no_controller
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("zero coefficient Castle");
        {
            let economy = no_controller.economy_mut();
            for stage in 1..=7 {
                assert!(economy.set_castle_controller_coefficient(stage, 0));
            }
            let castle = economy.castle.as_mut().expect("zero coefficient Castle");
            castle.action = 6;
            castle.level = 2;
        }
        assert!(no_controller.economy_mut().step_castle_action(zero_slot));
        assert_eq!(
            no_controller
                .economy()
                .castle()
                .expect("zero coefficient Castle")
                .controller_chain_count,
            0
        );
    }

    #[test]
    fn level_two_castle_stage_removal_uses_old_tab_and_sentinel_rng() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("stage removal session");
        let mut stage_two_payload = vec![0xff; 21 * 21 * 2];
        stage_two_payload[..2].copy_from_slice(&[0, 21]);
        let footprint = CastleBuildFootprint::from_owner_cells(2, 21, 21, &stage_two_payload)
            .expect("stage-two BUILD record");
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(2, footprint)
        );
        session.reset();
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("stage removal Castle");
        let source_seed = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("live Castle source RNG");
        let sentinel_seed = session.economy().sentinel_rng;
        {
            session.terrain.planes.ground_heights[terrain_index(246, 246)] = 100;
            let economy = session.economy_mut();
            let castle = economy.castle.as_mut().expect("stage removal Castle");
            castle.action = 6;
            castle.level = 2;
        }
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));
        let source_after = session
            .economy()
            .source_rng
            .get(&castle_slot)
            .copied()
            .expect("updated Castle source RNG");
        assert_eq!(source_after, source_seed);
        let first_sentinel_draw = level2_next_rng16(sentinel_seed);
        let expected_sentinel = if first_sentinel_draw % 50 <= 20 {
            first_sentinel_draw
        } else {
            level2_next_rng16(first_sentinel_draw)
        };
        let sentinel_after = session.economy().sentinel_rng;
        assert_eq!(sentinel_after, expected_sentinel);
        assert_ne!(sentinel_after, sentinel_seed);
        let changed_hash = session.economy().reset_hash();
        let mut sentinel_unchanged = session.economy().clone();
        sentinel_unchanged.sentinel_rng = sentinel_seed;
        assert_ne!(
            changed_hash,
            sentinel_unchanged.reset_hash(),
            "stage-removal sentinel RNG is hash-bound"
        );
        assert_ne!(changed_hash, baseline_hash);
        assert_eq!(
            session.economy().castle().expect("downgraded Castle").level,
            1
        );
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
        assert_eq!(session.economy().sentinel_rng, sentinel_seed);
        assert!(!session.economy().source_rng.contains_key(&castle_slot));
    }

    #[test]
    fn level_two_castle_model42_handler_preserves_clock_before_root_consumer() {
        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let footprint = inspection
            .level2_castle_stage_footprints
            .get(&1)
            .expect("stage-one BUILD footprint")
            .clone();
        let mut session = inspection
            .level2_reset_session()
            .expect("model-42 BUILD handler session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("model-42 BUILD root");
        let anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        assert!(session.economy_mut().set_castle_anchor(castle_slot, anchor));
        assert!(session.economy_mut().start_castle_construction(castle_slot));
        let expected_terrain = {
            let economy = session.economy();
            let mut expected = economy.terrain.clone();
            expected
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position: anchor,
                        remaining: 18,
                    },
                    &footprint,
                )
                .expect("stage-one BUILD pass");
            expected
        };
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let child_slot = session
            .economy()
            .castle()
            .expect("BUILD root")
            .construction_slot;
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        let initialized = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .copied()
            .expect("first model-42 BUILD callback");
        assert_eq!(initialized.source_word_2e, 0);
        assert_eq!(initialized.construction_target_height, 0);
        assert_eq!(initialized.source_dword_10, 18);
        assert_eq!(
            initialized.source_byte_3e,
            (child_slot as u8).wrapping_add(1),
            "BUILD callback epilogue advances byte_0x3E exactly once"
        );
        assert_eq!(initialized.source_byte_3b, 1);
        assert_eq!(
            initialized.control_flags & LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT,
            LEVEL2_CASTLE_CONSTRUCTION_INITIALIZED_BIT
        );
        assert_ne!(initialized.control_flags & 1, 0);
        assert_eq!(
            session.economy().terrain.planes,
            expected_terrain.planes,
            "37BC0 applies the canonical BUILD payload on its first callback"
        );

        for _ in 0..17 {
            assert!(
                session
                    .economy_mut()
                    .step_castle_child_terminal_slot(child_slot)
            );
        }
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .expect("positive BUILD clock")
                .source_dword_10,
            1
        );
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        let timer_one = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .copied()
            .expect("BUILD timer-one transition");
        assert_eq!(timer_one.source_dword_10, -25);

        for _ in 0..24 {
            assert!(
                session
                    .economy_mut()
                    .step_castle_child_terminal_slot(child_slot)
            );
        }
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .expect("negative BUILD clock")
                .source_dword_10,
            -1
        );
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        assert_eq!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == child_slot)
                .expect("completed BUILD clock")
                .source_dword_10,
            0
        );
        let state_two = session.economy().castle().expect("completed BUILD root");
        assert_eq!((state_two.action, state_two.build_state), (5, 2));
        assert_eq!(state_two.source_word_2e, 2);
        assert!(!state_two.construction_child_active);
        let hidden_child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == child_slot)
            .copied()
            .expect("hidden model-42 BUILD child remains allocated");
        let hidden_generation = session.economy().slot_generation(child_slot);
        assert!(hidden_child.active);
        assert_ne!(hidden_child.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        assert!(!session.economy().map_chains().is_linked(child_slot));

        // `UpdateEntities_57730` owns this reclaim, not the Castle root. A
        // destroyed root must not strand its already-hidden BUILD child;
        // after the generic prepass the old identity is gone, and the lowest
        // freed slot may be reused by the same ordinary turn.
        let mut root_destroyed = session.clone();
        let root_destroyed_generation = root_destroyed.economy().slot_generation(child_slot);
        root_destroyed
            .economy_mut()
            .castle
            .as_mut()
            .expect("root-gone Castle")
            .active = false;
        root_destroyed.select_level2_campaign_entry_stage();
        root_destroyed.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            root_destroyed
                .economy()
                .castle_children()
                .iter()
                .all(|child| {
                    !(child.active
                        && child.source_entity_index == child_slot
                        && child.source_class == hidden_child.source_class
                        && child.model == hidden_child.model
                        && child.action == hidden_child.action
                        && child.id_entity_index == hidden_child.id_entity_index
                        && child.owner_entity_index == hidden_child.owner_entity_index
                        && child.parent_entity_index == hidden_child.parent_entity_index)
                })
        );
        match root_destroyed
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == child_slot)
        {
            None => {
                assert!(!root_destroyed.economy().map_chains().is_linked(child_slot));
                assert!(root_destroyed.economy().free_slots.contains(&child_slot));
            }
            Some(row) => {
                assert_ne!(row.generation, root_destroyed_generation);
                assert_eq!(row.kind, Level2DynamicArenaKind::AuthoredBee);
                assert!(!root_destroyed.economy().free_slots.contains(&child_slot));
                assert!(root_destroyed.economy().map_chains().is_linked(child_slot));
            }
        }

        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session.economy().castle_children().iter().all(|child| {
                !(child.active
                    && child.source_entity_index == child_slot
                    && child.source_class == hidden_child.source_class
                    && child.model == hidden_child.model
                    && child.action == hidden_child.action
                    && child.id_entity_index == hidden_child.id_entity_index
                    && child.owner_entity_index == hidden_child.owner_entity_index
                    && child.parent_entity_index == hidden_child.parent_entity_index)
            }),
            "following UpdateEntities prologue reclaims the old model-42 BUILD row"
        );
        let slot_owner = session
            .economy()
            .live_arena_cursor_rows()
            .into_iter()
            .find(|row| row.source_slot == child_slot);
        match slot_owner {
            None => {
                assert!(!session.economy().map_chains().is_linked(child_slot));
                assert!(session.economy().free_slots.contains(&child_slot));
            }
            Some(row) => {
                // The source cursor may reuse the reclaimed lowest slot in
                // the same ordinary turn. Keep this assertion at the cursor
                // layer: the exact owner is the new source generation, not
                // necessarily another Castle-child struct.
                assert_ne!(row.generation, hidden_generation);
                assert_eq!(row.kind, Level2DynamicArenaKind::AuthoredBee);
                assert!(!session.economy().free_slots.contains(&child_slot));
                assert!(session.economy().map_chains().is_linked(child_slot));
            }
        }
        let consumed = session.economy().castle().expect("consumed root");
        assert_eq!((consumed.action, consumed.build_state), (4, 0));
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_outer_turn_sub5fd00_uses_source_share_and_one_draw_rng() {
        let package = level_two_fixture_path();
        let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("sub_5FD00 Castle");
        let global_before = session.economy().global_rng;
        let source_rng_before = *session
            .economy()
            .source_rng
            .get(&castle_slot)
            .expect("Castle source RNG");
        let before_spheres = session.economy().spheres().len();
        {
            let castle = session.economy_mut().castle.as_mut().expect("Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 7_500;
            castle.mana_capacity = 1_000;
        }

        // The ordinary Castle root route releases 6,500 excess mana. Source
        // sub_5FD00 uses six equal 1,083-unit shares and leaves the final two
        // excess units on the Castle; it does not top up the final sphere to
        // 1,085.
        session.select_level2_campaign_entry_stage();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let new_spheres = &session.economy().spheres()[before_spheres..];
        assert_eq!(new_spheres.len(), 6);
        assert!(new_spheres.iter().all(|sphere| sphere.mana == 1_083));
        assert_eq!(session.economy().castle().expect("Castle").mana, 1_002);
        assert_eq!(
            session
                .economy()
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Level2EconomyEvent::ManaSphereSpawned {
                            source_entity_index,
                            mana: 1_083,
                            ..
                        } if *source_entity_index == castle_slot
                    )
                })
                .count(),
            6
        );

        // NewEvent uses the post-pass global stream for every allocated slot;
        // sub_5FD00 advances each new sphere RNG exactly once and advances the
        // Castle RNG twice per sphere for radius then yaw.
        let global_after = level2_next_rng32(global_before);
        for sphere in new_spheres {
            let seed = sphere.source_entity_index.wrapping_add(global_after as u16);
            let one_draw = level2_next_rng16(seed);
            let two_draws = level2_next_rng16(one_draw);
            assert_eq!(sphere.entity_rng, one_draw);
            assert_ne!(sphere.entity_rng, two_draws);
        }
        let mut expected_source_rng = source_rng_before;
        for _ in 0..12 {
            expected_source_rng = level2_next_rng16(expected_source_rng);
        }
        assert_eq!(
            session.economy().source_rng.get(&castle_slot),
            Some(&expected_source_rng)
        );
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);

        // Insufficient mana does not enter the overflow allocator even though
        // the same qualified ordinary root route is visited.
        let mut no_overflow = inspection
            .level2_reset_session()
            .expect("qualified Level 2 reset session");
        let no_overflow_slot = no_overflow
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("no-overflow Castle");
        let no_overflow_before = no_overflow.economy().spheres().len();
        {
            let castle = no_overflow
                .economy_mut()
                .castle
                .as_mut()
                .expect("no-overflow Castle");
            castle.level = 1;
            castle.phase = 0;
            castle.mana = 1_000;
            castle.mana_capacity = 1_000;
        }
        no_overflow.select_level2_campaign_entry_stage();
        no_overflow.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            no_overflow.economy().spheres().len(),
            no_overflow_before,
            "sub_5FD00 does not allocate without excess mana"
        );
        assert_eq!(
            no_overflow
                .economy()
                .castle()
                .expect("no-overflow Castle")
                .mana,
            1_000
        );
        assert!(no_overflow.dispatch_events().iter().any(|event| matches!(
            event,
            Level2DispatchEvent::Castle { source_slot, .. }
                if *source_slot == no_overflow_slot
        )));
    }

    #[test]
    fn level_two_castle_model42_stage_two_composes_prior_build_cells() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("stage-two composite session");

        let mut stage_one_payload = vec![0xff_u8; 8 * 8 * 2];
        stage_one_payload[(4 * 8 + 4) * 2..(4 * 8 + 4) * 2 + 2].copy_from_slice(&[7, 4]);
        let stage_one = CastleBuildFootprint::from_owner_cells(1, 8, 8, &stage_one_payload)
            .expect("synthetic stage-one BUILD");

        let mut stage_two_payload = vec![0xff_u8; 21 * 21 * 2];
        stage_two_payload[..2].copy_from_slice(&[9, 6]);
        let stage_two = CastleBuildFootprint::from_owner_cells(2, 21, 21, &stage_two_payload)
            .expect("synthetic stage-two BUILD");
        assert_eq!(stage_two.cells()[10 * 21 + 10], [0xff, 0xff]);
        let stage_three =
            CastleBuildFootprint::from_owner_cells(3, 21, 21, &vec![0xff_u8; 21 * 21 * 2])
                .expect("synthetic stage-three BUILD");
        let stage_four =
            CastleBuildFootprint::from_owner_cells(4, 35, 35, &vec![0xff_u8; 35 * 35 * 2])
                .expect("synthetic stage-four BUILD");
        let mut stage_five_payload = vec![0xff_u8; 35 * 35 * 2];
        stage_five_payload[..2].copy_from_slice(&[0x11, 0x0c]);
        let stage_five = CastleBuildFootprint::from_owner_cells(5, 35, 35, &stage_five_payload)
            .expect("synthetic odd stage-five BUILD");
        let stage_six_payload = vec![0xff_u8; 48 * 48 * 2];
        let stage_six = CastleBuildFootprint::from_owner_cells(6, 48, 48, &stage_six_payload)
            .expect("synthetic stage-six BUILD");
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(1, stage_one)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(2, stage_two)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(3, stage_three)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(4, stage_four)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(5, stage_five)
        );
        assert!(
            session
                .economy_mut()
                .set_castle_stage_footprint(6, stage_six)
        );
        session.reset();

        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("stage-two composite Castle");
        let anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        assert!(session.economy_mut().set_castle_anchor(castle_slot, anchor));
        {
            let castle = session
                .economy_mut()
                .castle
                .as_mut()
                .expect("stage-two Castle");
            castle.action = 5;
            castle.build_state = 0;
            castle.level = 1;
            castle.source_word_2e = 2;
        }
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let child_slot = session
            .economy()
            .castle()
            .expect("stage-two Castle root")
            .construction_slot;

        let composite = session
            .economy()
            .composite_castle_construction_footprint(2)
            .expect("composite stage-two BUILD");
        assert_eq!(composite.cells()[10 * 21 + 10], [7, 4]);
        assert_eq!(composite.cells()[0], [9, 6]);

        let expected_footprint = CastleBuildFootprint::from_owner_cells(2, 21, 21, &{
            let mut payload = vec![0xff_u8; 21 * 21 * 2];
            payload[(10 * 21 + 10) * 2..(10 * 21 + 10) * 2 + 2].copy_from_slice(&[7, 4]);
            payload[..2].copy_from_slice(&[9, 6]);
            payload
        })
        .expect("expected composite stage-two BUILD");
        let mut expected_terrain = session.economy().terrain.clone();
        expected_terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining: 18,
                },
                &expected_footprint,
            )
            .expect("expected composite BUILD pass");

        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(child_slot)
        );
        assert_eq!(
            session.economy().terrain.planes,
            expected_terrain.planes,
            "37BC0 must preserve BUILD1 data inside the stage-two ff hole"
        );
        let center_x = anchor.x.wrapping_add(128).to_be_bytes()[0];
        let center_y = anchor.y.wrapping_add(128).to_be_bytes()[0];
        let inner_index = terrain_index(center_x, center_y);
        let outer_index = terrain_index(center_x.wrapping_sub(10), center_y.wrapping_sub(10));
        assert_eq!(
            session.economy().terrain.planes.ground_heights[inner_index],
            expected_terrain.planes.ground_heights[inner_index]
        );
        assert_eq!(
            session.economy().terrain.planes.terrain_types[outer_index],
            expected_terrain.planes.terrain_types[outer_index]
        );

        let odd_composite = session
            .economy()
            .composite_castle_construction_footprint(6)
            .expect("composite odd stage-six BUILD");
        assert_eq!(
            odd_composite.cells()[7 * 48 + 7],
            [0x11, 0x0c],
            "source centers stage-five origin at 24>>1 - 35>>1 = 7"
        );
        assert_ne!(
            odd_composite.cells()[6 * 48 + 6],
            [0x11, 0x0c],
            "midpoint subtraction would incorrectly place the marker at 6"
        );
    }

    #[test]
    fn level_two_castle_material_11_uses_source_corner_table() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("material-11 Castle session");
        let position = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        let x = position.x.wrapping_add(128).to_be_bytes()[0];
        let y = position.y.wrapping_add(128).to_be_bytes()[0];
        let index = terrain_index(x, y);
        let before = session.economy().terrain.clone();
        let corner = classify_castle_corner(&before.planes, x, y, 7);
        let table_offset = 0x40 + 2 * corner;
        let footprint = CastleBuildFootprint::from_owner_cells(3, 1, 1, &[0x11, 0xff])
            .expect("material-11 BUILD cell");

        assert!(
            session
                .economy_mut()
                .terrain
                .apply_castle_update(
                    CastleTerrainUpdate::ConstructionPass {
                        position,
                        remaining: 1,
                    },
                    &footprint,
                )
                .is_ok()
        );

        let after = &session.economy().terrain.planes;
        assert_eq!(
            after.terrain_types[index], TERRAIN_GENERATION_TABLE_PREFIX[table_offset],
            "material 0x11 must select the source 0x40 corner table"
        );
        assert_eq!(
            after.angles[index],
            ((before.planes.angles[index] & 0x8f)
                | TERRAIN_GENERATION_TABLE_PREFIX[table_offset + 1]
                | 0x80)
                & !0x08
        );
        for (corner_x, corner_y) in [
            (x, y),
            (x.wrapping_add(1), y),
            (x.wrapping_add(1), y.wrapping_add(1)),
            (x, y.wrapping_add(1)),
        ] {
            assert_eq!(
                after.angles[terrain_index(corner_x, corner_y)] & 0x08,
                0,
                "material 0x11 keeps the existing Day neighbor-angle handling"
            );
        }
    }

    #[test]
    fn level_two_castle_model41_msprd_is_separate_from_model42_build() {
        assert_eq!(
            LEVEL2_CASTLE_MODEL41_MSPRD_RECTS,
            [
                (0, 0),
                (16, 11),
                (16, 11),
                (16, 9),
                (16, 9),
                (16, 11),
                (1, 0),
                (1, 0)
            ]
        );
        assert_eq!(level2_castle_model41_msprd_rect(6), Some((1, 0)));

        let package = level_two_fixture_path();
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
        let mut session = inspection
            .level2_reset_session()
            .expect("model-41 discriminator session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("discriminator Castle");
        let anchor = Position {
            x: 3_072,
            y: 4_096,
            z: 0,
        };
        assert!(session.economy_mut().set_castle_anchor(castle_slot, anchor));
        assert!(session.economy_mut().start_castle_construction(castle_slot));
        assert!(session.economy_mut().step_castle_action(castle_slot));
        session.economy_mut().terrain.planes.ground_heights.fill(20);
        assert!(
            session
                .economy_mut()
                .start_castle_defense_child(castle_slot)
        );
        assert!(session.economy_mut().step_castle_action(castle_slot));
        let defense_slot = session
            .economy()
            .castle()
            .expect("defense root")
            .defense_slot;
        let before = session.economy().terrain.planes.clone();
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(defense_slot)
        );
        let defense = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defense_slot)
            .copied()
            .expect("model-41/MSPRD child");
        assert_eq!((defense.model, defense.action), (41, 43));
        assert_eq!(defense.cave_collision_cooldown, 1);
        assert_eq!(defense.source_word_2e, 0);
        assert_eq!(defense.source_dword_10, 10);
        assert_eq!(defense.construction_target_height, 20);
        assert_eq!(
            defense.source_byte_3e,
            (defense_slot as u8).wrapping_add(1),
            "MSPRD callback epilogue advances byte_0x3E exactly once"
        );
        assert_eq!(session.economy().terrain.planes, before);
    }

    #[test]
    fn level_two_applyevents_terminal_reclaims_castle_children() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("ApplyEvents terminal-reclaim session");
        let baseline_hash = session.economy().reset_hash();
        // ApplyEvents terminally advances/reclaims class-10 models 43 and 79
        // through the same final matrix as models 41/42. Model 79/action
        // 0x56 is a live callback only when the action-table route dispatches
        // that specific action; this direct matrix test uses the terminal
        // action value and proves both source models are covered.
        let models = [15_u8, 41, 42, 43, 79];
        let expected_count = models.len();
        let (slots, free_before) = {
            let economy = session.economy_mut();
            let mut slots = Vec::with_capacity(expected_count);
            for (ordinal, model) in models.iter().copied().enumerate() {
                let source_entity_index = economy
                    .allocate_slot()
                    .expect("ApplyEvents terminal child slot");
                let child = Level2CastleChild {
                    source_entity_index,
                    entity_rng: economy.new_event_rng(source_entity_index),
                    id_entity_index: 111,
                    source_class: if model == 15 { 5 } else { 10 },
                    parent_entity_index: 111,
                    owner_entity_index: 111,
                    player_entity_index: 111,
                    position: Position {
                        x: 0x1000 + (ordinal as u16) * 0x100,
                        y: 0x2000,
                        z: 0,
                    },
                    axis_position: Position::default(),
                    model,
                    action: if model == 15 { 121 } else { 0 },
                    mana: 0,
                    max_mana: 0,
                    life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    max_life: LEVEL2_CASTLE_CHILD_DEFAULT_MAX_LIFE,
                    speed: LEVEL2_CASTLE_CHILD_DEFAULT_SPEED,
                    target_sphere_slot: 0,
                    source_byte_3e: source_entity_index as u8,
                    source_byte_3b: 0,
                    cave_collision_cooldown: 0,
                    source_dword_10: 0,
                    source_word_2e: 0,
                    construction_target_height: 0,
                    source_subspell_index: 0,
                    yaw: 0,
                    roll: 0,
                    control_flags: 0,
                    chain_previous_slot: 0,
                    chain_next_slot: 0,
                    source_byte_43: 0,
                    source_word_4a: 0,
                    active: true,
                };
                economy.sync_castle_child_map(child);
                slots.push(source_entity_index);
                economy.castle_children.push(child);
            }
            (slots, economy.free_slots.len())
        };
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == slots[0])
            .expect("wrong class/model guard child")
            .source_class = 10;
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(slots[0])
                == false
        );
        session
            .economy_mut()
            .castle_children
            .iter_mut()
            .find(|child| child.source_entity_index == slots[0])
            .expect("restore class/model guard child")
            .source_class = 5;
        for (index, slot) in slots.iter().enumerate() {
            let reclaimed = if index == 0 {
                assert!(
                    session.economy_mut().step_castle_child_terminal_slot(*slot),
                    "ApplyEvents disables and reclaims terminal child in one visit"
                );
                None
            } else {
                Some(
                    session
                        .economy_mut()
                        .applyevents_terminal_reclaim_castle_child(*slot)
                        .expect("terminal child reclaim returns source write"),
                )
            };
            if let Some(reclaimed) = reclaimed {
                assert_eq!(
                    reclaimed.source_byte_3e,
                    u8::try_from(*slot)
                        .expect("fixture terminal source slot fits byte_0x3E")
                        .wrapping_add(1),
                    "class-10 terminal row advances byte_0x3E before reclaim"
                );
            }
            assert!(
                session
                    .economy()
                    .castle_children()
                    .iter()
                    .all(|child| child.source_entity_index != *slot)
            );
            assert!(
                !session
                    .economy()
                    .map_chains()
                    .linked_entity_indices()
                    .contains(slot)
            );
            assert_eq!(session.economy().free_slots.len(), free_before + index + 1);
        }
        assert!(
            session
                .economy_mut()
                .step_castle_child_terminal_slot(slots[0])
                == false
        );
        assert!(session.economy().castle_children().is_empty());
        for slot in &slots {
            assert_eq!(session.economy_mut().allocate_slot(), Some(*slot));
        }
        assert_eq!(session.economy().free_slots.len(), free_before);
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_outer_turn_reclaims_model15_auxiliary_through_source_no_call_route() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("model-15 outer-dispatch session");
        // Materialise the authored group-8 constructor before allocating a
        // test Castle, so the test's free-stack assertions isolate the
        // Castle lifecycle rather than same-turn authored construction.
        session.step_outer_turn(PlayerCommand::default(), 0);
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("model-15 source Castle");
        session
            .economy_mut()
            .castle
            .as_mut()
            .expect("model-15 Castle")
            .level = 3;
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let auxiliary_slot = session
            .economy()
            .castle()
            .expect("model-15 Castle state")
            .auxiliary_slots[0];
        assert_ne!(auxiliary_slot, 0);
        let free_before = session.economy().free_slots.len();

        // ApplyEvents_498A0 does not call the action table for class-5 rows:
        // it disables drawing and reclaims the model-15 row in this visit.
        session.step_outer_turn(PlayerCommand::default(), 0);

        assert!(
            session
                .economy()
                .castle_children()
                .iter()
                .all(|child| child.source_entity_index != auxiliary_slot)
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&auxiliary_slot)
        );
        assert_eq!(session.economy().free_slots.len(), free_before + 1);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::CastleAuxiliary,
                    ..
                } if *source_slot == auxiliary_slot
            )
        }));
    }

    #[test]
    fn level_two_castle_auxiliary_creation_sets_source_relation_and_cooldown() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("auxiliary creation session");
        let root_position = Position {
            x: 0x20_00,
            y: 0x30_00,
            z: 0,
        };
        let castle_slot = session
            .economy_mut()
            .build_castle(root_position, 111)
            .expect("auxiliary creation Castle");
        session
            .economy_mut()
            .castle
            .as_mut()
            .expect("auxiliary creation Castle")
            .level = 3;
        session.economy_mut().maintain_castle_defenders(castle_slot);

        let castle = session.economy().castle().expect("auxiliary Castle");
        assert_eq!(castle.auxiliary_count, 4);
        assert_eq!(castle.auxiliary_cooldown, 16);
        let auxiliary_slot = castle.auxiliary_slots[0];
        assert_ne!(auxiliary_slot, 0);
        assert_eq!(&castle.auxiliary_slots[1..4], &[0, 0, 0]);
        let child = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == auxiliary_slot)
            .expect("created model-15 auxiliary child");
        assert_eq!(
            (
                child.source_class,
                child.model,
                child.action,
                child.id_entity_index,
                child.player_entity_index,
                child.yaw,
                child.roll
            ),
            (
                5,
                15,
                121,
                castle.source_id_entity_index,
                castle.source_id_entity_index,
                512,
                512
            )
        );
        assert_eq!(
            child.position,
            Position {
                x: root_position.x.wrapping_add(128),
                y: root_position.y.wrapping_add(640),
                z: session.economy().terrain.ground_height(Position {
                    x: root_position.x.wrapping_add(128),
                    y: root_position.y.wrapping_add(640),
                    z: 0,
                }),
            }
        );
    }

    #[test]
    fn level_two_castle_auxiliary_invalid_relation_clears_slot_and_restarts_cooldown() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("auxiliary invalidation session");
        let baseline_hash = session.economy().reset_hash();
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("auxiliary invalidation Castle");
        let invalid_slot;
        {
            let economy = session.economy_mut();
            economy
                .castle
                .as_mut()
                .expect("auxiliary invalidation Castle")
                .level = 3;
            economy.maintain_castle_defenders(castle_slot);
            invalid_slot = economy
                .castle
                .as_ref()
                .expect("created auxiliary Castle")
                .auxiliary_slots[0];
            economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == invalid_slot)
                .expect("created auxiliary child")
                .action = 125;
            economy
                .castle
                .as_mut()
                .expect("auxiliary invalidation Castle")
                .auxiliary_cooldown = 0;
            economy.maintain_castle_defenders(castle_slot);
        }
        let castle = session.economy().castle().expect("invalidated Castle");
        assert_eq!(castle.auxiliary_slots[0], 0);
        assert_eq!(castle.auxiliary_count, 4);
        assert_eq!(castle.auxiliary_cooldown, 16);
        // `sub_5FF50` clears only the stale relation/cooldown. The wrong
        // action row remains live and is not returned to the free arena by
        // this maintenance walk.
        assert!(!session.economy().free_slots.contains(&invalid_slot));
        assert!(
            session
                .economy()
                .map_chains()
                .linked_entity_indices()
                .contains(&invalid_slot)
        );
        assert!(
            session
                .economy()
                .castle_children()
                .iter()
                .find(|child| child.source_entity_index == invalid_slot)
                .is_some_and(|child| child.active && child.action == 125)
        );
        assert_ne!(session.economy().reset_hash(), baseline_hash);
        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_auxiliary_walk_respects_authored_tail_bound() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("auxiliary tail-bound session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("auxiliary tail-bound Castle");
        {
            let economy = session.economy_mut();
            let castle = economy
                .castle
                .as_mut()
                .expect("auxiliary tail-bound Castle");
            castle.level = 3;
            castle.auxiliary_slots[4] = 0xbeef;
            castle.auxiliary_count = 0;
            castle.auxiliary_cooldown = 16;
            economy.maintain_castle_defenders(castle_slot);
        }
        let castle = session.economy().castle().expect("tail-bound Castle");
        assert_eq!(castle.auxiliary_cooldown, 15);
        assert_eq!(castle.auxiliary_slots[4], 0xbeef);
    }

    #[test]
    fn level_two_owner_mana_projection_matches_source_filters_and_reset() {
        let rows = vec![
            Level2OwnerManaRow {
                source_entity_index: 1,
                source_class: 3,
                source_model: 2,
                player_entity_index: 111,
                mana: 7,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 2,
                source_class: 3,
                source_model: 3,
                player_entity_index: 111,
                mana: 11,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 3,
                source_class: 5,
                source_model: 15,
                player_entity_index: 111,
                mana: 13,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 4,
                source_class: 10,
                source_model: 39,
                player_entity_index: 111,
                mana: 17,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 5,
                source_class: 10,
                source_model: 45,
                player_entity_index: 111,
                mana: 19,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 6,
                source_class: 10,
                source_model: 58,
                player_entity_index: 111,
                mana: 23,
                active: true,
            },
            // A zero player relation still contributes to the source global
            // counter but cannot enter an owner's dword_0x13C bucket.
            Level2OwnerManaRow {
                source_entity_index: 7,
                source_class: 10,
                source_model: 45,
                player_entity_index: 0,
                mana: 29,
                active: true,
            },
            // Unsupported class/model rows are skipped entirely.
            Level2OwnerManaRow {
                source_entity_index: 8,
                source_class: 10,
                source_model: 40,
                player_entity_index: 111,
                mana: 31,
                active: true,
            },
            Level2OwnerManaRow {
                source_entity_index: 9,
                source_class: 3,
                source_model: 2,
                player_entity_index: 111,
                mana: 37,
                active: false,
            },
        ];
        let (aggregate, global) = level2_owner_mana_projection(&rows);
        assert_eq!(aggregate.get(&111), Some(&19));
        assert_eq!(aggregate.get(&0), None);
        assert_eq!(global, 1 + 7 + 11 + 13 + 17 + 19 + 23 + 29);

        // `sub_61000` feeds every qualifying row into the owner's maxMana;
        // model-39 is deliberately distinct from the model-45 aggregate
        // assertion above.
        let baseline = BTreeMap::from([(111_u16, 100_i32), (222_u16, 50_i32)]);
        let projected = level2_owner_max_mana_projection(&baseline, &rows);
        assert_eq!(projected.get(&111), Some(&190));
        assert_eq!(projected.get(&222), Some(&50));

        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("owner aggregate session");
        assert_eq!(session.economy().owner_mana_global(), 12_835);
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        let baseline_hash = session.economy().reset_hash();

        session.reset();
        assert_eq!(session.economy().reset_hash(), baseline_hash);
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        assert_eq!(session.economy().owner_mana_global(), 12_835);
    }

    #[test]
    fn level_two_house_possession_reaches_castle_projection_at_next_source_boundary() {
        let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/level2-day-owner-cache/components/gameplay");
        let inspection = NativeWorld::inspect_level2_with_owner_cache(
            &level_two_fixture_path(),
            &gameplay_cache,
        )
        .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("qualified Level 2 production session");
        session.select_level2_campaign_entry_stage();
        assert_eq!(session.economy().player_mana(), 1_000);

        let live_house_slots = session
            .ordinary()
            .houses()
            .iter()
            .filter(|house| house.active)
            .map(|house| house.entity_index)
            .collect::<Vec<_>>();
        assert_eq!(live_house_slots, (1_u16..=22).collect::<Vec<_>>());
        let house_slots = session
            .ordinary()
            .houses()
            .iter()
            .filter(|house| house.active && !house.parameter.possession_blocked())
            .map(|house| house.entity_index)
            .collect::<Vec<_>>();
        let house_mana = session
            .economy()
            .baseline_owner_mana_rows
            .iter()
            .filter(|row| {
                house_slots.contains(&row.source_entity_index)
                    && row.source_class == HOUSE_CLASS
                    && row.source_model == HOUSE_MODEL
                    && row.active
            })
            .map(|row| row.mana)
            .sum::<i32>();
        assert_eq!(house_mana, 2_034);
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);

        let castle_position = Position::default();
        let sphere_position = Position {
            x: 10_000,
            y: 10_000,
            z: 0,
        };
        let loaded_sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(901, sphere_position, 0, 0x4321, 7_000, 0)
            .expect("qualified loading sphere");
        let loaded_sphere_slot = u16::try_from(loaded_sphere_id).expect("loading sphere slot");
        assert!(
            session
                .economy_mut()
                .claim_mana_sphere(loaded_sphere_slot, 111)
        );
        let carrier_slot = session
            .economy_mut()
            .spawn_mana_carrier(sphere_position, 111, loaded_sphere_id)
            .expect("qualified loading carrier");
        let castle_slot = session
            .economy_mut()
            .build_castle(castle_position, 111)
            .expect("qualified level-one Castle");
        let castle = session.economy().castle().expect("loaded Castle");
        assert_eq!(
            (castle.level, castle.mana, castle.mana_capacity),
            (1, 0, 8_500)
        );
        assert_eq!(castle.phase & 1, (castle_slot as u8) & 1);
        assert!(
            session
                .economy_mut()
                .absorb_sphere_into_carrier(loaded_sphere_slot, carrier_slot)
        );
        assert!(session.economy_mut().return_carrier_to_castle(carrier_slot));
        assert!(
            session
                .economy_mut()
                .step_carrier_to_castle(carrier_slot, castle_position)
        );
        assert_eq!(
            session.economy().castle().expect("loaded Castle").mana,
            7_000
        );
        let loaded_castle = session.economy().castle().expect("loaded Castle");
        assert!(
            session
                .economy()
                .player_mana()
                .saturating_add(loaded_castle.mana)
                < loaded_castle.mana_capacity
        );
        assert!(house_mana.saturating_add(loaded_castle.mana) > loaded_castle.mana_capacity);

        for &house_slot in &house_slots {
            session
                .ordinary_mut()
                .submit_house_possession(house_slot, 111, 1)
                .expect("qualified House possession mailbox");
        }

        let initial_castle_phase = session.economy().castle().expect("initial Castle").phase;
        assert_eq!(initial_castle_phase & 1, (castle_slot as u8) & 1);
        assert_ne!(initial_castle_phase & 1, 0);

        session.step_outer_turn(PlayerCommand::default(), 0);
        let possessed_slots = session
            .dispatch_events()
            .iter()
            .flat_map(|event| match event {
                Level2DispatchEvent::House { events, .. } => events
                    .iter()
                    .filter_map(|event| match event {
                        HouseStepEvent::Possessed {
                            entity_index,
                            owner_entity_index,
                            ..
                        } => Some((*entity_index, *owner_entity_index)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .map(|(source_slot, owner)| {
                assert_eq!(owner, 111);
                source_slot
            })
            .collect::<Vec<_>>();
        assert_eq!(possessed_slots, house_slots);
        // House mailboxes are consumed by this cursor, but sub_60F00's
        // source-boundary projection remains frozen for the rest of the pass.
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        let first_castle = session.economy().castle().expect("first-turn Castle");
        assert_eq!(first_castle.phase & 1, 0);
        let owner_max_before_model39 = session
            .economy()
            .owner_max_mana
            .get(&111)
            .copied()
            .expect("player max-mana projection");
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));

        let global_before_model39 = session.economy().owner_mana_global();
        let model39_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(902, sphere_position, 0, 0x5678, 1, 111)
            .expect("qualified owned model-39 negative sphere");
        let model39_slot = u16::try_from(model39_id).expect("model-39 sphere slot");
        assert_eq!(
            session.economy().owner_mana_global(),
            global_before_model39,
            "a same-pass model-39 spawn cannot publish sub_60F00's global counter early"
        );
        assert_eq!(session.economy().owner_mana_aggregate(111), 0);
        assert_eq!(
            session.economy().owner_max_mana.get(&111),
            Some(&owner_max_before_model39)
        );

        session.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(session.economy().owner_mana_aggregate(111), house_mana);
        assert_eq!(
            session.economy().owner_max_mana.get(&111),
            Some(&(owner_max_before_model39 + house_mana + 1))
        );
        assert_eq!(
            session.economy().owner_mana_global(),
            global_before_model39 + 1
        );
        let castle = session.economy().castle().expect("second-turn Castle");
        assert_eq!(castle.mana, 7_000);
        assert_eq!(castle.defender_count, 1);
        let defender_slot = castle.defender_slots[0];
        let defender = session
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == defender_slot)
            .copied()
            .expect("ordinary Castle defender");
        assert_eq!(
            (defender.source_class, defender.model, defender.action),
            (3, 3, 9)
        );
        assert_eq!(defender.target_sphere_slot, castle_slot);
        assert_ne!(defender.target_sphere_slot, model39_slot);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::Castle { source_slot, .. }
                    if *source_slot == castle_slot
            )
        }));
        let castle_dispatch_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::Castle { source_slot, .. }
                        if *source_slot == castle_slot
                )
            })
            .expect("ordinary Castle dispatch");
        let defender_dispatch_index = session
            .dispatch_events()
            .iter()
            .position(|event| {
                matches!(
                    event,
                    Level2DispatchEvent::DynamicArena {
                        source_slot,
                        kind: Level2DynamicArenaKind::CastleDefender,
                        ..
                    } if *source_slot == defender_slot
                )
            })
            .expect("ordinary Castle defender dispatch");
        assert!(castle_dispatch_index < defender_dispatch_index);

        session.reset();
        session.select_level2_campaign_entry_stage();
        let positive_house_slots = session
            .economy()
            .baseline_owner_mana_rows
            .iter()
            .filter(|row| row.source_class == HOUSE_CLASS && row.source_model == HOUSE_MODEL)
            .filter(|row| row.mana > 0)
            .map(|row| row.source_entity_index)
            .collect::<BTreeSet<_>>();
        let disabled_house = session
            .ordinary()
            .houses()
            .iter()
            .copied()
            .find(|house| house.active && positive_house_slots.contains(&house.entity_index))
            .expect("qualified positive-mana House for disable path");
        let disabled_mana = session
            .economy()
            .baseline_owner_mana_rows
            .iter()
            .find(|row| row.source_entity_index == disabled_house.entity_index)
            .expect("disabled House owner row")
            .mana;
        let baseline_global = session.economy().owner_mana_global();
        let baseline_campaign_hash = session.campaign_state_hash();
        session
            .ordinary_mut()
            .inject_house_damage(
                disabled_house.entity_index,
                disabled_house.life.wrapping_add(1),
                111,
            )
            .expect("qualified House destruction mailbox");
        session.step_outer_turn(PlayerCommand::default(), 0);
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(session.dispatch_events().iter().any(|event| {
            matches!(
                event,
                Level2DispatchEvent::House { events, .. }
                    if events.iter().any(|event| matches!(
                        event,
                        HouseStepEvent::RemoveFootprint { entity_index, .. }
                            if *entity_index == disabled_house.entity_index
                    ))
            )
        }));
        assert!(
            !session
                .ordinary()
                .houses()
                .iter()
                .find(|house| house.entity_index == disabled_house.entity_index)
                .expect("disabled House state")
                .active
        );
        assert_eq!(
            session.economy().source_owner(disabled_house.entity_index),
            0
        );
        assert!(
            !session
                .economy()
                .map_chains()
                .is_linked(disabled_house.entity_index)
        );
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .economy()
                .owner_mana_rows
                .iter()
                .any(|row| row.source_entity_index == disabled_house.entity_index)
        );
        assert_eq!(
            session.economy().owner_mana_global(),
            baseline_global - disabled_mana
        );
        assert_ne!(session.campaign_state_hash(), baseline_campaign_hash);
        session.reset();
        assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
        assert_eq!(session.economy().owner_mana_global(), baseline_global);
        assert!(
            session
                .economy()
                .owner_mana_rows
                .iter()
                .any(|row| row.source_entity_index == disabled_house.entity_index)
        );
    }

    #[test]
    fn level_two_sub5f810_uses_xyz_suppression_relation_exclusion_and_source_order() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("sub5F810 target session");
        let economy = session.economy_mut();
        let holder_position = Position {
            x: 100,
            y: 200,
            z: 300,
        };
        let spawn = |economy: &mut Level2EconomyState, position: Position, owner: u16| {
            economy
                .spawn_mana_sphere_from_death(901, position, 0, 0x1234, 1, owner)
                .expect("target sphere") as u16
        };
        let suppressed = spawn(
            economy,
            Position {
                x: 100,
                y: 200,
                z: 301,
            },
            111,
        );
        let tied_first = spawn(
            economy,
            Position {
                x: 110,
                y: 200,
                z: 300,
            },
            111,
        );
        let tied_second = spawn(
            economy,
            Position {
                x: 90,
                y: 200,
                z: 300,
            },
            111,
        );
        let wrong_player = spawn(
            economy,
            Position {
                x: 101,
                y: 200,
                z: 300,
            },
            112,
        );
        let excluded = spawn(
            economy,
            Position {
                x: 100,
                y: 200,
                z: 300,
            },
            111,
        );
        economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == suppressed)
            .expect("suppressed sphere")
            .control_flags = LEVEL2_MANA_SPHERE_TARGET_SUPPRESSION_BIT;
        economy
            .spheres
            .iter_mut()
            .find(|sphere| sphere.source_entity_index == wrong_player)
            .expect("wrong-player sphere")
            .player_entity_index = 112;
        economy.spheres.reverse();
        let expected_tie = tied_first.min(tied_second);
        assert_eq!(
            economy.select_castle_defender_sphere(holder_position, 111, excluded, 0),
            Some(expected_tie)
        );
        assert_eq!(
            economy.select_castle_defender_sphere(holder_position, 111, expected_tie, 0),
            Some(excluded)
        );
        let other_tie = if expected_tie == tied_first {
            tied_second
        } else {
            tied_first
        };
        // Excluding both the first tie winner and the exact-position row
        // leaves the other equal-distance source-order candidate.
        assert_eq!(
            economy.select_castle_defender_sphere(holder_position, 111, expected_tie, excluded),
            Some(other_tie)
        );
    }

    #[test]
    fn level_two_reset_projects_static_model39_rows_into_sub5f810_selector() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut entities = inspection.entities().to_vec();
        let source_relation = 111_u16;
        let static_position = entities[1].position;

        // The qualified fixture has no authored model-39 row yet. Reuse
        // active source slots so this test exercises the same from-parts
        // projection against lossless entity records, rather than inserting
        // a sphere directly into the typed economy vector.
        let mut static_entity = entities[1].clone();
        static_entity.class = 10;
        static_entity.model = 39;
        static_entity.entity_rng = 0x1357;
        static_entity.position = static_position;
        static_entity.yaw = -321;
        static_entity.mana = 321;
        static_entity.control_flags = 0;
        static_entity = static_entity
            .with_record_byte(152, source_relation as u8)
            .with_record_byte(153, (source_relation >> 8) as u8);
        entities[1] = static_entity;

        let player_entity = &entities[usize::from(inspection.players()[0].entity_index)];
        let player_baseline_max_mana = entities[usize::from(source_relation)].max_mana;
        let mut economy = Level2EconomyState::from_parts(
            SessionTerrain::from_inspection(&inspection),
            &entities,
            player_entity.mana,
            player_entity.model,
            0,
            inspection.players()[0].enabled_spell_holders[2],
            inspection.runtime().random_state,
        )
        .expect("static model-39 economy projection");

        assert_eq!(economy.spheres().len(), 1);
        let sphere = economy.spheres()[0];
        assert_eq!(
            (
                sphere.id,
                sphere.source_entity_index,
                sphere.entity_rng,
                sphere.position,
                sphere.yaw,
                sphere.mana,
                sphere.control_flags,
                sphere.owner_entity_index,
                sphere.player_entity_index,
            ),
            (
                1,
                1,
                0x1357,
                static_position,
                -321,
                321,
                0,
                source_relation,
                source_relation,
            )
        );
        assert_eq!(
            economy.select_castle_defender_sphere(static_position, source_relation, 0, 0),
            Some(1)
        );
        assert_eq!(
            economy.owner_max_mana.get(&source_relation),
            Some(&(player_baseline_max_mana + 321)),
            "sub_61000 adds static model-39 mana to the linked player's maxMana"
        );

        let baseline_hash = economy.reset_hash();
        economy.spheres[0].mana = 999;
        assert_ne!(economy.reset_hash(), baseline_hash);
        economy.reset();
        assert_eq!(economy.spheres().len(), 1);
        assert_eq!(economy.spheres()[0].mana, 321);
        assert_eq!(economy.reset_hash(), baseline_hash);
    }

    #[test]
    fn level_two_castle_defender_arbitration_tracks_nearest_sphere_and_aggregate() {
        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 package");
        let mut session = inspection
            .level2_reset_session()
            .expect("defender arbitration session");
        let castle_slot = session
            .economy_mut()
            .build_castle(Position::default(), 111)
            .expect("defender arbitration Castle");
        {
            let economy = session.economy_mut();
            let castle = economy
                .castle
                .as_mut()
                .expect("defender arbitration Castle");
            castle.level = 1;
            castle.phase = 1;
            castle.mana = 0;
            castle.mana_capacity = 10_000;
        }
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let defender_slot = session
            .economy()
            .castle()
            .expect("defender arbitration Castle")
            .defender_slots[0];
        {
            let economy = session.economy_mut();
            let defender = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            defender.mana = 7;
            defender.max_mana = 100;
        }
        let sphere_id = session
            .economy_mut()
            .spawn_mana_sphere_from_death(
                77,
                Position {
                    x: 256,
                    y: 128,
                    z: 0,
                },
                0,
                0x1234,
                50,
                111,
            )
            .expect("nearest defender sphere");
        session.economy_mut().maintain_castle_defenders(castle_slot);
        let castle = session
            .economy()
            .castle()
            .expect("defender arbitration Castle");
        assert_eq!(
            (castle.defender_total_mana, castle.defender_total_max_mana),
            (7, 100)
        );
        assert_eq!(
            castle.defender_target_spheres[0],
            u16::try_from(sphere_id).unwrap()
        );
        let sphere_slot = u16::try_from(sphere_id).unwrap();
        {
            let economy = session.economy_mut();
            economy
                .castle
                .as_mut()
                .expect("defender arbitration Castle")
                .mana_capacity = 10_000;
            let defender = economy
                .castle_children
                .iter_mut()
                .find(|child| child.source_entity_index == defender_slot)
                .expect("model-3 defender");
            defender.mana = defender.max_mana;
            defender.target_sphere_slot = 0;
            economy.maintain_castle_defenders(castle_slot);
        }
        assert_eq!(
            session
                .economy()
                .castle()
                .expect("defender arbitration Castle")
                .defender_target_spheres[0],
            castle_slot
        );
        assert_eq!(
            session
                .economy()
                .castle()
                .expect("defender arbitration Castle")
                .defender_total_mana,
            100
        );
        assert!(
            session
                .economy()
                .spheres()
                .iter()
                .any(|sphere| sphere.source_entity_index == sphere_slot)
        );
    }
