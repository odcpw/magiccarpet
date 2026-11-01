#[test]
fn level_two_type_str_164_auxiliary_materializes_and_reset_replays() {
    let inspection =
        NativeWorld::inspect_level2(&level_two_fixture_path()).expect("qualified Level 2 package");
    let player = &inspection.players()[0];
    let auxiliary = player
        .type_str_164_auxiliary()
        .expect("Level 2 player carries the source Type_str_164 extension");
    assert_eq!(auxiliary.byte_0x154_340, 1);
    assert_eq!(auxiliary.dword_0x16D_365, 1_999);
    assert_ne!(auxiliary.byte_0x154_340, 0);
    assert_ne!(auxiliary.dword_0x16D_365, 0);
    assert_eq!(player.record_bytes().len(), PLAYER_LEVEL2_RECORD_BYTES);
    assert_eq!(player.banishment_state_0x006, Some(1));
    assert_eq!(
        inspection
            .players()
            .iter()
            .map(|row| row.record_bytes()[PLAYER_LEVEL2_BANISHMENT_STATE_OFFSET])
            .collect::<Vec<_>>(),
        vec![1, 1, 0, 0, 0, 0, 0, 0]
    );
    for row in inspection.players() {
        assert_eq!(row.active_player_index, 31, "player slot {}", row.slot);
        assert_eq!(
            &row.record_bytes()[6..8],
            &31_i16.to_le_bytes(),
            "player slot {} compact ActPlayerIndex bytes",
            row.slot
        );
    }
    let mut encoded = Vec::with_capacity(TYPE_STR_164_AUXILIARY_BYTES);
    for value in auxiliary.word_0x26_38 {
        encoded.extend_from_slice(&value.to_le_bytes());
    }
    encoded.push(auxiliary.byte_0x154_340);
    encoded.extend_from_slice(&auxiliary.dword_0x16D_365.to_le_bytes());
    encoded.extend_from_slice(&auxiliary.word_0x1A2_418.to_le_bytes());
    for value in auxiliary.array_0x1FC_508 {
        encoded.extend_from_slice(&value.to_le_bytes());
    }
    encoded.extend_from_slice(&auxiliary.word_0x242_578.to_le_bytes());
    assert_eq!(encoded.len(), TYPE_STR_164_AUXILIARY_BYTES);
    assert_eq!(
        &player.record_bytes()[PLAYER_LEGACY_RECORD_BYTES..PLAYER_LEVEL2_V4_RECORD_BYTES],
        encoded.as_slice()
    );
    assert_eq!(
        player.record_bytes()[PLAYER_LEVEL2_BANISHMENT_STATE_OFFSET],
        player.banishment_state_0x006.expect("v5 state carrier")
    );

    let mut session = inspection
        .level2_reset_session()
        .expect("qualified Level 2 session");
    let baseline_hash = session.campaign_state_hash();
    let baseline_record = session.shared_spells().players[0].record_bytes().to_vec();
    let baseline_auxiliary = session.shared_spells().players[0]
        .type_str_164_auxiliary()
        .copied();
    let baseline_banishment_state = session.players[0].banishment_state_0x006;
    session.shared_spells_mut().players[0].record[PLAYER_LEGACY_RECORD_BYTES] ^= 1;
    assert_ne!(session.campaign_state_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(
        session.shared_spells().players[0].record_bytes(),
        baseline_record
    );
    assert_eq!(
        session.shared_spells().players[0].type_str_164_auxiliary(),
        baseline_auxiliary.as_ref()
    );
    session.players[0].banishment_state_0x006 = Some(0);
    assert_ne!(session.campaign_state_hash(), baseline_hash);
    session.reset();
    assert_eq!(session.campaign_state_hash(), baseline_hash);
    assert_eq!(
        session.players[0].banishment_state_0x006,
        baseline_banishment_state
    );
}

#[test]
fn campaign_player_hands_accept_independent_empty_selectors_and_reject_corrupt_values() {
    let original = fs::read(level_two_fixture_path().join("players.bin")).unwrap();
    for profile in [OwnerPackageProfile::Level2, OwnerPackageProfile::Campaign] {
        for (left, right) in [(-1_i16, 0_i16), (2, -1), (-1, -1), (0, 1)] {
            let mut bytes = original.clone();
            bytes[12 + 554..12 + 556].copy_from_slice(&left.to_le_bytes());
            bytes[12 + 556..12 + 558].copy_from_slice(&right.to_le_bytes());
            let players = parse_players(&bytes, profile, OwnerPackageSchema::V5).unwrap();
            assert_eq!(players[0].left_spell_slot_raw, left);
            assert_eq!(players[0].right_spell_slot_raw, right);
        }
        for invalid in [-2_i16, PLAYER_SPELL_COUNT_I16] {
            for offset in [554, 556] {
                let mut bytes = original.clone();
                bytes[12 + offset..12 + offset + 2].copy_from_slice(&invalid.to_le_bytes());
                assert!(parse_players(&bytes, profile, OwnerPackageSchema::V5).is_err());
            }
        }
    }
    let (package, _) = fixture_paths();
    let mut sealed = fs::read(package.join("players.bin")).unwrap();
    sealed[12 + 554..12 + 556].copy_from_slice(&(-1_i16).to_le_bytes());
    assert!(parse_players(&sealed, OwnerPackageProfile::Level1, OwnerPackageSchema::V2).is_err());
}

#[test]
fn level_two_type_str_164_rejects_malformed_record_and_descriptor_inputs() {
    let package = level_two_fixture_path();
    let original = fs::read(package.join("players.bin")).unwrap();

    let truncated = &original[..original.len() - 1];
    assert!(
        parse_players(
            truncated,
            OwnerPackageProfile::Level2,
            OwnerPackageSchema::V5
        )
        .is_err()
    );

    let mut wrong_version = original.clone();
    wrong_version[4..6].copy_from_slice(&2_u16.to_le_bytes());
    assert!(
        parse_players(
            &wrong_version,
            OwnerPackageProfile::Level2,
            OwnerPackageSchema::V5
        )
        .is_err()
    );

    let mut wrong_record_size = original.clone();
    wrong_record_size[6..8].copy_from_slice(&573_u16.to_le_bytes());
    assert!(
        parse_players(
            &wrong_record_size,
            OwnerPackageProfile::Level2,
            OwnerPackageSchema::V5
        )
        .is_err()
    );

    let temp = TempLevel2V4Package::copy_from(&package);
    let manifest_path = temp.package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["record_layouts"]["player"]["type_str_164_auxiliary"]["record_bytes"] =
        json!(TYPE_STR_164_AUXILIARY_BYTES - 1);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let error = NativeWorld::inspect_level2(&temp.package)
        .expect_err("wrong Type_str_164 descriptor extent must fail closed");
    assert!(error.to_string().contains("descriptor layout"));
}

#[test]
fn level_two_public_castle_relation_selects_strict_shift_aabb_upkeep() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let configure = |session: &mut Mc2Simulation, player_height: i16, boost: bool| {
        let player_index = session.state.entity_index;
        let root_slot = session
            .economy_mut()
            .build_castle(
                Position {
                    x: 8_192,
                    y: 8_192,
                    z: 0,
                },
                player_index,
            )
            .expect("live Castle relation");
        let root_position = session
            .economy()
            .castle()
            .expect("installed Castle root")
            .position;
        {
            let mut economy_view = session.economy_mut();
            let castle = economy_view.castle.as_mut().expect("installed Castle root");
            castle.shift_yaw = -8_192;
            castle.shift_pitch = 1_664;
            castle.shift_roll = 1_664;
            castle.shift_fov = 0x4000;
        }
        let player_position = Position {
            x: root_position.x,
            y: root_position.y,
            z: root_position.z.wrapping_add(player_height),
        };
        session.state.position = player_position;
        let set_player_extents = |entity: &mut EntitySnapshot| {
            entity.record[86..88].copy_from_slice(&100_i16.to_le_bytes());
            entity.record[88..90].copy_from_slice(&121_i16.to_le_bytes());
            entity.record[90..92].copy_from_slice(&121_i16.to_le_bytes());
            entity.record[92..94].copy_from_slice(&100_i16.to_le_bytes());
            entity.set_position(player_position);
            entity.control_flags = if boost { 0x0000_1000 } else { 0 };
            entity.record[16..20].copy_from_slice(&entity.control_flags.to_le_bytes());
        };
        let live = &mut session.session.state.live;
        let shared = &mut live.shared_spells;
        shared.player_entity.mana = 1_000;
        shared.player_entity.mana_regen = 100;
        shared.player_entity.life = 1_000;
        shared.player_entity.record[148..152].copy_from_slice(&1_000_i32.to_le_bytes());
        shared.player_entity.record[140..144].copy_from_slice(&100_i32.to_le_bytes());
        set_player_extents(&mut shared.player_entity);
        shared.player_entities[0] = shared.player_entity.clone();
        shared.caster.mana = 1_000;
        shared.caster.control_flags = shared.player_entity.control_flags;
        shared.player_mana = 1_000;
        set_player_extents(
            live.economy
                .entities
                .get_mut(usize::from(player_index))
                .expect("local player entity"),
        );
        live.player_life = 1_000;
        shared.player_life_regen = 100;
        (
            root_slot,
            shared.player_entity.max_mana,
            shared.player_entity.max_life,
        )
    };

    let mut on_castle = inspection
        .level2_reset_session()
        .expect("on-Castle upkeep session");
    let (on_root_slot, on_max_mana, on_max_life) = configure(&mut on_castle, 1_000, false);
    let on_player = on_castle
        .economy
        .entities
        .iter()
        .find(|entity| entity.index == on_castle.state.entity_index)
        .expect("on-Castle player entity");
    assert_eq!(
        on_castle.economy().player_castle_entity_index(),
        on_root_slot
    );
    assert!(
        on_castle
            .economy()
            .player_castle_shift_aabb_overlap(on_player)
    );
    on_castle.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        on_castle.shared_spells().player_entity.mana_regen,
        (on_max_mana / 200).max(1_000)
    );
    assert_eq!(
        on_castle.shared_spells().player_life_regen,
        on_max_life / 250
    );
    assert_eq!(
        u16::from_le_bytes(
            on_castle.shared_spells().local_player.record_bytes()[345..347]
                .try_into()
                .expect("Castle word159")
        ),
        1
    );

    let mut boosted = inspection
        .level2_reset_session()
        .expect("Castle boost-clearing session");
    let (boosted_root_slot, boosted_max_mana, boosted_max_life) =
        configure(&mut boosted, 20_000, true);
    assert_eq!(
        boosted.economy().player_castle_entity_index(),
        boosted_root_slot
    );
    boosted.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        boosted.shared_spells().player_entity.control_flags & 0x1000,
        0
    );
    assert_eq!(
        boosted.shared_spells().player_entity.mana_regen,
        (boosted_max_mana / 200).max(1_000)
    );
    assert_eq!(
        boosted.shared_spells().player_life_regen,
        boosted_max_life / 250
    );
    assert_eq!(
        u16::from_le_bytes(
            boosted.shared_spells().local_player.record_bytes()[345..347]
                .try_into()
                .expect("boost-only word159")
        ),
        0
    );

    on_castle.state.position.z = on_castle.state.position.z.wrapping_add(20_000);
    on_castle.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        u16::from_le_bytes(
            on_castle.shared_spells().local_player.record_bytes()[345..347]
                .try_into()
                .expect("first outdoor word159 countdown")
        ),
        0
    );

    let mut off_castle = inspection
        .level2_reset_session()
        .expect("off-Castle upkeep session");
    let (_off_root_slot, off_max_mana, off_max_life) = configure(&mut off_castle, 20_000, false);
    let off_player = off_castle
        .economy
        .entities
        .iter()
        .find(|entity| entity.index == off_castle.state.entity_index)
        .expect("off-Castle player entity");
    assert!(
        !off_castle
            .economy()
            .player_castle_shift_aabb_overlap(off_player)
    );
    off_castle.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        off_castle.shared_spells().player_entity.mana_regen,
        (off_max_mana / 2_000).max(100)
    );
    assert_eq!(
        off_castle.shared_spells().player_life_regen,
        off_max_life / 2_000
    );
    assert_eq!(
        u16::from_le_bytes(
            off_castle.shared_spells().local_player.record_bytes()[345..347]
                .try_into()
                .expect("outdoor word159")
        ),
        0
    );
}

#[test]
fn level_two_bad_stones_reconstruct_action9_map_chain_and_presentation() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 reset session with bad stones");
    let expected_cells = [(2, 203), (2, 103), (51, 154), (208, 154)];
    let expected_hashes = LEVEL2_BAD_STONE_RECORD_SHA256;
    let baseline_hash = session.bad_stone_hash();
    assert_eq!(session.bad_stones().len(), 4);
    assert_eq!(
        session
            .bad_stone_presentation(33)
            .map(|bytes| sha256_hex(bytes)),
        Some("2c3dfa0bc2870d8d1f2c9aa203f9ca6f19363f5411f515627bf43437397f0f78".to_owned())
    );
    assert_eq!(
        session
            .bad_stone_presentation(33)
            .expect("bad-stone presentation"),
        &[
            0, 0, 56, 0, 5, 0, 22, 0, 5, 0, 0, 7, 0, 0, 252, 255, 0, 1, 0, 2, 255, 255, 255, 255,
            0, 12, 50, 0, 0, 16, 0, 2, 0, 0,
        ]
    );
    assert_eq!(
        session.bad_stone_particle_presentation(33),
        Some(&ParticleParameter {
            base_tmap: 198,
            word_2: 8_492,
            word_4: 42,
            speed: 138,
            rotation_speed: 400,
            byte_10: 0,
            byte_11: 1,
            render_mode: 0,
            byte_13: 0,
        })
    );
    assert_eq!(session.bad_stone_sprite_selector(33), Some(0));
    assert_eq!(
        session.bad_stone_sprite_disposition(33).unwrap(),
        SourceSpriteTemplateDisposition::Render(crate::source_sprite_tmap::SourceSpriteTemplate {
            render_parameter_index: 270,
            base_tmap: 198,
            render_mode: 0,
            logical_height: 42,
            animation_owner: crate::source_sprite_tmap::SourceSpriteAnimationOwner::SourceEntity,
            palette_owner: crate::source_sprite_tmap::SourceSpritePaletteOwner::NightLevel,
            shadow_eligible: true,
            reflection_eligible: true,
        })
    );
    for (ordinal, stone) in session.bad_stones().iter().enumerate() {
        let entity = stone.entity();
        assert_eq!(entity.index, LEVEL2_BAD_STONE_SLOTS[ordinal]);
        assert_eq!((entity.class, entity.model, entity.action), (2, 3, 9));
        assert_eq!(entity.max_life, 300);
        assert_eq!(entity.life, 300);
        assert_eq!(entity.source_word_10(), entity.index % 11);
        assert_eq!(entity.particle_parameter_index, Some(270));
        assert_eq!(entity.parameter_relation, Some(59));
        assert_eq!(
            entity.special_relation,
            EntitySpecialRelation::GlobalSpecialState
        );
        assert_eq!(entity.old_map_entity(), 0);
        assert_eq!(entity.next_entity(), 0);
        assert_eq!(stone.source_record_sha256(), expected_hashes[ordinal]);
        assert_eq!(stone.map_cell(), expected_cells[ordinal]);
        let map_index = terrain_index(stone.map_cell().0, stone.map_cell().1);
        assert_eq!(
            session.canonical_terrain().planes().entity_indices[map_index],
            entity.index as i16
        );
        assert_eq!(
            entity.position.z,
            FlightWorld::ground_height(session.canonical_terrain(), entity.position)
        );
    }
    let terrain_hash = session.terrain_hash();
    let mission_hash = session.mission().state_hash();
    let baseline_phase = session.bad_stones()[0].entity.phase;
    let expected_z = FlightWorld::ground_height(
        session.canonical_terrain(),
        session.bad_stones()[0].entity.position,
    );
    {
        let stone = &mut session.bad_stones[0];
        stone.entity.position.z = expected_z.wrapping_add(1);
        stone.entity.record[84..86].copy_from_slice(&stone.entity.position.z.to_le_bytes());
        stone.entity.control_flags &= !0x0002_0000;
        stone.entity.record[18] &= !2;
    }
    let step = session
        .step_bad_stone_action_9(33)
        .expect("action-9 source dispatch");
    assert_eq!(step.source_slot, 33);
    assert_eq!(step.position.z, expected_z);
    assert_ne!(step.position.z, step.previous_position.z);
    assert_eq!(step.previous_control_flags & 0x0002_0000, 0);
    assert_ne!(step.control_flags & 0x0002_0000, 0);
    assert_eq!(step.phase, step.previous_phase.wrapping_add(1));
    assert_ne!(session.bad_stone_hash(), baseline_hash);
    assert_eq!(session.terrain_hash(), terrain_hash);
    assert_eq!(session.mission().state_hash(), mission_hash);
    assert!(session.step_bad_stone_action_9(32).is_err());
    session.reset();
    assert_eq!(session.bad_stone_hash(), baseline_hash);
    assert_eq!(session.bad_stones()[0].entity().phase, baseline_phase);
}

#[test]
fn level_two_bad_stone_constructor_rejects_wrong_model_action_parameter_and_cross_level_rows() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let stone = inspection.entities()[33].clone();
    let planes = inspection.terrain_planes();
    let mut wrong_model = stone.clone();
    wrong_model.model = 2;
    assert!(Level2BadStone::from_entity(&wrong_model, planes).is_err());
    let mut wrong_action = stone.clone();
    wrong_action.action = 6;
    assert!(Level2BadStone::from_entity(&wrong_action, planes).is_err());
    let mut wrong_parameter = stone.clone();
    wrong_parameter.parameter_relation = Some(60);
    assert!(Level2BadStone::from_entity(&wrong_parameter, planes).is_err());
    let dolmen = inspection.entities()[108].clone();
    assert!(Level2BadStone::from_entity(&dolmen, planes).is_err());
}

#[test]
fn level_two_authored_subtype45_runs_action51_and_reclaims_at_following_boundary() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 model-45/action-51 session");
    let source = session
        .economy_mut()
        .allocate_authored_model45(
            153,
            22,
            21,
            Position {
                x: 0x1234,
                y: 0x5678,
                z: 0,
            },
        )
        .expect("group-8 authored subtype-45 source");
    let source_slot = source.source_entity_index;
    assert_eq!(source.position.x, 0x1200);
    assert_eq!(source.position.y, 0x5600);
    assert_eq!(
        (
            source.source_class,
            source.model,
            source.action,
            source.max_life,
            source.life,
            source.actual_speed,
            source.minimum_speed,
            source.subspell_index,
            source.dword_0x10,
            source.font_type_index,
        ),
        (10, 45, 51, 30, 30, 16, 56, 6, 2, 54)
    );
    assert_eq!(
        (
            source.source_byte_38,
            source.control_flags,
            source.mana,
            source.max_mana,
            source.particle_index,
            source.particle_byte_0x5d,
            source.byte_0x46,
            source.parent_entity_index,
        ),
        (33, 9, 0, 0, 177, 1, 21, 0)
    );
    assert_eq!(
        (
            source.view_pitch,
            source.view_yaw,
            source.view_fov,
            source.view_roll
        ),
        (2560, 0, 256, 2560)
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let hidden = session
        .economy()
        .authored_model45()
        .iter()
        .find(|row| row.source_entity_index == source_slot)
        .expect("model-45 row remains allocated through its callback pass");
    assert!(hidden.active);
    assert!(!hidden.drawing_enabled);
    assert_eq!(hidden.source_byte_3e, source.source_byte_3e.wrapping_add(1));
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&source_slot)
    );
    assert!(!session.economy().free_slots.contains(&source_slot));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .authored_model45()
            .iter()
            .all(|row| row.source_entity_index != source_slot)
    );
}

#[test]
fn level_two_authored_class14_rows_dispatch_source_terrain_callbacks() {
    let mut session = level_two_campaign_session();
    let baseline_hash = session.economy().reset_hash();

    // The pinned corpus contains one subtype-3 row (244) and subtype-5
    // rows (248+).  Materialize one of each through the explicit
    // constructor seam; group-trigger callbacks are outside this check.
    let model3 = session
        .allocate_level2_authored_class14(244)
        .expect("authored subtype-3 row");
    let model5 = session
        .allocate_level2_authored_class14(248)
        .expect("authored subtype-5 row");
    assert_eq!(
        (
            model3.model,
            model3.action,
            model3.particle,
            model3.control_flags,
            model3.shift_rot_x,
            model3.shift_rot_y,
        ),
        (3, 8, 338, 1, 0, 0)
    );
    assert_eq!(
        (
            model5.model,
            model5.action,
            model5.particle,
            model5.control_flags,
            model5.shift_rot_x,
            model5.shift_rot_y,
        ),
        (5, 10, 280, 0, 768, 1280)
    );
    assert_eq!(
        session.economy().class14_model3_handle(),
        model3.source_entity_index
    );
    assert_ne!(model3.source_entity_index, model5.source_entity_index);
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&model3.source_entity_index)
    );
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&model5.source_entity_index)
    );

    // Deliberately perturb the live rows after their source-backed
    // allocation. A cursor implementation that only resynchronizes the
    // map, but does not dispatch the pinned model-3/model-5 callbacks,
    // would retain these incorrect heights.
    let model3_ground = session.canonical_terrain().ground_height(model3.position);
    let model5_ground = session.canonical_terrain().ground_height(model5.position);
    session
        .economy_mut()
        .authored_class14
        .iter_mut()
        .find(|source| source.source_entity_index == model3.source_entity_index)
        .expect("model-3 row remains allocated")
        .position
        .z = model3_ground.wrapping_add(512);
    session
        .economy_mut()
        .authored_class14
        .iter_mut()
        .find(|source| source.source_entity_index == model5.source_entity_index)
        .expect("model-5 row remains allocated")
        .position
        .z = model5_ground.wrapping_add(512);
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredClass14,
            source_slot,
            ..
        } if *source_slot == model3.source_entity_index
    )));
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::AuthoredClass14,
            source_slot,
            ..
        } if *source_slot == model5.source_entity_index
    )));
    let after_turn = session.economy().authored_class14();
    let model3_after = after_turn
        .iter()
        .find(|source| source.source_entity_index == model3.source_entity_index)
        .expect("model-3 row remains live after terrain callback");
    let model5_after = after_turn
        .iter()
        .find(|source| source.source_entity_index == model5.source_entity_index)
        .expect("model-5 row remains live after terrain callback");
    assert_eq!(model3_after.position.z, model3_ground);
    assert_eq!(model5_after.position.z, model5_ground);
    assert!(model3_after.active);
    assert!(model5_after.active);

    session.reset();
    assert!(session.economy().authored_class14().is_empty());
    assert_eq!(session.economy().reset_hash(), baseline_hash);
}

#[test]
fn level_two_authored_class14_model5_collects_for_local_player_and_reclaims() {
    let mut session = level_two_campaign_session();
    let source = session
        .allocate_level2_authored_class14(248)
        .expect("authored subtype-5 row");
    let source_slot = source.source_entity_index;
    let source_phase = source.phase;
    let scroll_position = source.position;
    session
        .shared_spells_mut()
        .acquire_level2_spell_holder(12, PhysicalSpellActionRole::OrdinaryPickup, true)
        .expect("source-shaped family-12 acquisition");
    let experience_before = *session.shared_spells().spell_experience();
    let enabled_before = session.shared_spells().players[0].enabled_spell_holders;
    session.state.position = scroll_position;

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert!(
        session
            .shared_spells()
            .pickup_events()
            .iter()
            .any(|event| matches!(
                event,
                Level2SharedSpellEvent::ExperienceScrollCollected {
                    source_slot: observed,
                    player_entity_index,
                } if *observed == source_slot && *player_entity_index == session.state.entity_index
            ))
    );
    assert!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .any(|event| matches!(event, FlightEvent::PrepareSound { sound_id: 63, .. }))
    );
    for family in 0..PLAYER_SPELL_COUNT {
        let expected = if enabled_before[family] == 0 {
            experience_before[family]
        } else if family == usize::from(CASTLE_SPELL_FAMILY) {
            experience_before[family]
                .saturating_add(LEVEL2_EXPERIENCE_SCROLL_AWARD)
                .min(7)
        } else {
            experience_before[family].saturating_add(LEVEL2_EXPERIENCE_SCROLL_AWARD)
        };
        assert_eq!(session.shared_spells().spell_experience()[family], expected);
    }
    let hidden = session
        .economy()
        .authored_class14()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .copied()
        .expect("collected scroll remains allocated through its callback pass");
    assert!(hidden.active);
    assert!(!hidden.drawing_enabled);
    assert_eq!(hidden.phase, source_phase.wrapping_add(1));
    assert!(session.economy().map_chains().is_linked(source_slot));
    assert!(!session.economy().free_slots.contains(&source_slot));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .economy()
            .authored_class14()
            .iter()
            .any(|source| source.source_entity_index == source_slot)
    );
    assert!(!session.economy().map_chains().is_linked(source_slot));
    assert!(session.economy().free_slots.contains(&source_slot));
}

#[test]
fn level_two_authored_class14_model5_uses_strict_collision_boundaries() {
    let mut session = level_two_campaign_session();
    let source = session
        .allocate_level2_authored_class14(248)
        .expect("authored subtype-5 row");
    let scroll_position = source.position;
    let before = *session.shared_spells().spell_experience();
    let x_boundary = scroll_position.x.wrapping_add(
        (LEVEL2_SCROLL_COLLISION_HALF_WIDTH + LEVEL2_PLAYER_COLLISION_HALF_WIDTH).cast_unsigned(),
    );
    session.state.position = Position {
        x: x_boundary,
        y: scroll_position.y,
        z: scroll_position.z,
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(*session.shared_spells().spell_experience(), before);
    assert!(
        session
            .economy()
            .authored_class14()
            .iter()
            .any(|row| row.authored_index == 248 && row.active)
    );
}

#[test]
fn level_two_authored_class14_model5_scans_past_remote_collision_for_local_xp() {
    let mut session = level_two_campaign_session();
    let source = session
        .allocate_level2_authored_class14(248)
        .expect("authored subtype-5 row");
    session
        .shared_spells_mut()
        .acquire_level2_spell_holder(12, PhysicalSpellActionRole::OrdinaryPickup, true)
        .expect("source-shaped family-12 acquisition");
    let source_slot = source.source_entity_index;
    let scroll_position = source.position;
    let remote_entity_index = session
        .shared_spells()
        .players
        .iter()
        .find(|player| player.slot == 1)
        .expect("remote Level 2 player")
        .entity_index;
    let remote = session
        .economy
        .entities
        .iter_mut()
        .find(|entity| entity.index == remote_entity_index)
        .expect("remote player entity");
    remote.position = scroll_position;
    // The reset package's remote player is a model-1 source row. The
    // source scroll callback only admits model-0 players, so this
    // discriminating candidate is normalized to the live model-0 shape
    // before the source-order scan.
    remote.model = 0;
    session.state.position = scroll_position;
    // The owner package orders local then remote. Reversing only the
    // player cursor makes the remote overlap the first candidate and
    // proves the source scan does not return before the local player.
    session.shared_spells_mut().players.swap(0, 1);
    let before = session.shared_spells().spell_experience()[12];

    session.step_outer_turn(PlayerCommand::default(), 0);

    assert_eq!(
        session.shared_spells().spell_experience()[12],
        before + LEVEL2_EXPERIENCE_SCROLL_AWARD
    );
    assert!(
        session
            .shared_spells()
            .pickup_events()
            .iter()
            .any(|event| matches!(
                event,
                Level2SharedSpellEvent::ExperienceScrollCollected {
                    source_slot: observed,
                    player_entity_index,
                } if *observed == source_slot && *player_entity_index == remote_entity_index
            ))
    );
    let hidden = session
        .economy()
        .authored_class14()
        .iter()
        .find(|source| source.source_entity_index == source_slot)
        .expect("collected remote scroll remains until the following prologue");
    assert!(!hidden.drawing_enabled);
    assert!(session.economy().map_chains().is_linked(source_slot));
    assert!(!session.economy().free_slots.contains(&source_slot));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().free_slots.contains(&source_slot));
    assert!(!session.economy().map_chains().is_linked(source_slot));
}

#[test]
fn level_two_authored_model13_runs_source_callback_and_spawn_branch() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 authored model-13 session");
    let baseline_hash = session.campaign_state_hash();

    let source = session
        .allocate_level2_authored_model13(365)
        .expect("authored model-13 row 365");
    assert_eq!(
        (
            source.entity_id,
            source.source_class,
            source.model,
            source.action,
            source.subspell_index,
            source.max_life,
            source.life,
            source.particle_index,
            source.control_flags,
            source.shift_rot_x,
            source.shift_rot_y,
        ),
        (
            source.source_entity_index,
            10,
            19,
            19,
            200,
            240,
            240,
            228,
            0x0002_0001,
            512,
            512,
        )
    );

    // Pick a source RNG whose first SEARCH.DAT cell takes the source's
    // emitting branch, then force life=2 so the post-decrement life is
    // odd and the source emits its four yaw phases.
    let seed = (1..=u16::MAX)
        .find(|candidate| level2_next_rng16(*candidate) % 0x9d >= 79)
        .expect("a model-13 emitting RNG seed exists");
    let mut economy_view = session.economy_mut();
    let parent = economy_view
        .authored_model13
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .expect("model-13 parent remains live");
    parent.life = 2;
    parent.entity_rng = seed;
    assert!(
        session
            .economy_mut()
            .step_authored_model13_slot(source.source_entity_index)
    );
    let stepped_parent = session
        .economy()
        .authored_model13()
        .iter()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .copied()
        .expect("model-13 parent survives first callback");
    assert_eq!(stepped_parent.life, 1);
    assert_eq!(stepped_parent.dword_0x10, 0);
    let child_count = session.economy().model13_spawned_children().len();
    assert!(child_count >= 4 && child_count % 4 == 0);
    assert!(
        session
            .economy()
            .model13_spawned_children()
            .iter()
            .all(|child| {
                child.parent_entity_index == source.source_entity_index
                    && child.parent_entity_id == source.entity_id
                    && (child.model, child.action, child.source_class) == (14, 14, 10)
                    && child.max_life == child.life
                    && child.entity_index == 9
                    && child.subtype == 14
            })
    );
    let child = session.economy().model13_spawned_children()[0];
    assert!(
        session
            .economy_mut()
            .step_model13_spawned_child_slot(child.source_entity_index)
    );
    let stepped_child = session
        .economy()
        .model13_spawned_children()
        .iter()
        .find(|candidate| candidate.source_entity_index == child.source_entity_index)
        .copied()
        .expect("model-13 child survives first callback");
    assert_eq!(
        stepped_child.actual_speed,
        child.actual_speed.saturating_sub(4).max(64)
    );
    assert!(stepped_child.position.z >= child.position.z);

    let mut exhausted = session.economy().clone();
    exhausted.free_slots.clear();
    for source in &mut exhausted.authored_model13 {
        source.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    for child in &mut exhausted.model13_spawned_children {
        child.control_flags &= !SOURCE_REPLACEABLE_FLAG;
    }
    let before_exhaustion = exhausted.clone();
    let terrain = session.canonical_terrain().clone();
    assert!(
        exhausted
            .allocate_authored_model13(366, Position::default(), &terrain)
            .is_none()
    );
    assert_eq!(exhausted, before_exhaustion);

    session
        .economy_mut()
        .authored_model13
        .iter_mut()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .expect("model-13 parent for expiry")
        .life = -1;
    assert!(
        session
            .economy_mut()
            .step_authored_model13_slot(source.source_entity_index)
    );
    let expired = session
        .economy()
        .authored_model13()
        .iter()
        .find(|candidate| candidate.source_entity_index == source.source_entity_index)
        .copied()
        .expect("model-13 terminal row remains hidden in the source arena");
    assert!(expired.active);
    assert!(!expired.drawing_enabled);
    session.economy_mut().reclaim_inactive_slots();
    assert!(
        session
            .economy()
            .authored_model13()
            .iter()
            .any(|candidate| candidate.source_entity_index == source.source_entity_index)
    );

    session.reset();
    assert!(session.economy().authored_model13().is_empty());
    assert!(session.economy().model13_spawned_children().is_empty());
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_castle_rebuild_discards_reclaimed_source_rng() {
    // Pinned remc2 NewEvent_4A050 overwrites rand_0x14 on every fresh
    // allocation. A reclaimed Castle must not resurrect its old source
    // RNG when the same arena slot is reused.
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Castle rebuild session");
    let slot = session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("initial Castle allocation");
    session.economy_mut().source_rng.insert(slot, 0xdead);
    session.economy_mut().free_slots.clear();
    session
        .economy_mut()
        .castle
        .as_mut()
        .expect("initial Castle")
        .active = false;
    session.economy_mut().reclaim_inactive_slots();
    assert!(!session.economy().source_rng.contains_key(&slot));
    let rebuilt = session
        .economy_mut()
        .build_castle(Position::default(), 111)
        .expect("same-slot Castle rebuild");
    assert_eq!(rebuilt, slot);
    assert_eq!(
        session.economy().source_rng.get(&slot),
        Some(&(slot.wrapping_add(session.economy().global_rng as u16)))
    );
}

#[test]
fn level_two_outer_turn_rescans_same_pass_castle_children() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    let footprint = inspection
        .level2_castle_stage_footprints
        .get(&1)
        .cloned()
        .expect("qualified stage-1 BUILD footprint");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 dynamic cursor session");
    session.select_level2_campaign_entry_stage();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let castle_slot = session
        .economy_mut()
        .build_castle_with_footprint(Position::default(), 111, &footprint)
        .expect("source constructor Castle allocation");
    let free_after_root_build = session.economy().free_slots.len();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let events = session.dispatch_events();
    let root_event = events
        .iter()
        .position(|event| {
            matches!(
                event,
                Mc2DispatchEvent::Castle {
                    source_slot,
                    ..
                } if *source_slot == castle_slot
            )
        })
        .expect("same-pass Castle root event");
    let child_slot = events
        .iter()
        .find_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::CastleChild,
                ..
            } => Some(*source_slot),
            _ => None,
        })
        .expect("same-pass construction child event");
    let child_event = events
        .iter()
        .position(|event| {
            matches!(
                event,
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::CastleChild,
                    ..
                } if *source_slot == child_slot
            )
        })
        .expect("same-pass construction child event");
    let controller_event = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::CastleController,
                ..
            } => Some((index, *source_slot)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(controller_event.len(), 1);
    let (controller_event_index, controller_slot) = controller_event[0];
    assert!(
        root_event < child_event,
        "child must be visible on the runagain scan"
    );
    assert!(child_event < controller_event_index);
    assert!(!session.economy().free_slots.contains(&castle_slot));
    assert!(session.economy().castle_children().iter().any(|candidate| {
        candidate.active
            && candidate.source_entity_index == child_slot
            && candidate.source_class == 10
            && candidate.model == 42
            && candidate.action == 44
    }));
    assert!(!session.economy().free_slots.contains(&child_slot));
    assert!(
        !session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&child_slot)
    );
    let live_build_children = session
        .economy()
        .castle_children()
        .iter()
        .filter(|child| {
            child.active
                && child.source_entity_index == child_slot
                && child.source_class == 10
                && child.model == 42
                && child.action == 44
                && child.parent_entity_index == castle_slot
        })
        .count();
    assert_eq!(live_build_children, 1);
    let live_controllers = session
        .economy()
        .castle_children()
        .iter()
        .filter(|child| {
            child.active
                && child.source_entity_index == controller_slot
                && child.source_class == 10
                && child.model == 79
                && child.action == 0x56
                && child.max_life == 100_000
        })
        .count();
    assert_eq!(live_controllers, 1);
    assert!(!session.economy().free_slots.contains(&controller_slot));
    assert!(session.economy().map_chains().is_linked(controller_slot));
    assert_eq!(
        session.economy().free_slots.len(),
        free_after_root_build.saturating_sub(live_build_children + live_controllers)
    );
}

#[test]
fn owner_level_one_has_a_qualified_castle_site_near_the_stage_three_route() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let (castles, buildings) = world.level1_mission.castle_placement_blockers();
    let former_site = Position {
        x: 50_048,
        y: 51_584,
        z: 1_200,
    };
    let qualified_site = Position {
        x: 49_920,
        y: 43_776,
        z: 1_200,
    };
    assert!(!crate::level1_mission::castle_placement_valid(
        &world,
        former_site,
        castles,
        buildings
    ));
    assert!(crate::level1_mission::castle_placement_valid(
        &world,
        qualified_site,
        castles,
        buildings
    ));
}

#[test]
fn qualified_level_one_castle_build_reaches_every_owner_height_transactionally() {
    let (package, closure) = fixture_paths();
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let world = NativeWorld::load_with_owner_cache(
        package,
        closure,
        private.join("generated/level1-unity-asset-cache"),
    )
    .expect("validated package M and owner cache");
    let guard_x = 20_u8;
    let guard_y = 21_u8;
    let mut unconditional = world.terrain_planes().clone();
    let mut signed = unconditional.clone();
    for (x, y, angle) in [
        (guard_x, guard_y, 0x80),
        (guard_x.wrapping_sub(1), guard_y, 0x81),
        (guard_x.wrapping_sub(1), guard_y.wrapping_sub(1), 0x82),
        (guard_x, guard_y.wrapping_sub(1), 0x83),
    ] {
        let index = terrain_index(x, y);
        unconditional.terrain_types[index] = 42;
        unconditional.angles[index] = angle;
        signed.terrain_types[index] = 42;
        signed.angles[index] = angle;
    }
    let mut unconditional_rng = 9_775;
    let mut signed_rng = 9_775;
    add_building_to_terrain(
        &mut unconditional,
        guard_x,
        guard_y,
        MapType::Night,
        &mut unconditional_rng,
    );
    apply_level1_load_path_point(&mut signed, guard_x, guard_y, &mut signed_rng);
    for (x, y) in [
        (guard_x, guard_y),
        (guard_x.wrapping_sub(1), guard_y),
        (guard_x.wrapping_sub(1), guard_y.wrapping_sub(1)),
        (guard_x, guard_y.wrapping_sub(1)),
    ] {
        let index = terrain_index(x, y);
        assert_ne!(
            unconditional.terrain_types[index], 42,
            "AddBuildingToTerrain must mark bit-0x80 cell ({x},{y})"
        );
        assert_eq!(
            signed.terrain_types[index], 42,
            "sub_462A0 must preserve bit-0x80 cell ({x},{y})"
        );
    }
    let footprint = world
        .level1_mission()
        .castle_stage_one_footprint()
        .expect("qualified BUILD[1]");
    let candidate = Position {
        x: 49_920,
        y: 43_776,
        z: 1_200,
    };
    let mut root = Position {
        x: candidate.x & 0xff00,
        y: candidate.y & 0xff00,
        z: world.ground_height(candidate),
    };
    if ((root.x >> 8) + (root.y >> 8)) & 1 != 0 {
        root.x = root.x.wrapping_add(256);
    }
    let start_x = (root.x >> 8) as u8 - footprint.width / 2;
    let start_y = (root.y >> 8) as u8 - footprint.height / 2;
    let mut base = u8::MAX;
    for offset in 0..footprint.width {
        for (x, y) in [
            (start_x.wrapping_add(offset), start_y),
            (
                start_x.wrapping_add(offset),
                start_y.wrapping_add(footprint.height),
            ),
        ] {
            base = base.min(world.terrain_planes().ground_heights[terrain_index(x, y)]);
        }
    }
    for offset in 0..footprint.height {
        for (x, y) in [
            (
                start_x.wrapping_add(footprint.width),
                start_y.wrapping_add(offset),
            ),
            (start_x, start_y.wrapping_add(offset)),
        ] {
            base = base.min(world.terrain_planes().ground_heights[terrain_index(x, y)]);
        }
    }
    let anchor = Position {
        z: i16::from(base) * 32,
        ..root
    };
    let mut terrain = SessionTerrain::from_world(&world);
    let unchanged = terrain.clone();
    let error = terrain
        .apply_castle_update(
            CastleTerrainUpdate::ConstructionPass {
                position: anchor,
                remaining: 0,
            },
            footprint,
        )
        .expect_err("a malformed construction timer must fail closed");
    assert!(error.contains("outside 1..=18"));
    assert_eq!(
        terrain, unchanged,
        "a rejected update must not mutate planes, RNG, or sequence"
    );

    let mut first_session = world.session();
    let mut second_session = world.session();
    first_session
        .terrain
        .apply_castle_update(
            CastleTerrainUpdate::ConstructionPass {
                position: anchor,
                remaining: 18,
            },
            footprint,
        )
        .expect("qualified first construction pass");
    assert_ne!(first_session.terrain, second_session.terrain);
    assert_eq!(second_session.terrain.mutation_sequence(), 0);
    first_session.reset();
    second_session.reset();
    assert_eq!(first_session.terrain, second_session.terrain);
    assert_eq!(first_session.terrain, SessionTerrain::from_world(&world));

    for remaining in (1..=18).rev() {
        terrain
            .apply_castle_update(
                CastleTerrainUpdate::ConstructionPass {
                    position: anchor,
                    remaining,
                },
                footprint,
            )
            .expect("qualified Level 1 footprint does not require an unsupported topology");
    }
    terrain
        .apply_castle_update(
            CastleTerrainUpdate::ConstructionCompleted { position: anchor },
            footprint,
        )
        .expect("qualified completion");
    for row in 0..footprint.height {
        for column in 0..footprint.width {
            let cell = footprint.cells()
                [usize::from(row) * usize::from(footprint.width) + usize::from(column)];
            assert_ne!(cell[1], 0xff);
            assert_eq!(
                terrain.planes().ground_heights
                    [terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row))],
                base.wrapping_add(cell[1])
            );
        }
    }
    assert_eq!(terrain.terrain_rng(), 37_256);
    assert_eq!(terrain.mutation_sequence(), 19);
    assert_eq!(
        sha256_hex(&terrain.planes().terrain_types),
        "6cf4397c008d4ea16e37340bbe819f6d58235e61f8d669260ee9f0ac41e94e16"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().ground_heights),
        "7eff800ac2a60ccf3314bd917408c65a11c1a5cbe59b52d9670b4ed76b3c4533"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().shading),
        "ad91ad8c250c0cd4e3bd9248ae9fa7bf95cb72d056921e7f0dbff03c7fdf7ec7"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().angles),
        "79962c82610ea6abf727a11f82f5351b69355915f11aba9738e4eaca8d18484e"
    );
    terrain
        .apply_castle_update(
            CastleTerrainUpdate::StageDestroyed {
                position: anchor,
                entity_rng: 100,
            },
            footprint,
        )
        .expect("qualified Level 1 teardown");
    assert_eq!(terrain.terrain_rng(), 33_028);
    assert_eq!(terrain.mutation_sequence(), 20);
    assert_eq!(
        sha256_hex(&terrain.planes().terrain_types),
        "5793a3375673e55b74dd3b62159ff6b239241035a3100d599987350b6faa24ae"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().ground_heights),
        "50f3a45214f435ac3b8423254a69b7cc87b80a8039085c25a0fe2033eed1ecba"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().shading),
        "f1a63a2c818a1e16f8e781dd8ef0777833b5194ac8f69a604723408c23e486a8"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().angles),
        "4c72d5ae2beee363ffa2b51ef0b181bcd8c8d5f05ac4a801735ec2722aad3891"
    );
    assert_eq!(
        sha256_hex(&terrain.planes().secondary_heights),
        "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31"
    );
    assert_eq!(
        terrain.planes().entity_indices,
        world.terrain_planes().entity_indices
    );

    let mut ambient = SessionTerrain::from_world(&world);
    let material_x = 200_u8;
    let material_y = 201_u8;
    for (x, y) in [
        (material_x, material_y),
        (material_x.wrapping_add(1), material_y),
        (material_x.wrapping_add(1), material_y.wrapping_add(1)),
        (material_x, material_y.wrapping_add(1)),
    ] {
        ambient.planes.ground_heights[terrain_index(x, y)] = 50;
        ambient.planes.angles[terrain_index(x, y)] = 0;
    }
    ambient
        .apply_castle_update(
            CastleTerrainUpdate::AmbientMaterial {
                position: Position {
                    x: u16::from(material_x) << 8,
                    y: u16::from(material_y) << 8,
                    z: 1_600,
                },
                in_type: 2,
                target_type: 20,
            },
            footprint,
        )
        .expect("source action-0 terrain material update");
    assert_eq!(
        ambient.planes.terrain_types[terrain_index(material_x, material_y)],
        TERRAIN_GENERATION_TABLE_PREFIX[0x18]
    );
    assert_eq!(
        ambient.planes.angles[terrain_index(material_x, material_y)],
        (TERRAIN_GENERATION_TABLE_PREFIX[0x19] | 0x80) & 0xf7
    );

    let collision_x = 210_u8;
    let collision_y = 211_u8;
    for (x, y) in [
        (collision_x, collision_y),
        (collision_x.wrapping_add(1), collision_y),
        (collision_x.wrapping_add(1), collision_y.wrapping_add(1)),
        (collision_x, collision_y.wrapping_add(1)),
    ] {
        ambient.planes.ground_heights[terrain_index(x, y)] = 100;
        ambient.planes.angles[terrain_index(x, y)] = 0;
    }
    ambient
        .apply_castle_update(
            CastleTerrainUpdate::AmbientCollision {
                position: Position {
                    x: u16::from(collision_x) << 8,
                    y: u16::from(collision_y) << 8,
                    z: 3_200,
                },
                height_delta: -3,
            },
            footprint,
        )
        .expect("source action-0 SEARCH terrain deformation");
    for (x, y) in [
        (collision_x, collision_y),
        (collision_x.wrapping_add(1), collision_y),
        (collision_x, collision_y.wrapping_add(1)),
        (collision_x.wrapping_add(1), collision_y.wrapping_add(1)),
    ] {
        assert_eq!(ambient.planes.ground_heights[terrain_index(x, y)], 97);
    }
    assert_eq!(ambient.mutation_sequence(), 2);
}

#[test]
fn qualified_level_one_house_footprints_drive_transactional_action_53_terrain() {
    let (package, closure) = fixture_paths();
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let world = NativeWorld::load_with_owner_cache(
        package,
        closure,
        private.join("generated/level1-unity-asset-cache"),
    )
    .expect("validated package M and owner cache");
    let expected = [
        (17, 48, 48),
        (26, 9, 11),
        (29, 13, 11),
        (30, 13, 15),
        (31, 23, 11),
        (34, 22, 22),
        (42, 11, 11),
        (47, 11, 11),
        (50, 7, 7),
        (51, 4, 12),
        (52, 12, 4),
        (53, 8, 8),
    ];
    for (index, width, height) in expected {
        let footprint = world
            .house_build_footprint(index)
            .expect("qualified Level 1 house BUILD row");
        assert_eq!(
            (footprint.index, footprint.width, footprint.height),
            (index, width, height)
        );
        assert_eq!(
            footprint.cells().len(),
            usize::from(width) * usize::from(height)
        );
    }

    let house = &world.entities()[11];
    assert_eq!(house.level_gate, 29);
    let footprint = world
        .house_build_footprint(house.level_gate.cast_unsigned())
        .expect("house 11 BUILD[29]");
    let mut terrain = SessionTerrain::from_world(&world);
    let before = terrain.deterministic_hash();
    let outcome = terrain
        .apply_house_removal(house.position, house.entity_rng, 2, true, footprint)
        .expect("action-53 terrain mutation");
    assert_eq!(outcome.final_occupants, 0);
    assert_eq!(outcome.final_rng, house.entity_rng);
    assert_eq!(
        outcome
            .residents
            .iter()
            .map(|resident| (resident.model, resident.action))
            .collect::<Vec<_>>(),
        [(4, 33), (12, 97)]
    );
    assert_eq!(terrain.mutation_sequence(), 1);
    assert_ne!(terrain.deterministic_hash(), before);

    let mut session = world.session();
    let empty_mission_before = session.mission.clone();
    let empty_live_world_before = session.live_world.clone();
    let empty_global_rng = session.mission.global_rng();
    let empty_events = session.mission.events().to_vec();
    let empty_arena_hash = session.mission.source_arena_hash_v1();
    let empty_live_hash = session.live_world_hash_v2();
    let actor_storage = session.mission.actors().as_ptr();
    let goat_storage = session.live_world.goats().as_ptr();
    assert!(
        apply_level1_source_effects(&mut session.mission, &mut session.live_world, &[],)
            .expect("empty effect batch is an exact no-op")
            .is_empty()
    );
    assert_eq!(session.mission, empty_mission_before);
    assert_eq!(session.live_world, empty_live_world_before);
    assert_eq!(session.mission.global_rng(), empty_global_rng);
    assert_eq!(session.mission.events(), empty_events);
    assert_eq!(session.mission.source_arena_hash_v1(), empty_arena_hash);
    assert_eq!(session.live_world_hash_v2(), empty_live_hash);
    assert_eq!(
        session.mission.actors().as_ptr(),
        actor_storage,
        "empty production dispatch must not clone mission storage"
    );
    assert_eq!(
        session.live_world.goats().as_ptr(),
        goat_storage,
        "empty production dispatch must not clone live-world storage"
    );

    let resident_position = session.live_world.houses()[0].position;
    let resident_slot = session
        .mission
        .spawn_house_resident(resident_position, 4, 33, 0)
        .expect("resident constructor")
        .expect("qualified source arena has a resident slot");
    let occupants_before = session.live_world.houses()[0].occupants;
    let live_hash_before = session.live_world_hash_v2();
    let arena_hash_before = session.mission.source_arena_hash_v1();
    let updates = apply_level1_source_effects(
        &mut session.mission,
        &mut session.live_world,
        &[Level1SourceEffect::TryEnterResident {
            resident_source_entity_index: resident_slot,
            house_source_entity_index: 1,
        }],
    )
    .expect("atomic resident entry effect");
    assert_eq!(updates.len(), 1);
    assert_eq!(
        session.live_world.houses()[0].occupants,
        occupants_before + 1,
        "successful entry increments the sole house authority exactly once"
    );
    assert!(!session.mission.dynamic_residents().next().unwrap().active);
    assert_ne!(session.live_world_hash_v2(), live_hash_before);
    assert_ne!(session.mission.source_arena_hash_v1(), arena_hash_before);

    while session.live_world.houses()[0].occupants
        < i32::from(session.live_world.houses()[0].capacity)
    {
        assert_eq!(
            session.live_world.try_enter_resident(1),
            HouseResidentEntry::Entered
        );
    }
    let full_resident = session
        .mission
        .spawn_house_resident(resident_position, 4, 33, 0)
        .expect("resident constructor")
        .expect("second resident slot");
    let full_live_hash = session.live_world_hash_v2();
    let full_arena_hash = session.mission.source_arena_hash_v1();
    assert!(
        apply_level1_source_effects(
            &mut session.mission,
            &mut session.live_world,
            &[Level1SourceEffect::TryEnterResident {
                resident_source_entity_index: full_resident,
                house_source_entity_index: 1,
            }],
        )
        .expect("full resident entry rejection")
        .is_empty()
    );
    assert_eq!(session.live_world_hash_v2(), full_live_hash);
    assert_eq!(session.mission.source_arena_hash_v1(), full_arena_hash);
    assert!(
        session
            .mission
            .dynamic_residents()
            .find(|resident| resident.source_entity_index == full_resident)
            .is_some_and(|resident| resident.active)
    );

    let mut dead_session = world.session();
    let lethal = dead_session.live_world.houses()[0].life + 1;
    dead_session
        .live_world
        .inject_damage(1, lethal, 152)
        .expect("lethal house damage");
    let _ = dead_session
        .live_world
        .step_house_source_slot(1, &dead_session.terrain);
    let _ = dead_session
        .live_world
        .step_house_source_slot(1, &dead_session.terrain);
    assert!(!dead_session.live_world.houses()[0].active);
    let dead_resident = dead_session
        .mission
        .spawn_house_resident(resident_position, 4, 33, 0)
        .expect("resident constructor")
        .expect("dead-house resident slot");
    let dead_live_hash = dead_session.live_world_hash_v2();
    let dead_arena_hash = dead_session.mission.source_arena_hash_v1();
    assert!(
        apply_level1_source_effects(
            &mut dead_session.mission,
            &mut dead_session.live_world,
            &[Level1SourceEffect::TryEnterResident {
                resident_source_entity_index: dead_resident,
                house_source_entity_index: 1,
            }],
        )
        .expect("dead resident entry rejection")
        .is_empty()
    );
    assert_eq!(dead_session.live_world_hash_v2(), dead_live_hash);
    assert_eq!(dead_session.mission.source_arena_hash_v1(), dead_arena_hash);
}

#[test]
fn direct_session_action_51_applies_source_ordered_synthetic_build_planes() {
    let (package, closure) = fixture_paths();
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut world = NativeWorld::load_with_owner_cache(
        package,
        closure,
        private.join("generated/level1-unity-asset-cache"),
    )
    .expect("validated package M and owner cache");
    let footprint =
        CastleBuildFootprint::from_owner_cells(29, 2, 2, &[8, 4, 0xff, 0xff, 9, 2, 1, 6])
            .expect("synthetic action-51 BUILD");
    world.house_build_footprints.insert(29, footprint);
    let mut session = world.session();
    let position = Position {
        x: 100 << 8,
        y: 100 << 8,
        z: 20 * 32,
    };
    let start_x = 99_u8;
    let start_y = 99_u8;
    for row in 0..4_u8 {
        for column in 0..4_u8 {
            let index = terrain_index(
                start_x.wrapping_sub(1).wrapping_add(column),
                start_y.wrapping_sub(1).wrapping_add(row),
            );
            session.terrain.planes.ground_heights[index] = 10;
            session.terrain.planes.terrain_types[index] = 42;
            session.terrain.planes.angles[index] = 0;
            session.terrain.planes.shading[index] = 0;
        }
    }
    let mut house = session
        .live_world
        .houses()
        .iter()
        .find(|house| house.entity_index == 11)
        .copied()
        .expect("house 11");
    house.position = position;
    house.building_index = 29;
    house.action = crate::house::HOUSE_TERRAIN_TO_LIVE_ACTION;
    house.max_life = 6;
    house.life = 6;
    house.owner_entity_index = session.state.entity_index;
    house.control_flags |= 1;
    session.live_world.initialize_builder_house(house).unwrap();
    let frozen_source_views =
        level1_turn_start_source_views(&session.mission, &session.state, &session.live_world);

    let sample = |terrain: &SessionTerrain| {
        [(0_u8, 0_u8), (0, 1), (1, 1)].map(|(column, row)| {
            let index = terrain_index(start_x.wrapping_add(column), start_y.wrapping_add(row));
            (
                terrain.planes.ground_heights[index],
                terrain.planes.terrain_types[index],
                terrain.planes.angles[index],
                terrain.planes.shading[index],
            )
        })
    };
    assert_eq!(sample(&session.terrain), [(10, 42, 0, 0); 3]);

    let first = session
        .live_world
        .step_house_source_slot(house.entity_index, &session.terrain);
    assert!(matches!(
        first.as_slice(),
        [HouseStepEvent::ConstructionPass { remaining: 5, .. }]
    ));
    assert!(session.process_house_events(&first, &frozen_source_views));
    let mid = sample(&session.terrain);
    assert_eq!(mid, [(12, 8, 145, 35), (12, 9, 129, 32), (13, 37, 1, 30)]);

    for expected_remaining in (1..=4).rev() {
        let events = session
            .live_world
            .step_house_source_slot(house.entity_index, &session.terrain);
        assert!(matches!(
            events.as_slice(),
            [HouseStepEvent::ConstructionPass { remaining, .. }]
                if *remaining == expected_remaining
        ));
        assert!(session.process_house_events(&events, &frozen_source_views));
    }
    let before_completion = sample(&session.terrain);
    assert_eq!(before_completion.map(|cell| cell.0), [24, 22, 26]);

    let completed = session
        .live_world
        .step_house_source_slot(house.entity_index, &session.terrain);
    assert!(matches!(
        completed.as_slice(),
        [HouseStepEvent::ConstructionCompleted { .. }]
    ));
    assert!(session.process_house_events(&completed, &frozen_source_views));
    let final_planes = sample(&session.terrain);
    assert_eq!(
        final_planes,
        [(24, 8, 145, 36), (22, 9, 129, 32), (26, 37, 1, 18)]
    );
    let live = session
        .live_world
        .houses()
        .iter()
        .find(|house| house.entity_index == 11)
        .copied()
        .expect("completed house 11");
    assert_eq!(live.action, crate::house::HOUSE_LIVE_ACTION);
    assert_eq!(live.owner_entity_index, session.state.entity_index);
    assert_eq!(live.control_flags & 1, 0);
    assert_eq!(session.terrain.mutation_sequence(), 6);
    assert!(matches!(
        session.pending_house_effects.as_slice(),
        [
            HouseStepEvent::ConstructionPass { remaining: 5, .. },
            HouseStepEvent::ConstructionPass { remaining: 4, .. },
            HouseStepEvent::ConstructionPass { remaining: 3, .. },
            HouseStepEvent::ConstructionPass { remaining: 2, .. },
            HouseStepEvent::ConstructionPass { remaining: 1, .. },
            HouseStepEvent::ConstructionCompleted { .. },
        ]
    ));
}

#[test]
fn house_action_53_interleaves_resident_and_height_rng_in_one_cell() {
    let mut boundary_20_rng = 3;
    assert_eq!(reduce_house_height(100, 10, &mut boundary_20_rng), 90);
    assert_eq!(
        boundary_20_rng, 37_570,
        "`rand % 0x32 == 20` consumes one draw"
    );
    let mut boundary_21_rng = 20;
    assert_eq!(reduce_house_height(100, 10, &mut boundary_21_rng), 108);
    assert_eq!(
        boundary_21_rng, 14_898,
        "`rand % 0x32 == 21` consumes the second draw"
    );
    let mut wrapping_rng = 29;
    assert_eq!(
        reduce_house_height(250, 1, &mut wrapping_rng),
        12,
        "the second branch truncates to the source byte"
    );
    assert_eq!(wrapping_rng, 20_859);
    let mut no_draw_rng = 0xabcd;
    assert_eq!(reduce_house_height(10, 10, &mut no_draw_rng), 0);
    assert_eq!(no_draw_rng, 0xabcd);

    let footprint =
        CastleBuildFootprint::from_owner_cells(99, 1, 1, &[0xff, 10]).expect("one-cell BUILD");
    let mut terrain = SessionTerrain {
        map_type: MapType::Night,
        planes: TerrainPlanes {
            terrain_types: vec![0; 65_536].into_boxed_slice(),
            ground_heights: vec![100; 65_536].into_boxed_slice(),
            shading: vec![0; 65_536].into_boxed_slice(),
            angles: vec![0; 65_536].into_boxed_slice(),
            secondary_heights: vec![0; 65_536].into_boxed_slice(),
            entity_indices: vec![0; 65_536].into_boxed_slice(),
        },
        terrain_rng: 39_663,
        mutation_sequence: 0,
    };
    let outcome = terrain
        .apply_house_removal(Position { x: 0, y: 0, z: 0 }, 2, 5, true, &footprint)
        .expect("synthetic action-53 pass");
    assert_eq!(outcome.final_occupants, 4);
    assert_eq!(outcome.final_rng, 62_591);
    assert_eq!(
        outcome.residents,
        [HouseRemovalResident {
            position: Position {
                x: 0,
                y: 0,
                z: 3_200
            },
            model: 13,
            action: 105,
        }]
    );
    assert_eq!(terrain.ground_heights()[0], 100);
    assert_eq!(terrain.mutation_sequence(), 1);
}

#[test]
fn direct_session_commits_house_action_53_trace_before_reclaim() {
    let (package, closure) = fixture_paths();
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let world = NativeWorld::load_with_owner_cache(
        package,
        closure,
        private.join("generated/level1-unity-asset-cache"),
    )
    .expect("validated package M and owner cache");
    let mut session = world.session();
    let initial_terrain_hash = session.terrain.deterministic_hash();
    session
        .live_world
        .inject_damage(11, 28_001, session.state.entity_index.cast_signed())
        .expect("fatal house damage");

    session.step(PlayerCommand::default());
    let dying = session
        .live_world
        .houses()
        .iter()
        .find(|house| house.entity_index == 11)
        .expect("house 11");
    assert!(dying.active);
    assert_eq!(dying.action, crate::house::HOUSE_REMOVE_ACTION);
    assert_eq!(
        session.terrain.mutation_sequence(),
        2,
        "the first source turn applies two action-0 village terrain effects"
    );

    session.step(PlayerCommand::default());
    let removed = session
        .live_world
        .houses()
        .iter()
        .find(|house| house.entity_index == 11)
        .expect("house 11 retained for deterministic history");
    assert!(!removed.active);
    assert_eq!(removed.occupants, 0);
    assert_eq!(removed.rng, world.entities()[11].entity_rng);
    assert_eq!(
        session.terrain.mutation_sequence(),
        3,
        "house removal follows the two source-ordered ambient mutations"
    );
    assert_ne!(session.terrain.deterministic_hash(), initial_terrain_hash);
    assert_eq!(
        session
            .pending_house_effects
            .iter()
            .filter_map(|event| match event {
                HouseStepEvent::ResidentRequested { model, action, .. } => Some((*model, *action)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [(4, 33), (12, 97)]
    );

    let mut replay = world.session();
    replay
        .live_world
        .inject_damage(11, 28_001, replay.state.entity_index.cast_signed())
        .expect("fatal replay damage");
    replay.step(PlayerCommand::default());
    replay.step(PlayerCommand::default());
    assert_eq!(replay.live_world, session.live_world);
    assert_eq!(replay.terrain, session.terrain);
    assert_eq!(replay.mission, session.mission);
    assert_eq!(replay.pending_house_effects, session.pending_house_effects);

    let mut exhausted = world.session();
    exhausted
        .live_world
        .inject_damage(11, 28_001, exhausted.state.entity_index.cast_signed())
        .expect("fatal house damage");
    let destruction_started = exhausted
        .live_world
        .step_house_source_slot(11, &exhausted.terrain);
    assert!(matches!(
        destruction_started.as_slice(),
        [HouseStepEvent::DestructionStarted { .. }]
    ));
    while exhausted
        .mission
        .spawn_house_resident(Position::default(), 4, 33, 152)
        .unwrap()
        .is_some()
    {}
    let live_before = exhausted.live_world.clone();
    let terrain_before = exhausted.terrain.clone();
    let mission_before = exhausted.mission.clone();
    let effects_before = exhausted.pending_house_effects.clone();
    let removal = exhausted
        .live_world
        .step_house_source_slot(11, &exhausted.terrain);
    let frozen_source_views =
        level1_turn_start_source_views(&exhausted.mission, &exhausted.state, &exhausted.live_world);
    assert!(!exhausted.process_house_events(&removal, &frozen_source_views));
    exhausted.live_world = live_before.clone();
    assert_eq!(exhausted.live_world, live_before);
    assert_eq!(exhausted.terrain, terrain_before);
    assert_eq!(exhausted.mission, mission_before);
    assert_eq!(exhausted.pending_house_effects, effects_before);
}

#[test]
fn native_session_dispatches_an_ordinary_castle_defender_from_action_zero_to_one() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let mut session = world.session();
    let controller_slot = session
        .mission
        .spawn_castle_defense_controller(
            Position {
                x: 100 << 8,
                y: 100 << 8,
                z: 384,
            },
            session.state.entity_index,
        )
        .expect("ordinary Level 1 source pool has a controller slot");
    assert_eq!(
        session.mission.castle_defense_controllers()[0].action_state,
        0
    );

    session.step(PlayerCommand::default());

    let controller = session
        .mission
        .castle_defense_controllers()
        .iter()
        .find(|controller| controller.source_entity_index == controller_slot)
        .unwrap();
    assert_eq!(controller.action_state, 1);
}

#[test]
fn level_two_direct_starting_spells_feed_requested_castle_route() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    assert!(
        inspection.level2_castle_stage_footprints.contains_key(&1),
        "Castle public route requires the validated stage-1 BUILD footprint"
    );
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 requested-Castle session");
    session.select_level2_campaign_entry_stage();
    let baseline_hash = session.campaign_state_hash();
    let baseline_shared_hash = session.shared_spells().reset_hash();
    let baseline_economy_hash = session.economy().reset_hash();
    let initial_shared = session.shared_spells().clone();

    assert_eq!(SOURCE_SPELL_INDEX_MAP[2], CASTLE_SPELL_FAMILY);
    let local_player = &session.shared_spells().local_player;
    assert_eq!(
        &local_player.enabled_spell_holders[..4],
        &[115, 116, 117, 118]
    );
    assert_eq!(session.economy().free_slot_count(), 881);
    let fire = session
        .context
        .spell_fire
        .as_ref()
        .expect("Level 2 spell-fire context");
    assert!(
        fire.holders
            .iter()
            .any(|holder| holder.model == CASTLE_SPELL_FAMILY)
    );
    let requested_cast = PlayerCommand {
        opcode: 6,
        argument_1: 2,
        flags: 0x40,
        argument_6: 17,
        argument_8: (-23_i16).cast_unsigned(),
        ..PlayerCommand::default()
    };
    session.state.actual_speed = 37;
    session.state.target_speed = 37;
    session.state.entity_fov = 29;
    let transferred_a4_byte = 7;
    {
        let mut shared = session.shared_spells_mut();
        let player_index = shared.player_entity.index;
        let player = shared
            .players
            .iter_mut()
            .find(|player| player.entity_index == player_index)
            .expect("qualified local Level 2 player row");
        player
            .type_str_164_auxiliary
            .as_mut()
            .expect("Level 2 player A4 carrier")
            .byte_0x154_340 = transferred_a4_byte;
        player.record[PLAYER_LEGACY_RECORD_BYTES + 16] = transferred_a4_byte;
        shared.local_player = player.clone();
    }
    session.step_outer_turn(requested_cast, 0);
    assert_eq!(
        session.economy().player_castle_entity_index(),
        0,
        "sub_5F660 arms the holder without a Castle relation"
    );
    assert!(
        session.economy().castle().is_none(),
        "the class-9 flyer is not the class-3 Castle root"
    );
    let flyer = session
        .economy()
        .castle_projectiles()
        .iter()
        .find(|projectile| projectile.active)
        .copied()
        .expect("successful holder callback creates a live Castle flyer");
    assert_eq!((flyer.source_class, flyer.source_model), (9, 10));
    let mut expected_launch_position = session.state.position;
    crate::move_entity(
        &mut expected_launch_position,
        session.state.yaw.wrapping_sub(512),
        0,
        256,
    );
    if session
        .canonical_terrain()
        .ground_height(expected_launch_position)
        > expected_launch_position.z
    {
        expected_launch_position = session.state.position;
    }
    expected_launch_position.z = expected_launch_position
        .z
        .wrapping_add(session.state.entity_fov);
    assert_eq!(
        flyer.position, expected_launch_position,
        "selected 0x40 uses the source left-hand displacement, strict terrain rollback, and parent FOV"
    );
    assert_eq!(flyer.yaw, 17_i16.wrapping_add(session.state.yaw));
    assert_eq!(
        flyer.pitch,
        (-23_i16).wrapping_add(session.state.display_pitch),
        "sub_69AB0 wraps the parent A4 pitch word with the live display pitch"
    );
    let mut expected_axis_position = session.state.position;
    crate::move_entity(
        &mut expected_axis_position,
        17_i16.wrapping_add(session.state.yaw),
        0,
        4_096,
    );
    expected_axis_position.z = session
        .canonical_terrain()
        .ground_height(expected_axis_position);
    assert_eq!(
        flyer.axis_position, expected_axis_position,
        "sub_69AB0 derives the Castle target from the undisplaced parent position"
    );
    assert_eq!(flyer.source_subspell_index, 0);
    assert_eq!(flyer.source_mana, 9);
    assert_eq!(flyer.source_a4_byte_154, transferred_a4_byte);
    let local_player_after_launch = session
        .shared_spells()
        .players
        .iter()
        .find(|player| player.entity_index == session.state.entity_index)
        .expect("qualified local player remains materialized");
    assert_eq!(
        local_player_after_launch
            .type_str_164_auxiliary()
            .expect("Level 2 A4 carrier")
            .byte_0x154_340,
        0,
        "successful allocation transfers and clears the parent A4 byte"
    );
    assert_eq!(
        session
            .shared_spells()
            .activation_events()
            .iter()
            .filter(|event| matches!(
                event,
                FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::InitialCastleLaunch,
                    sound_id: 15,
                    ..
                }
            ))
            .count(),
        1,
        "successful sub_69AB0 allocation emits exactly one launch sound 15"
    );
    let player_actual_speed_after_callback = session.state.actual_speed;
    assert_ne!(player_actual_speed_after_callback, 0);
    assert_eq!(
        flyer.actual_speed,
        384_i16.wrapping_add(player_actual_speed_after_callback)
    );
    assert_eq!(flyer.minimum_speed, 384);
    assert_eq!((flyer.source_byte_43, flyer.source_byte_44), (3, 2));
    assert_eq!(flyer.target_castle_slot, 0);
    assert!(
        session
            .economy()
            .events()
            .iter()
            .all(|event| { !matches!(event, Level2EconomyEvent::CastleBuilt { .. }) })
    );

    let request_family38_receipts = session
        .dispatch_events()
        .iter()
        .filter_map(|event| {
            if let Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::CastleProjectile,
                ..
            } = event
            {
                Some((
                    *source_slot,
                    session.economy().slot_generation(*source_slot),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        request_family38_receipts.len(),
        1,
        "the live higher-slot Castle callback emits exactly one family38 receipt"
    );
    assert_eq!(
        request_family38_receipts[0].0, flyer.source_entity_index,
        "the receipt is bound to the allocated flyer source row"
    );
    assert!(!session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::CastleRoot,
            ..
        }
    )));

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .castle_projectiles()
            .iter()
            .all(|projectile| !projectile.active),
        "the following callback consumes the gate-passed flyer"
    );
    let consumed_flyer = session
        .economy()
        .castle_projectiles()
        .iter()
        .find(|projectile| projectile.source_entity_index == flyer.source_entity_index)
        .expect("the consumed flyer remains inspectable after DisableEntityDrawing");
    assert_ne!(
        consumed_flyer.position, flyer.position,
        "the first Castle callback moves the flyer at its source-derived speed"
    );
    let following_family1_receipt = session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                kind: Level2DynamicArenaKind::CastleRoot,
                ..
            }
        )
    });
    let following_family38_receipt = session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                kind: Level2DynamicArenaKind::CastleProjectile,
                ..
            }
        )
    });
    assert!(
        following_family1_receipt,
        "the following source callback dispatches the Castle root"
    );
    assert!(!following_family38_receipt);
    let root = session
        .economy()
        .castle()
        .expect("following flyer callback allocates a class-3/model-2 Castle seed");
    assert_eq!((root.source_class, root.source_model), (3, 2));
    assert_eq!(root.action, 5);
    assert_eq!(root.source_word_2e, 4);
    assert_eq!(root.level, 1);
    assert_eq!(
        session.economy().player_castle_entity_index(),
        root.source_entity_index,
        "the higher-slot root callback runs in the allocation pass and publishes its child relation"
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(
        event,
        Mc2DispatchEvent::DynamicArena {
            kind: Level2DynamicArenaKind::CastleChild,
            ..
        }
    )));
    assert!(
        session
            .economy()
            .events()
            .iter()
            .all(|event| { !matches!(event, Level2EconomyEvent::CastleBuilt { .. }) })
    );

    session.step_outer_turn(PlayerCommand::default(), 0);
    let completed_root = session
        .economy()
        .castle()
        .expect("Castle seed remains live after construction child");
    assert_eq!(
        session.economy().player_castle_entity_index(),
        completed_root.source_entity_index,
        "sub_60480 publishes the relation only after child allocation"
    );
    assert_eq!(completed_root.action, 5);
    assert_eq!(completed_root.source_word_2e, 4);
    assert_eq!(
        (completed_root.shift_yaw, completed_root.shift_fov),
        (-8_192, 0x4000)
    );
    assert!(
        !session
            .economy()
            .events()
            .iter()
            .any(|event| matches!(event, Level2EconomyEvent::CastleBuilt { .. }))
    );

    session.reset();
    assert_eq!(
        session.shared_spells().dynamic_pickups,
        initial_shared.dynamic_pickups
    );
    assert_eq!(session.shared_spells().players, initial_shared.players);
    assert_eq!(
        session.shared_spells().player_entities,
        initial_shared.player_entities
    );
    assert_eq!(
        session.shared_spells().local_player,
        initial_shared.local_player
    );
    assert_eq!(
        session.shared_spells().player_mana,
        initial_shared.player_mana
    );
    assert_eq!(
        session.shared_spells().spell_levels,
        initial_shared.spell_levels
    );
    assert_eq!(session.shared_spells().reset_hash(), baseline_shared_hash);
    assert_eq!(session.economy().reset_hash(), baseline_economy_hash);
    assert_eq!(session.campaign_state_hash(), baseline_hash);
}

#[test]
fn level_two_castle_public_hands_apply_source_launch_displacement() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");

    for (label, assignment_opcode, flags, yaw_delta) in [
        ("left", Some(31_i8), 0x10_i8, -512_i16),
        ("right", Some(32_i8), 0x20_i8, 512_i16),
        ("selected", None, 0x40_i8, -512_i16),
    ] {
        let mut session = inspection
            .level2_reset_session()
            .expect("Level 2 Castle hand-displacement session");
        session.select_level2_campaign_entry_stage();
        if let Some(opcode) = assignment_opcode {
            session.step_outer_turn(
                PlayerCommand {
                    opcode,
                    argument_1: CASTLE_SPELL_FAMILY.cast_signed(),
                    argument_2: 0,
                    ..PlayerCommand::default()
                },
                0,
            );
        }

        session.state.position = Position {
            x: 384,
            y: 62_080,
            z: 256,
        };
        session.state.yaw = 0;
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        session.state.entity_fov = 0;
        session.step_outer_turn(
            PlayerCommand {
                opcode: if assignment_opcode.is_none() { 6 } else { 0 },
                argument_1: if assignment_opcode.is_none() {
                    CASTLE_SPELL_FAMILY.cast_signed()
                } else {
                    0
                },
                flags,
                ..PlayerCommand::default()
            },
            0,
        );

        let flyer = session
            .economy()
            .castle_projectiles()
            .iter()
            .find(|projectile| projectile.active)
            .copied()
            .unwrap_or_else(|| panic!("{label} public hand allocates a live Castle flyer"));
        let mut expected = session.state.position;
        crate::move_entity(
            &mut expected,
            session.state.yaw.wrapping_add(yaw_delta),
            0,
            256,
        );
        if session.canonical_terrain().ground_height(expected) > expected.z {
            expected = session.state.position;
        }
        assert_eq!(
            flyer.position, expected,
            "{label} hand follows sub_68E50 displacement and strict terrain rollback"
        );
        assert_eq!(
            session
                .shared_spells()
                .activation_events()
                .iter()
                .filter(|event| matches!(
                    event,
                    FlightEvent::PrepareSound {
                        audio_site: crate::MissionAudioSite::InitialCastleLaunch,
                        sound_id: 15,
                        ..
                    }
                ))
                .count(),
            1,
            "{label} successful allocation emits one launch sound"
        );
    }
}

#[test]
fn level_two_castle_public_route_rejects_invalid_input_and_allocation_failures() {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component");
    let request = PlayerCommand {
        opcode: 6,
        argument_1: 2,
        flags: 0x40,
        ..PlayerCommand::default()
    };

    let mut no_cast = inspection
        .level2_reset_session()
        .expect("Level 2 flags-negative session");
    no_cast.select_level2_campaign_entry_stage();
    no_cast.step_outer_turn(
        PlayerCommand {
            flags: 0,
            ..request
        },
        0,
    );
    assert_eq!(no_cast.economy().player_castle_entity_index(), 0);
    assert!(no_cast.economy().castle().is_none());
    assert!(
        no_cast
            .economy()
            .castle_projectiles()
            .iter()
            .all(|projectile| !projectile.active)
    );

    let mut missing_holder = inspection
        .level2_reset_session()
        .expect("Level 2 missing-holder session");
    missing_holder.select_level2_campaign_entry_stage();
    let holder_index = missing_holder
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .expect("family-2 holder")
        .index;
    {
        let mut shared = missing_holder.shared_spells_mut();
        shared
            .dynamic_pickups
            .retain(|holder| holder.index != holder_index);
        shared.local_player.enabled_spell_holders[usize::from(CASTLE_SPELL_FAMILY)] = 0;
        shared.players[0].enabled_spell_holders[usize::from(CASTLE_SPELL_FAMILY)] = 0;
    }
    let fire = missing_holder
        .context
        .spell_fire
        .as_mut()
        .expect("Level 2 spell-fire context");
    fire.enabled_holder_indices[usize::from(CASTLE_SPELL_FAMILY)] = 0;
    fire.selected_holder_index = 0;
    fire.holders
        .retain(|holder| holder.entity_index != holder_index);
    missing_holder.step_outer_turn(request, 0);
    assert_eq!(missing_holder.economy().player_castle_entity_index(), 0);
    assert!(missing_holder.economy().castle().is_none());
    assert!(
        missing_holder
            .economy()
            .castle_projectiles()
            .iter()
            .all(|projectile| !projectile.active)
    );

    let mut allocation_failed = inspection
        .level2_reset_session()
        .expect("Level 2 projectile-allocation failure session");
    allocation_failed.select_level2_campaign_entry_stage();
    let baseline_hash = allocation_failed.campaign_state_hash();
    let holder_before = allocation_failed
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .cloned()
        .expect("family-2 holder");
    let word24_before = 0x1357_u16;
    let word36_before = 3_u16;
    {
        let mut shared_view = allocation_failed.shared_spells_mut();
        let holder = shared_view
            .dynamic_pickups
            .iter_mut()
            .find(|holder| holder.index == holder_before.index)
            .expect("family-2 holder for source word offsets");
        holder.record[40..42].copy_from_slice(&word24_before.to_le_bytes());
        holder.record[58..60].copy_from_slice(&word36_before.to_le_bytes());
    }
    let retained_a4_byte = 11;
    {
        let mut shared = allocation_failed.shared_spells_mut();
        let player_entity_index = shared.player_entity.index;
        let player = shared
            .players
            .iter_mut()
            .find(|player| player.entity_index == player_entity_index)
            .expect("qualified local Level 2 player row");
        player
            .type_str_164_auxiliary
            .as_mut()
            .expect("Level 2 player A4 carrier")
            .byte_0x154_340 = retained_a4_byte;
        player.record[PLAYER_LEGACY_RECORD_BYTES + 16] = retained_a4_byte;
        shared.local_player = player.clone();
    }
    allocation_failed.economy_mut().free_slots.clear();
    allocation_failed.step_outer_turn(request, 0);
    assert_eq!(allocation_failed.economy().player_castle_entity_index(), 0);
    assert!(allocation_failed.economy().castle().is_none());
    assert!(
        allocation_failed
            .economy()
            .castle_projectiles()
            .iter()
            .all(|projectile| !projectile.active)
    );
    let holder_after = allocation_failed
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .expect("family-2 holder remains after allocation failure");
    assert_eq!(holder_after.index, holder_before.index);
    // `sub_5F660`/`sub_68DE0` still arm the holder and charge through the
    // source mana-regeneration side effect; only the absent flyer is
    // rolled back by the failed NewEvent allocation.
    assert_ne!(holder_after.active_value, 0);
    assert_eq!(
        u16::from_le_bytes([holder_after.record[40], holder_after.record[41]]),
        word24_before,
        "Castle action-6 must leave package word24 untouched"
    );
    assert_eq!(
        u16::from_le_bytes([holder_after.record[58], holder_after.record[59]]),
        word36_before - 1,
        "Castle action-6 decrements package word36 at bytes 58..60"
    );
    assert_eq!(
        allocation_failed
            .shared_spells()
            .local_player
            .type_str_164_auxiliary()
            .expect("Level 2 A4 carrier")
            .byte_0x154_340,
        retained_a4_byte,
        "failed NewEvent allocation leaves the source parent A4 byte untouched"
    );
    assert!(
        allocation_failed
            .shared_spells()
            .activation_events()
            .iter()
            .all(|event| !matches!(
                event,
                FlightEvent::PrepareSound {
                    audio_site: crate::MissionAudioSite::InitialCastleLaunch,
                    sound_id: 15,
                    ..
                }
            )),
        "failed NewEvent allocation emits no Castle launch sound"
    );
    allocation_failed.reset();
    assert_eq!(allocation_failed.campaign_state_hash(), baseline_hash);

    let mut site_rejected = inspection
        .level2_reset_session()
        .expect("Level 2 terrain-site rejection session");
    site_rejected.select_level2_campaign_entry_stage();
    let blocked_tile = (0_u16..=u16::from(u8::MAX))
        .flat_map(|x| (0_u16..=u16::from(u8::MAX)).map(move |y| (x as u8, y as u8)))
        .find(|&(x, y)| {
            (0_u8..8).any(|row| {
                (0_u8..8).any(|column| {
                    site_rejected
                        .canonical_terrain()
                        .angle_flags(crate::Tile {
                            x: x.wrapping_sub(8).wrapping_add(column),
                            y: y.wrapping_sub(8).wrapping_add(row),
                        })
                        .cast_signed()
                        < 0
                })
            })
        })
        .expect("qualified Day terrain has a signed-negative site window");
    let blocked_position = Position {
        x: u16::from(blocked_tile.0) << 8,
        y: u16::from(blocked_tile.1) << 8,
        z: site_rejected.canonical_terrain().ground_height(Position {
            x: u16::from(blocked_tile.0) << 8,
            y: u16::from(blocked_tile.1) << 8,
            z: 0,
        }),
    };
    site_rejected.state.position = blocked_position;
    let site_holder_index = site_rejected
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .expect("family-2 holder")
        .index;
    site_rejected.step_outer_turn(request, 0);
    let family38_receipts = site_rejected
        .dispatch_events()
        .iter()
        .filter_map(|event| {
            if let Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::CastleProjectile,
                ..
            } = event
            {
                Some(*source_slot)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(
        family38_receipts.is_empty(),
        "a blocked first flyer callback returns false without a DynamicArena receipt"
    );
    assert!(
        !site_rejected.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                kind: Level2DynamicArenaKind::CastleRoot,
                ..
            }
        )),
        "a blocked first flyer cannot produce a same-call Castle root"
    );
    let hidden_projectile = site_rejected
        .economy()
        .castle_projectiles()
        .iter()
        .find(|projectile| projectile.site_rejected && !projectile.active)
        .copied()
        .expect("blocked flyer remains hidden until the next source prepass");
    let hidden_generation = site_rejected
        .economy()
        .slot_generation(hidden_projectile.source_entity_index);
    assert!(hidden_projectile.drawing_disabled);
    assert!(
        site_rejected
            .economy()
            .map_chains()
            .is_linked(hidden_projectile.source_entity_index),
        "DisableEntityDrawing leaves the map relation reserved"
    );
    assert!(
        !site_rejected
            .economy()
            .free_slots
            .contains(&hidden_projectile.source_entity_index),
        "the hidden flyer slot is not reusable in the same pass"
    );
    assert_eq!(
        site_rejected
            .economy()
            .slot_generation(hidden_projectile.source_entity_index),
        hidden_generation,
        "the hidden row keeps its source generation before following-pass reclaim"
    );
    site_rejected.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(site_rejected.economy().player_castle_entity_index(), 0);
    assert!(site_rejected.economy().castle().is_none());
    assert!(
        site_rejected
            .economy()
            .castle_projectiles()
            .iter()
            .all(|projectile| projectile.source_entity_index
                != hidden_projectile.source_entity_index)
    );
    assert!(
        site_rejected
            .economy()
            .free_slots
            .contains(&hidden_projectile.source_entity_index),
        "the following source prepass reclaims the hidden flyer"
    );
    assert!(
        !site_rejected
            .economy()
            .map_chains()
            .is_linked(hidden_projectile.source_entity_index)
    );
    let holder_projections = site_rejected
        .shared_spells()
        .holders
        .iter()
        .chain(site_rejected.shared_spells().dynamic_pickups.iter())
        .filter(|holder| holder.index == site_holder_index)
        .map(|holder| (holder.active_value, holder.record[50], holder.record[51]))
        .collect::<Vec<_>>();
    assert_eq!(holder_projections, vec![(0, 0, 0)]);
}

#[test]
fn level_two_castle_public_route_retries_root_and_child_failures_and_upgrades() {
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let request = PlayerCommand {
        opcode: 6,
        argument_1: 2,
        flags: 0x40,
        ..PlayerCommand::default()
    };

    let mut root_failed = level_two_campaign_session_with_owner_cache(&gameplay_cache);
    root_failed.select_level2_campaign_entry_stage();
    root_failed.step_outer_turn(request, 0);
    let free_slots = root_failed.economy().free_slots.clone();
    root_failed.economy_mut().free_slots.clear();
    let mut free_before_root_tick = Vec::new();
    for _ in 0..32 {
        // Other live source actors can be reclaimed between outer turns.
        // In this owner package Bee 46 is reclaimed before the Castle flyer
        // lands, so keep the failure prerequisite exhausted on each turn.
        root_failed.economy_mut().free_slots.clear();
        free_before_root_tick = root_failed.economy().free_slots.clone();
        root_failed.step_outer_turn(PlayerCommand::default(), 0);
        if root_failed.economy().castle().is_some() {
            break;
        }
    }
    assert_eq!(root_failed.economy().player_castle_entity_index(), 0);
    assert!(
        root_failed.economy().castle().is_none(),
        "unexpected root slot {:?}, generation {:?}; free stack before tick: len {}, next {:?}; after tick: len {}, next {:?}",
        root_failed.economy().castle().map(|root| root.source_entity_index),
        root_failed.economy().castle().map(|root| root_failed.economy().slot_generation(root.source_entity_index)),
        free_before_root_tick.len(),
        free_before_root_tick.last(),
        root_failed.economy().free_slots.len(),
        root_failed.economy().free_slots.last(),
    );
    assert!(
        root_failed
            .economy()
            .castle_projectiles()
            .iter()
            .any(|projectile| projectile.active)
    );
    root_failed.economy_mut().free_slots = vec![
        *free_slots
            .last()
            .expect("the qualified arena retains a root retry slot"),
    ];
    let mut root = None;
    for _ in 0..32 {
        root_failed.step_outer_turn(PlayerCommand::default(), 0);
        root = root_failed.economy().castle();
        if root.is_some() {
            break;
        }
    }
    let root = root.expect("root allocation retries after the arena is restored");
    assert_eq!(
        (root.source_class, root.source_model, root.action),
        (3, 2, 5)
    );
    assert_eq!(root.source_word_2e, 0);
    assert_eq!(root_failed.economy().player_castle_entity_index(), 0);

    let mut child_failed = level_two_campaign_session_with_owner_cache(&gameplay_cache);
    child_failed.select_level2_campaign_entry_stage();
    child_failed.step_outer_turn(request, 0);
    let free_slots = child_failed.economy().free_slots.clone();
    child_failed.economy_mut().free_slots = vec![free_slots[0]];
    for _ in 0..32 {
        // Admit exactly the seed slot until landing, despite unrelated source
        // rows reclaimed during earlier turns. The construction child then
        // encounters the real empty stack in the same source pass.
        child_failed.economy_mut().free_slots = vec![free_slots[0]];
        child_failed.step_outer_turn(PlayerCommand::default(), 0);
        if child_failed.economy().castle().is_some() {
            break;
        }
    }
    let root_slot = child_failed
        .economy()
        .castle()
        .expect("Castle root seed")
        .source_entity_index;
    let failed_root = child_failed
        .economy()
        .castle()
        .expect("root remains after child allocation failure");
    assert_eq!(failed_root.source_entity_index, root_slot);
    assert_eq!(failed_root.source_word_2e, 0);
    assert_eq!(
        failed_root.phase,
        (root_slot as u8).wrapping_add(1),
        "the initial Castle callback's allocation-failure return still receives the common source epilogue"
    );
    assert_eq!(child_failed.economy().player_castle_entity_index(), 0);
    assert!(
        !child_failed
            .economy()
            .events()
            .iter()
            .any(|event| matches!(event, Level2EconomyEvent::CastleBuilt { .. }))
    );
    child_failed.economy_mut().free_slots = free_slots;
    for _ in 0..8 {
        child_failed.step_outer_turn(PlayerCommand::default(), 0);
        if child_failed
            .economy()
            .castle()
            .is_some_and(|castle| castle.source_word_2e == 4)
        {
            break;
        }
    }
    let completed_root = child_failed.economy().castle().expect("root after retry");
    assert_eq!(
        child_failed.economy().player_castle_entity_index(),
        completed_root.source_entity_index
    );
    assert_eq!(completed_root.source_word_2e, 4);
    assert!(
        !child_failed
            .economy()
            .events()
            .iter()
            .any(|event| matches!(event, Level2EconomyEvent::CastleBuilt { .. }))
    );

    let mut occupied = level_two_campaign_session_with_owner_cache(&gameplay_cache);
    occupied.select_level2_campaign_entry_stage();
    occupied.step_outer_turn(request, 0);
    for _ in 0..32 {
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        if occupied.economy().castle().is_some() {
            break;
        }
    }
    let root_slot = occupied
        .economy()
        .castle()
        .expect("Castle root before construction callback")
        .source_entity_index;
    let busy_holder_value = occupied
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .expect("family-2 holder remains busy during construction")
        .active_value;
    assert_ne!(busy_holder_value, 0);
    occupied.step_outer_turn(request, 0);
    assert!(
        occupied
            .economy()
            .castle_projectiles()
            .iter()
            .all(|projectile| !projectile.active),
        "nonzero family-2 cooldown rejects an early public recast"
    );
    assert_eq!(
        occupied.economy().castle().map(|castle| castle.build_state),
        Some(4),
        "the early recast leaves the model-42 construction child active"
    );
    let construction_slot = occupied
        .economy()
        .castle()
        .expect("Castle root with construction slot")
        .construction_slot;
    let construction_child_before_turn = occupied
        .economy()
        .castle_children()
        .iter()
        .find(|child| {
            child.active && child.source_entity_index == construction_slot && child.model == 42
        })
        .copied()
        .expect("model-42 construction child before source reclaim");
    let construction_generation = occupied.economy().slot_generation(construction_slot);
    let mut construction_finished = false;
    for _ in 0..64 {
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        if occupied
            .economy()
            .castle()
            .is_some_and(|castle| (castle.action, castle.build_state) == (4, 0))
        {
            construction_finished = true;
            break;
        }
    }
    assert!(
        construction_finished,
        "37BC0 construction completes in bounded turns"
    );
    let completed_root = occupied
        .economy()
        .castle()
        .expect("completed callback root");
    assert_eq!((completed_root.action, completed_root.build_state), (4, 0));
    // The higher-slot BUILD callback hides the child after the lower-slot
    // root was already visited. On the next UpdateEntities invocation the
    // prologue reclaims that hidden row, then the root consumes build-state 2
    // and enters action 4. Therefore observing the completed root also means
    // the old child identity has already crossed its following-pass boundary.
    let following = occupied.clone();
    let old_identity_still_live = following.economy().castle_children().iter().any(|child| {
        child.active
            && child.source_entity_index == construction_slot
            && child.source_class == construction_child_before_turn.source_class
            && child.model == construction_child_before_turn.model
            && child.action == construction_child_before_turn.action
            && child.id_entity_index == construction_child_before_turn.id_entity_index
            && child.owner_entity_index == construction_child_before_turn.owner_entity_index
            && child.parent_entity_index == construction_child_before_turn.parent_entity_index
    });
    assert!(
        !old_identity_still_live
            || following.economy().slot_generation(construction_slot) != construction_generation,
        "following UpdateEntities prologue must remove the old model-42 generation"
    );
    let slot_owner = following
        .economy()
        .live_arena_cursor_rows()
        .into_iter()
        .find(|row| row.source_slot == construction_slot);
    match slot_owner {
        None => {
            assert!(
                !following
                    .economy()
                    .map_chains()
                    .is_linked(construction_slot)
            );
            assert!(following.economy().free_slots.contains(&construction_slot));
        }
        Some(row) => {
            // The source cursor may reuse the reclaimed lowest slot in
            // the following ordinary turn; require a new owner generation.
            assert_ne!(row.generation, construction_generation);
            assert_eq!(row.kind, Level2DynamicArenaKind::CastleDefender);
            assert!(!following.economy().free_slots.contains(&construction_slot));
            assert!(
                following
                    .economy()
                    .map_chains()
                    .is_linked(construction_slot)
            );
        }
    }
    assert_eq!(
        occupied
            .shared_spells()
            .dynamic_pickups
            .iter()
            .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
            .expect("family-2 holder remains authoritative after root consumption")
            .active_value,
        0,
        "root state 2 consumes the busy holder in the same source-ordered pass"
    );
    assert_eq!(completed_root.shift_yaw, -8_192);
    assert_eq!(completed_root.shift_fov, 0x4000);
    // The real completed root also supplies the state-3 retry control.
    // sub_5F890 runs before the constructor whether NewEvent succeeds or
    // fails. Only arena capacity differs between these two source visits.
    for allocation_available in [false, true] {
        let mut retry = occupied.clone();
        let holder_slot = retry.shared_spells().local_player.enabled_spell_holders[2];
        assert_eq!(retry.shared_spells().pickup(holder_slot).unwrap().active_value, 0);
        {
            let mut economy = retry.economy_mut();
            let root = economy.castle_at_mut(root_slot).unwrap();
            root.action = 5;
            root.build_state = 3;
            root.source_word_2e = 3;
        }
        if !allocation_available {
            retry.economy_mut().free_slots.clear();
        }
        retry.step_outer_turn(PlayerCommand::default(), 0);
        assert!(retry.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "Castle source retry must dispatch: {:?}", retry.dispatch_events());
        assert_ne!(retry.shared_spells().pickup(holder_slot).unwrap().active_value, 0,
            "state 3 marks its actual holder busy before allocation, available={allocation_available}");
        assert_eq!(retry.economy().castle_at(root_slot).unwrap().build_state,
            if allocation_available { 4 } else { 3 },
            "only the successful constructor advances the real root");
    }
    for valid_parent in [false, true] {
        let mut release = occupied.clone();
        let holder_slot = release.shared_spells().local_player.enabled_spell_holders[2];
        {
            let mut economy = release.economy_mut();
            let root = economy.castle_at_mut(root_slot).unwrap();
            root.action = 5;
            root.build_state = 2;
            root.source_word_2e = 2;
            root.auxiliary_cooldown = 37;
        }
        if !valid_parent {
            let live = &mut release.session.state.live;
            let mut holder = live.shared_spells.pickup(holder_slot).unwrap().clone();
            holder.record[44..46].copy_from_slice(&0_u16.to_le_bytes());
            live.shared_spells.sync_pickup_from_authority(&holder);
            live.economy.entities[usize::from(holder_slot)] = holder;
        }
        release.step_outer_turn(PlayerCommand::default(), 0);
        assert!(release.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "Castle state-2 release must dispatch: {:?}", release.dispatch_events());
        let root = release.economy().castle_at(root_slot).unwrap();
        assert_eq!((root.action, root.build_state, root.source_word_2e), (4, 0, 0));
        assert_eq!(root.auxiliary_cooldown, if valid_parent { 0 } else { 37 },
            "6D880 clears the root word2c only when its owner's actual holder resolves");
    }
    let occupied_holder = occupied
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .expect("family-2 holder remains authoritative after construction");
    let occupied_mana_cost = occupied_holder.mana_cost;
    let holder_max_mana = occupied_holder.max_mana;
    let occupied_tier = crate::spell_activation::level2_applied_tier(occupied_holder.level_gate)
        .expect("the acquired Castle holder has an applied source tier");
    let next_castle_stage = occupied
        .economy()
        .castle()
        .expect("the first upgrade retains its real Castle")
        .level
        .checked_add(1)
        .expect("this ordinary upgrade has a following Castle stage");
    // sub_68D50 gates on the holder's full maxMana. The profile's mana
    // field is only the per-pulse projectile payload, not the cast price.
    let repeat_cost = occupied
        .shared_spells()
        .physical_spell_profile(
            CASTLE_SPELL_FAMILY,
            occupied_tier,
            occupied.state.entity_index,
            Some(i32::from(next_castle_stage)),
        )
        .expect("the actual next-stage Castle price is defined")
        .max_mana;
    let required_capacity = occupied_mana_cost.max(repeat_cost);
    // The real reset capacity is only 1,000. Waiting at the Castle cannot
    // fund an upgrade until the player actually owns more mana. The final
    // repeated cast pays the new stage's price, so collect its capacity too.
    // Use the ordinary row-11 switch and its authored group-1 spheres,
    // followed by hand casts and the normal ownership/capacity boundary.
    // Position and aim are controlled inputs to this focused regression;
    // no mana, capacity, ownership mailbox or mission completion is granted.
    let player_slot = occupied.state.entity_index;
    let recharge_position = occupied.state.position;
    let recharge_yaw = occupied.state.yaw;
    let switch_position = occupied
        .mission()
        .authored_features()
        .iter()
        .find(|feature| feature.index == 11)
        .expect("the package retains the ordinary group-1 switch")
        .world_position(0);
    let mana_positions = occupied
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| feature.activation_group == 1 && feature.kind == 10 && feature.subtype == 39)
        .map(|feature| {
            let position = feature.world_position(0);
            (position.x, position.y)
        })
        .collect::<BTreeSet<_>>();
    assert!(!mana_positions.is_empty());
    // Source class-11/model-0 tests overlap only when phase & 7 == 0.
    // Construction has already advanced that phase; hold the declared
    // position input through one complete eight-visit source cadence.
    for _ in 0..8 {
        occupied
            .set_level2_entity_position(player_slot, switch_position)
            .expect("controlled overlap with the real mana switch");
        assert!(occupied.level2_authored_switch_matches(11).unwrap());
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        assert!(occupied.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "the ordinary mana producer must commit: {:?}", occupied.dispatch_events());
        if occupied.economy().spheres().iter().any(|sphere| {
            sphere.active && !sphere.pending_delete
                && mana_positions.contains(&(sphere.position.x, sphere.position.y))
        }) {
            break;
        }
    }
    assert!(occupied.economy().spheres().iter().any(|sphere| {
        sphere.active && !sphere.pending_delete
            && mana_positions.contains(&(sphere.position.x, sphere.position.y))
    }), "the real switch must expose its authored model-39 products");
    let mana_sources = occupied.economy().spheres().iter()
        .filter(|sphere| sphere.active && !sphere.pending_delete
            && mana_positions.contains(&(sphere.position.x, sphere.position.y)))
        .map(|sphere| occupied.session.state.live.economy.handle(sphere.source_entity_index)
            .expect("the ordinary producer publishes a generation-bound mana row"))
        .collect::<Vec<_>>();
    assert!(!mana_sources.is_empty());
    for _ in 0..128 {
        if occupied.economy().spheres().iter().any(|sphere| {
            sphere.active && !sphere.pending_delete && sphere.player_entity_index == 0
                && sphere.vertical_velocity == 0
                && mana_positions.contains(&(sphere.position.x, sphere.position.y))
        }) {
            break;
        }
        occupied.step_outer_turn(PlayerCommand::default(), 0);
    }
    let possession_ui = crate::SOURCE_SPELL_INDEX_MAP.iter()
        .position(|family| *family == 1).expect("public Possession menu family") as i8;
    let acquire_mana = |occupied: &mut Mc2Simulation, required_capacity: i32, require_switch_product: bool| {
    for claim in 0..mana_sources.len() {
        if occupied.shared_spells().player_entity().max_mana >= required_capacity {
            break;
        }
        // Retire the previous player's flyer and impact before attributing
        // another public cast. Attraction fields remain live as in the game.
        for _ in 0..32 {
            if occupied.economy().possession_effects().iter().all(|effect| {
                    !effect.active || effect.source_id_entity_index != player_slot
                })
                && occupied.economy().possession_projectiles().iter().all(|projectile| {
                    !projectile.active || projectile.source_id_entity_index != player_slot
                })
            {
                break;
            }
            occupied.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert!(occupied.economy().possession_effects().iter().all(|effect| {
            !effect.active || effect.source_id_entity_index != player_slot
        }));
        assert!(occupied.economy().possession_projectiles().iter().all(|projectile| {
            !projectile.active || projectile.source_id_entity_index != player_slot
        }));
        occupied.set_level2_entity_position(player_slot, recharge_position).unwrap();
        let holder_slot = occupied.shared_spells().local_player.enabled_spell_holders[1];
        for _ in 0..32 {
            let holder = occupied.shared_spells().pickup(holder_slot)
                .expect("the real player retains its Possession holder");
            if holder.active_value == 0 && occupied.shared_spells().caster.mana >= holder.mana_cost {
                break;
            }
            occupied.step_outer_turn(PlayerCommand::default(), 0);
        }
        // A source merge immediately reclaims the absorbed slot, so the
        // original handles cannot identify every remaining mana sphere. The
        // first claim still proves the switch product; later casts may claim
        // any live, eligible sphere produced by this ordinary session.
        let sphere = occupied.economy().spheres().iter()
            .filter(|sphere| sphere.active && !sphere.pending_delete
                // Spheres roll and rival carriers can move/claim them.
                // Tier-0 Possession admits an unprotected different owner,
                // rather than only neutral rows still at their birth point.
                && sphere.player_entity_index != player_slot
                && sphere.control_flags & 0x0020_0000 == 0
                && occupied.session.state.live.economy.handle(sphere.source_entity_index)
                    .is_some_and(|handle| !require_switch_product || claim != 0 || mana_sources.contains(&handle)))
            .min_by_key(|sphere| {
                let original = occupied.session.state.live.economy.handle(sphere.source_entity_index)
                    .is_some_and(|handle| mana_sources.contains(&handle));
                (!original, sphere.attraction_source_entity_index != 0,
                    i32::from(sphere.velocity_x).abs() + i32::from(sphere.velocity_y).abs(),
                    crate::distance_3d_wrapped(recharge_position, sphere.position))
            })
            .copied().unwrap_or_else(|| panic!(
                "actual authored mana must remain claimable before both Castle casts are affordable: claim={claim}, max={}, first_cost={occupied_mana_cost}, repeat_cost={repeat_cost}, required={required_capacity}, owned={}, sources={mana_sources:?}, current={:?}",
                occupied.shared_spells().player_entity().max_mana,
                occupied.economy().owner_mana_aggregate(player_slot),
                occupied.economy().spheres().iter().map(|sphere| (
                    sphere.source_entity_index, sphere.position, sphere.player_entity_index,
                    sphere.control_flags, sphere.mana, sphere.active, sphere.pending_delete,
                )).collect::<Vec<_>>()));
        let target = occupied.possession_guidance_target_top_for_test(sphere.source_entity_index)
            .expect("the live sphere supplies its real guidance top");
        let mut approach = Position {
            x: target.x.wrapping_sub(768),
            y: target.y,
            z: target.z.wrapping_add(512),
        };
        approach.z = approach.z.max(occupied.canonical_terrain().ground_height(approach).wrapping_add(512));
        occupied.set_level2_entity_position(player_slot, approach).unwrap();
        occupied.state.target_speed = 0;
        occupied.state.actual_speed = 0;
        let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(approach, target);
        // Publish the declared aim to the same source player and pitch
        // accumulator as the other controlled public-cast checks. Assigning
        // only display_pitch lets the intervening flight step reset the
        // launch to horizontal before Possession can acquire its real target.
        occupied.set_level2_player_aim_for_test(yaw, pitch);
        let capacity_before_claim = occupied.shared_spells().player_entity().max_mana;
        let event_start = occupied.economy().events().len();
        occupied.step_outer_turn(PlayerCommand {
            opcode: 6,
            argument_1: possession_ui,
            flags: 0x40,
            ..PlayerCommand::default()
        }, 0);
        let mut saw_flyer = false;
        let mut saw_effect = false;
        let mut claimed_sphere = None;
        let mut flyer_slot = None;
        let mut ownership_trace = Vec::new();
        for turn in 0..32 {
            ownership_trace.push((
                turn,
                occupied.session.state.live.economy.handle(sphere.source_entity_index),
                occupied.economy().spheres().iter()
                    .find(|row| row.source_entity_index == sphere.source_entity_index).copied(),
                occupied.economy().possession_projectiles().iter()
                    .filter(|row| row.source_id_entity_index == player_slot)
                    .copied().collect::<Vec<_>>(),
                occupied.economy().possession_effects().iter()
                    .filter(|row| row.source_id_entity_index == player_slot)
                    .copied().collect::<Vec<_>>(),
                occupied.state.position,
            ));
            if let Some(flyer) = occupied.economy().possession_projectiles().iter()
                .find(|row| row.source_id_entity_index == player_slot)
            {
                saw_flyer = true;
                flyer_slot.get_or_insert(flyer.source_entity_index);
            }
            saw_effect |= occupied.economy().possession_effects().iter()
                .any(|row| row.source_id_entity_index == player_slot
                    && Some(row.projectile_entity_index) == flyer_slot);
            claimed_sphere = claimed_sphere.or_else(|| occupied.economy().events()[event_start..].iter().find_map(|event| match event {
                Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                    if *owner_entity_index == player_slot && (!require_switch_product || claim != 0 || *sphere_id == sphere.id) => Some(*sphere_id),
                _ => None,
            }));
            assert!(occupied.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "public Possession must commit: {:?}", occupied.dispatch_events());
            if claimed_sphere.is_some() {
                break;
            }
            occupied.step_outer_turn(PlayerCommand::default(), 0);
        }
        assert!(saw_flyer && saw_effect && claimed_sphere.is_some(),
            "claim {claim} must use the real hand, flyer, effect and owner mailbox: flyer={saw_flyer}, effect={saw_effect}, claim={claimed_sphere:?}, target={sphere:?}, source_trace={ownership_trace:#?}");
        // Claiming is not allowed to publish sub_60F00's capacity early.
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        assert!(occupied.shared_spells().player_entity().max_mana > capacity_before_claim,
            "source ownership must increase real player capacity: claimed={claimed_sphere:?}, before={capacity_before_claim}, after={}",
            occupied.shared_spells().player_entity().max_mana);
    }
    };
    acquire_mana(&mut occupied, required_capacity, true);
    occupied.set_level2_entity_position(player_slot, recharge_position).unwrap();
    occupied.state.yaw = recharge_yaw;
    occupied.state.display_pitch = 0;
    occupied.state.effective_pitch = 0;
    occupied.state.target_speed = 0;
    occupied.state.actual_speed = 0;
    assert!(occupied.shared_spells().player_entity().max_mana >= required_capacity,
        "actual owned source mana must supply both Castle cast capacities: max={}, first_cost={occupied_mana_cost}, repeat_cost={repeat_cost}, sources={mana_sources:?}, current={:?}",
        occupied.shared_spells().player_entity().max_mana,
        occupied.economy().spheres());
    let player_before_recharge = occupied.shared_spells().player_entity().clone();
    let overlap_before_recharge = occupied
        .economy()
        .player_castle_shift_aabb_overlap(&player_before_recharge);
    let mut recharge_turns = 0_u8;
    while recharge_turns < 32 && occupied.shared_spells().caster.mana < occupied_mana_cost {
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        recharge_turns = recharge_turns.saturating_add(1);
    }
    assert!(
        occupied.shared_spells().caster.mana >= occupied_mana_cost,
        "ordinary public upkeep must reach the holder cost before recast: turns={recharge_turns}, cost={occupied_mana_cost}, holder_max={holder_max_mana}, start_mana={}, start_regen={}, start_max={}, start_position={:?}, start_overlap={overlap_before_recharge}, end_mana={}, end_regen={}, end_max={}, end_position={:?}, end_overlap={}",
        player_before_recharge.mana,
        player_before_recharge.mana_regen,
        player_before_recharge.max_mana,
        player_before_recharge.position,
        occupied.shared_spells().caster.mana,
        occupied.shared_spells().player_entity().mana_regen,
        occupied.shared_spells().player_entity().max_mana,
        occupied.shared_spells().player_entity().position,
        occupied.economy().player_castle_shift_aabb_overlap(occupied.shared_spells().player_entity()),
    );
    occupied.step_outer_turn(request, 0);
    let upgrade = occupied
        .economy()
        .castle_projectiles()
        .iter()
        .find(|projectile| projectile.target_castle_slot == root_slot)
        .copied()
        .expect("occupied Castle allocates an upgrade flyer");
    assert_eq!((upgrade.source_byte_43, upgrade.source_byte_44), (10, 43));
    assert_eq!(upgrade.source_word_96, root_slot);
    assert_eq!(upgrade.target_castle_slot, root_slot);
    assert!(!upgrade.active);
    assert!(upgrade.drawing_disabled);
    assert_eq!(
        occupied
            .economy()
            .live_arena_slots()
            .iter()
            .filter(|(_, kind)| *kind == Level2DynamicArenaKind::CastleRoot)
            .count(),
        1
    );
    let upgrade_product_before = occupied
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.active && child.model == 43 && child.action == 45)
        .copied()
        .expect("the source landing constructs its actual 10/43 product");
    let upgrade_product_generation = occupied
        .economy()
        .slot_generation(upgrade_product_before.source_entity_index);
    let upgrade_product_handle = occupied.session.state.live.economy
        .handle(upgrade_product_before.source_entity_index)
        .expect("the landed product has a source generation");
    let product_dispatched = |session: &Mc2Simulation| {
        session.dispatch_events().iter().any(|event| {
            matches!(event, Mc2DispatchEvent::DynamicArena {
            pass: 0, source_slot, kind: Level2DynamicArenaKind::CastleChild,
        } if *source_slot == upgrade_product_before.source_entity_index)
        })
    };
    let mut product_callback_prediction = None;
    if !product_dispatched(&occupied) {
        // Observe the exact source producer on an immutable execution clone
        // before the ordinary numeric pass can also consume its mailbox.
        let mut callback_probe = occupied.clone();
        assert!(callback_probe.step_level2_dynamic_arena(
            0,
            upgrade_product_before.source_entity_index,
            Level2DynamicArenaKind::CastleChild
        ));
        let probe_root = callback_probe
            .economy()
            .castle()
            .expect("real callback target");
        assert_eq!(probe_root.source_word_7c, 10);
        assert_eq!(probe_root.source_word_80, upgrade.owner_entity_index);
        product_callback_prediction = callback_probe
            .economy()
            .castle_children()
            .iter()
            .find(|child| child.source_entity_index == upgrade_product_before.source_entity_index)
            .copied();
        assert!(
            upgrade_product_before.source_entity_index < upgrade.source_entity_index,
            "only a real lower-slot product can await the next source scan: flyer={upgrade:?} product={upgrade_product_before:?} dispatch={:?}",
            occupied.dispatch_events()
        );
        assert_eq!(
            upgrade_product_before.control_flags & ENTITY_DRAW_DISABLED_BIT,
            0
        );
        assert_eq!(upgrade_product_before.life, 8);
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        assert_eq!(
            occupied
                .economy()
                .slot_generation(upgrade_product_before.source_entity_index),
            upgrade_product_generation
        );
    }
    assert!(
        product_dispatched(&occupied),
        "the generation-qualified 10/43 product must run on its first reachable numeric source visit"
    );
    assert!(
        occupied.dispatch_events().iter().all(|event| !matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                kind: Level2DynamicArenaKind::CastleProjectile,
                ..
            }
        )),
        "the immediately consumed flyer does not publish a live-projectile callback"
    );
    let upgrade_spawn_position = upgrade.position;
    let landed_root = occupied.economy().castle().expect("live Castle root");
    let same_pass_consumer = if landed_root.source_word_80 == 0 {
        assert!(
            upgrade_product_before.source_entity_index < root_slot,
            "only an earlier source product can have its mailbox consumed by this root in the same pass"
        );
        assert_eq!((landed_root.action, landed_root.build_state), (5, 0));
        assert_eq!(landed_root.source_word_80, 0);
        assert_eq!(landed_root.source_word_7c, 10);
        assert_ne!(landed_root.control_flags & 0x40, 0);
        assert!(occupied.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, selector, .. }
                if *slot == root_slot
                    && selector.registry_key().selector == crate::campaign::spec::BehaviorSelector::ClassAction {
                        class: 3, action: 4,
                    })), "the exact real action4 consumer must run after the product");
        Some(landed_root)
    } else {
        assert_eq!(landed_root.source_word_7c, 10);
        assert_eq!(landed_root.source_word_80, upgrade.owner_entity_index);
        None
    };
    let upgrade_product_slot = occupied
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.active && child.model == 43 && child.action == 45)
        .map(|child| child.source_entity_index)
        .expect("the 10/43 callback leaves its disabled product live");
    let upgrade_product = occupied
        .economy()
        .castle_children()
        .iter()
        .find(|child| child.source_entity_index == upgrade_product_slot)
        .expect("landed 10/43 product");
    assert_eq!(
        upgrade_product.position, upgrade_spawn_position,
        "the flyer terrain landing is carried unchanged into the higher-slot 10/43 product"
    );
    if let Some(predicted) = product_callback_prediction {
        assert_eq!(
            *upgrade_product, predicted,
            "the ordinary source visit must match the exact unmodified producer callback prediction"
        );
    }
    assert_eq!(upgrade_product.parent_entity_index, root_slot);
    assert_ne!(upgrade_product.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
    assert_ne!(upgrade_product.control_flags & 0x2, 0);
    assert!(
        occupied
            .economy()
            .map_chains()
            .is_linked(upgrade_product_slot)
    );
    assert!(
        !occupied
            .economy()
            .free_slots
            .contains(&upgrade_product_slot)
    );
    occupied.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(occupied.economy().player_castle_entity_index(), root_slot);
    assert!(occupied.economy().castle_projectiles().is_empty());
    assert!(occupied.session.state.live.economy.resolve(upgrade_product_handle).is_none(),
        "the old model-43 generation must be reclaimed at the next source boundary");
    assert!(!occupied.economy().castle_children().iter().any(|child| {
        child.source_entity_index == upgrade_product_slot && child.model == 43 && child.action == 45
    }), "the old model-43 child must not survive beside a replacement");
    let replacement = occupied.economy().live_arena_cursor_rows().into_iter()
        .find(|row| row.source_slot == upgrade_product_slot);
    if let Some(replacement) = replacement {
        assert_ne!(replacement.generation, upgrade_product_generation);
        assert_eq!(replacement.kind, Level2DynamicArenaKind::CastleChild);
        let child = occupied.economy().castle_children().iter()
            .find(|child| child.source_entity_index == upgrade_product_slot)
            .expect("reused slot is the real construction child");
        assert_eq!((child.source_class, child.model, child.action, child.parent_entity_index),
            (10, 42, 44, root_slot));
        assert!(child.active);
        assert_eq!(child.control_flags & 0x4, 0);
        // Source sub_50370 copies position directly; model42 is not map-linked.
        assert!(!occupied.economy().map_chains().is_linked(upgrade_product_slot));
        assert!(!occupied.economy().free_slots.contains(&upgrade_product_slot));
    } else {
        assert!(!occupied.economy().map_chains().is_linked(upgrade_product_slot));
        assert!(occupied.economy().free_slots.contains(&upgrade_product_slot));
    }
    let mailbox_root = same_pass_consumer.unwrap_or_else(|| {
        occupied
            .economy()
            .castle()
            .expect("Castle root after its first mailbox-consumer visit")
    });
    assert_eq!((mailbox_root.action, mailbox_root.build_state), (5, 0));
    assert_ne!(mailbox_root.control_flags & 0x40, 0);
    assert_eq!(mailbox_root.source_word_80, 0);
    assert!(
        !occupied
            .economy()
            .events()
            .iter()
            .any(|event| matches!(event, Level2EconomyEvent::CastleBuilt { .. }))
    );
    let mut holder_cleared = false;
    for _ in 0..32 {
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        holder_cleared = occupied
            .shared_spells()
            .dynamic_pickups
            .iter()
            .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
            .is_some_and(|holder| holder.active_value == 0);
        if holder_cleared {
            break;
        }
    }
    assert!(
        holder_cleared,
        "upgrade construction eventually releases holder"
    );
    let recast_holder = occupied
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
        .expect("the real Castle holder survives its release")
        .clone();
    let recast_mana_before = occupied.shared_spells().caster.mana;
    // Holder release is distinct from paying for another upgrade. Reuse the
    // same ordinary upkeep path that funded the first public cast; no mana
    // or capacity is granted to make a repeated cast succeed.
    for _ in 0..4 {
        acquire_mana(&mut occupied, recast_holder.mana_cost, false);
        occupied.set_level2_entity_position(player_slot, recharge_position).unwrap();
        occupied.state.yaw = recharge_yaw;
        occupied.state.display_pitch = 0;
        occupied.state.effective_pitch = 0;
        occupied.state.target_speed = 0;
        occupied.state.actual_speed = 0;
        for _ in 0..32 {
            if occupied.shared_spells().caster.mana >= recast_holder.mana_cost {
                break;
            }
            occupied.step_outer_turn(PlayerCommand::default(), 0);
        }
        if occupied.shared_spells().caster.mana >= recast_holder.mana_cost {
            break;
        }
    }
    assert!(
        occupied.shared_spells().caster.mana >= recast_holder.mana_cost,
        "ordinary upkeep must fund the actual repeat cost: before={recast_mana_before}, after={}, cost={}, max={}, root={:?}, position={:?}, overlap={}",
        occupied.shared_spells().caster.mana,
        recast_holder.mana_cost,
        occupied.shared_spells().player_entity().max_mana,
        occupied.economy().castle(),
        occupied.state.position,
        occupied.economy().player_castle_shift_aabb_overlap(occupied.shared_spells().player_entity()),
    );
    let recast_caster = occupied.shared_spells().player_entity().clone();
    let recast_root = occupied.economy().castle().expect("live stage-two root");
    assert_eq!((recast_root.source_entity_index, recast_root.level), (root_slot, 2));
    let root_generation = occupied.economy().slot_generation(root_slot);
    occupied.step_outer_turn(request, 0);
    assert!(
        occupied
            .economy()
            .castle_projectiles()
            .iter()
            .any(|projectile| {
                (projectile.source_byte_43, projectile.source_byte_44) == (10, 43)
            }),
        "a funded later public recast allocates a new upgrade flyer even when occupied-root contact retires it in the same pass: holder={recast_holder:?}, caster={recast_caster:?}, root={recast_root:?}, after_caster={:?}, events={:?}, dispatch={:?}",
        occupied.shared_spells().player_entity(),
        occupied.economy().events(),
        occupied.dispatch_events(),
    );
    assert!(
        occupied
            .shared_spells()
            .dynamic_pickups
            .iter()
            .find(|holder| holder.model == CASTLE_SPELL_FAMILY)
            .is_some_and(|holder| holder.active_value > 0),
        "the successful recast advances the family-2 holder lifecycle"
    );
    let after_launch = occupied.shared_spells().player_entity();
    assert!(after_launch.mana < recast_caster.mana || after_launch.mana_regen <= -recast_holder.mana_cost,
        "the public recast must apply or queue the full source charge: before={}, after={}, regen={}, cost={}",
        recast_caster.mana, after_launch.mana, after_launch.mana_regen, recast_holder.mana_cost);
    let mut completed = false;
    for _ in 0..128 {
        if occupied.economy().castle_at(root_slot).is_some_and(|castle| {
            (castle.level, castle.action, castle.build_state) == (3, 4, 0)
        }) {
            completed = true;
            break;
        }
        occupied.step_outer_turn(PlayerCommand::default(), 0);
        assert!(occupied.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
            "the real repeat upgrade must continue dispatching: {:?}", occupied.dispatch_events());
    }
    assert!(completed, "the funded repeat must finish the actual stage-three Castle");
    let root = occupied.economy().castle_at(root_slot).expect("upgraded Castle root");
    assert_eq!(occupied.economy().slot_generation(root_slot), root_generation);
    assert_eq!((root.source_id_entity_index, root.player_entity_index, root.mana_capacity),
        (player_slot, player_slot, 38_800));
    assert_eq!(occupied.economy().player_castle_entity_index(), root_slot);
    assert!(occupied.shared_spells().pickup(occupied.shared_spells().local_player.enabled_spell_holders[2])
        .is_some_and(|holder| holder.active_value == 0), "completed construction releases its real holder");
    let sprites = occupied.campaign_entity_presentations(0).expect("Castle presentation census");
    let sprite = sprites.iter().find(|row| row.entity_index == root_slot)
        .expect("upgraded live Castle reaches presentation");
    assert_eq!((sprite.source_id, sprite.render_parameter_index, sprite.base_tmap),
        (player_slot, 177, 96));
}

#[test]
fn level_two_castle_upgrade_product_reclaims_at_next_entity_pass() {
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    let inspection =
        NativeWorld::inspect_level2_with_owner_cache(level_two_fixture_path(), &gameplay_cache)
            .expect("qualified Level 2 gameplay component");
    let mut session = inspection
        .level2_reset_session()
        .expect("multi-pass Castle upgrade session");
    let owner_entity_index = session.state.entity_index;
    let root_slot = session
        .economy_mut()
        .build_castle(
            Position {
                x: 8_192,
                y: 8_192,
                z: 0,
            },
            owner_entity_index,
        )
        .expect("real Castle relation");
    let root_position = session.economy().castle().expect("Castle root").position;
    let holder_slot = session
        .shared_spells()
        .dynamic_pickups
        .iter()
        .find(|holder| holder.model == 2)
        .expect("family-2 holder")
        .index;
    let flyer_slot = session
        .economy_mut()
        .spawn_castle_projectile(root_position, owner_entity_index, holder_slot, 0, 0, 0)
        .expect("occupied Castle flyer");
    let flyer_generation = session.economy().slot_generation(flyer_slot);
    let slot_generations_before = session.economy().slot_generations.clone();

    session.step_outer_turn(PlayerCommand::default(), 1);
    let product_event = session
        .dispatch_events()
        .iter()
        .find_map(|event| match event {
            Mc2DispatchEvent::DynamicArena {
                pass,
                source_slot,
                kind: Level2DynamicArenaKind::CastleChild,
            } => Some((*pass, *source_slot)),
            _ => None,
        })
        .expect("pass-zero landing dispatches the 10/43 product");
    assert_eq!(product_event.0, 0);
    let product_slot = product_event.1;
    let product_generation_before = slot_generations_before
        .get(&product_slot)
        .copied()
        .unwrap_or_default();
    assert_ne!(
        (product_slot, product_generation_before),
        (flyer_slot, flyer_generation),
        "the class-10 landing product has its own source identity"
    );
    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Mc2DispatchEvent::DynamicArena {
                        pass: 1,
                        source_slot,
                        ..
                    } if *source_slot == product_slot
                )
            })
            .count(),
        0,
        "the next source-pass prologue reclaims the disabled product before the cursor"
    );
    assert_eq!(session.economy().player_castle_entity_index(), root_slot);
    assert!(!session.economy().castle_children().iter().any(|child| {
        child.active && child.source_class == 10 && child.model == 43 && child.action == 45
    }));
    assert!(!session.economy().castle_children().iter().any(|child| {
        child.source_entity_index == product_slot && child.source_class == 10 && child.model == 43 && child.action == 45
    }), "the old product row must not survive inactive beside a replacement");
    let product_generation = product_generation_before.wrapping_add(1);
    let product_handle = crate::campaign::source_arena::SourceEntityHandle {
        source_slot: product_slot,
        generation: product_generation,
    };
    assert!(session.session.state.live.economy.resolve(product_handle).is_none(),
        "the original model-43 generation must not resolve after the source prepass");
    let final_generation = session.economy().slot_generation(product_slot);
    let replacement = session
        .economy()
        .live_arena_cursor_rows()
        .iter()
        .find(|row| row.source_slot == product_slot)
        .copied();
    if let Some(replacement) = replacement {
        assert!(
            final_generation > product_generation,
            "same-slot reuse must advance the source generation"
        );
        assert_eq!(replacement.kind, Level2DynamicArenaKind::CastleChild);
        let child = session.economy().castle_children().iter()
            .find(|child| child.source_entity_index == product_slot)
            .expect("reused slot is the construction child");
        assert_eq!((child.source_class, child.model, child.action, child.parent_entity_index),
            (10, 42, 44, root_slot));
        assert!(child.active);
        assert_eq!(child.control_flags & 0x4, 0);
        assert!(!session.economy().map_chains().is_linked(product_slot),
            "sub_50370 constructs model42 without map linkage");
        assert!(!session.economy().free_slots.contains(&product_slot));
    } else {
        assert_eq!(final_generation, product_generation);
        assert!(!session.economy().map_chains().is_linked(product_slot));
        assert!(session.economy().free_slots.contains(&product_slot));
    }
}
