#[test]
fn level_two_common_inspection_uses_typed_profile_and_exact_component_order() {
    let package = level_two_fixture_path();
    let inspection = NativeWorld::inspect_level2(&package).expect("qualified Level 2 package");
    assert_eq!(inspection.profile(), OwnerPackageProfile::Level2);
    assert_eq!(inspection.schema(), OwnerPackageSchema::V5);
    assert_eq!(
        inspection
            .collision_mask_bank()
            .map(|bank| bank.masks().len()),
        Some(9)
    );
    assert!(
        inspection
            .model45_occupancy(64, 0, 0)
            .expect("qualified v5 model-45 occupancy")
    );
    assert_eq!(inspection.metadata().external_level, 2);
    assert_eq!(inspection.metadata().internal_level, 1);
    assert_eq!(inspection.metadata().map_type, MapType::Day);
    assert_eq!(inspection.entities().len(), ENTITY_COUNT);
    assert_eq!(
        inspection
            .entities()
            .iter()
            .filter(|entity| entity.active)
            .count(),
        114
    );
    assert_eq!(inspection.players().len(), PLAYER_COUNT);
    assert_eq!(inspection.runtime().stages.len(), 8);
    assert_eq!(inspection.runtime().stage_variables.len(), 11);
    assert_eq!(inspection.runtime().objectives.len(), 8);
    assert_eq!(inspection.presentation().particles.len(), PARTICLE_COUNT);
    assert_eq!(
        inspection.presentation().level_sprite_selectors.len(),
        LEVEL_SPRITE_SELECTOR_COUNT
    );
    assert_eq!(
        inspection.presentation().entity_parameters.len(),
        ENTITY_PARAMETER_COUNT
    );
    assert_eq!(inspection.entities()[0].index, 0);
    assert_eq!(inspection.entities()[999].index, 999);
    assert_eq!(inspection.players()[0].slot, 0);
    assert_eq!(inspection.players()[7].slot, 7);
    assert_eq!(inspection.players()[0].left_spell_slot_raw, -1);
    assert_eq!(inspection.players()[0].right_spell_slot_raw, -1);
    let reset = inspection.reset_state().expect("Level 2 frame-zero reset");
    assert_eq!(reset.entity_index, 111);
    assert_eq!(
        reset.position,
        Position {
            x: 384,
            y: 62_080,
            z: 256
        }
    );
    assert_eq!(inspection.terrain_rng(), 9_858);
    assert!(inspection.mission_unimplemented().is_err());
}

#[test]
fn level_two_v4_rejects_non_cave_secondary_terrain_plane() {
    let package = level_two_fixture_path();
    let mut members = LEVEL2_V4_PACKAGE_MEMBERS
        .iter()
        .map(|(name, _)| ((*name).to_owned(), fs::read(package.join(name)).unwrap()))
        .collect::<BTreeMap<_, _>>();
    members.get_mut("terrain-pre-features.bin").unwrap()[0x40000] ^= 1;

    let error =
        validate_v4_non_cave_terrain_normalization(&members, OwnerPackageSchema::V4, MapType::Day)
            .expect_err("v4 Day must reject a nonzero inactive secondary terrain plane");
    assert!(
        error
            .to_string()
            .contains("terrain-pre-features.bin secondary terrain plane")
    );
}

#[test]
fn level_two_v3_package_remains_supported_without_type_str_or_collision_authority() {
    let inspection = NativeWorld::inspect_level2(&level_two_v3_fixture_path())
        .expect("historically supported Level 2 v3 package");
    assert_eq!(inspection.schema(), OwnerPackageSchema::V3);
    assert!(inspection.collision_mask_bank().is_none());
    assert!(inspection.model45_occupancy(64, 0, 0).is_err());
    assert_eq!(
        inspection.players()[0].record_bytes().len(),
        PLAYER_LEGACY_RECORD_BYTES
    );
    assert!(inspection.players()[0].type_str_164_auxiliary().is_none());
    assert!(inspection.players()[0].banishment_state_0x006.is_none());
    let error = inspection
        .level2_reset_session()
        .expect_err("missing banishment carrier must fail before session creation");
    assert!(
        error.to_string().contains("banishment-state"),
        "unexpected missing-carrier rejection: {error}"
    );
}

#[test]
fn level_two_v5_rejects_pre_v5_player_component_and_wrong_profile() {
    let package = level_two_fixture_path();
    let temp = TempLevel2V4Package::copy_from(&package);
    let players_path = temp.package.join("players.bin");
    let players = fs::read(&players_path).unwrap();
    let mut v3_players = Vec::with_capacity(12 + PLAYER_COUNT * PLAYER_LEVEL2_V4_RECORD_BYTES);
    v3_players.extend_from_slice(b"MCP1");
    v3_players.extend_from_slice(&3_u16.to_le_bytes());
    v3_players.extend_from_slice(&PLAYER_LEVEL2_V4_RECORD_BYTES_U16.to_le_bytes());
    v3_players.extend_from_slice(&PLAYER_COUNT_U32.to_le_bytes());
    for slot in 0..PLAYER_COUNT {
        let begin = 12 + slot * PLAYER_LEVEL2_RECORD_BYTES;
        v3_players.extend_from_slice(&players[begin..begin + PLAYER_LEVEL2_V4_RECORD_BYTES]);
    }
    fs::write(&players_path, v3_players).unwrap();
    let manifest_path = temp.package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"]["players.bin"]["bytes"] =
        json!(12 + PLAYER_COUNT * PLAYER_LEVEL2_V4_RECORD_BYTES);
    manifest["files"]["players.bin"]["sha256"] =
        json!(sha256_hex(&fs::read(&players_path).unwrap()));
    // The v5 package identity is refreshed, so rejection reaches the
    // versioned member-layout gate rather than a stale file hash.
    let mut identity = Vec::new();
    for (name, _) in LEVEL2_V5_PACKAGE_MEMBERS {
        let bytes = fs::read(temp.package.join(name)).unwrap();
        identity.extend_from_slice(name.as_bytes());
        identity.push(0);
        identity.extend_from_slice(sha256_hex(&bytes).as_bytes());
        identity.push(0);
        identity.extend_from_slice(bytes.len().to_string().as_bytes());
        identity.push(b'\n');
    }
    manifest["package_sha256"] = json!(sha256_hex(&identity));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        NativeWorld::inspect_level2(&temp.package).is_err(),
        "v5 manifest plus v3/662 player component must fail before session construction"
    );

    let temp = TempLevel2V4Package::copy_from(&package);
    let manifest_path = temp.package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["level"] =
        json!({"external": 1, "internal": 0, "map_type": {"id": 1, "name": "Night"}});
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        NativeWorld::inspect_level2(&temp.package).is_err(),
        "wrong Level 2 metadata must fail before session construction"
    );

    let temp = TempLevel2V4Package::copy_from(&package);
    let manifest_path = temp.package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["schema"] = json!(LEVEL2_MANIFEST_SCHEMA_V4);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        NativeWorld::inspect_level2(&temp.package).is_err(),
        "v4 manifest plus v4/663 player component must fail before session construction"
    );
}

#[test]
fn level_two_v5_gameplay_cache_binds_owner_component_identity() {
    let package = level_two_fixture_path();
    let cache = level_two_gameplay_cache_path();
    let inspection = NativeWorld::inspect_level2_with_owner_cache(&package, &cache)
        .expect("qualified v5 gameplay cache");
    assert!(inspection.level2_castle_stage_footprints.contains_key(&1));

    let root = std::env::temp_dir().join(format!(
        "mc2-level2-v5-gameplay-identity-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&root).expect("create v5 gameplay identity fixture");
    let manifest_path = root.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(cache.join("manifest.json")).unwrap()).unwrap();
    manifest["bindings"]["players_component_version"] = json!(3);
    let mut unsigned = manifest.clone();
    unsigned
        .as_object_mut()
        .expect("gameplay manifest object")
        .remove("manifest_payload_sha256");
    let canonical = serde_json::to_string_pretty(&unsigned).unwrap() + "\n";
    manifest["manifest_payload_sha256"] = json!(sha256_hex(canonical.as_bytes()));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let error = NativeWorld::inspect_level2_with_owner_cache(&package, &root)
        .expect_err("v5 gameplay cache with v3 player binding must fail closed");
    assert!(error.to_string().contains("players_component_version"));
    fs::remove_dir_all(root).expect("remove v5 gameplay identity fixture");
}

#[test]
fn level_two_v4_package_remains_inspectable_but_cannot_construct_session() {
    let inspection = NativeWorld::inspect_level2(level_two_v4_fixture_path())
        .expect("qualified v4 Level 2 package");
    assert_eq!(inspection.schema(), OwnerPackageSchema::V4);
    let bank = inspection
        .collision_mask_bank()
        .expect("v4 inspection retains the immutable collision bank");
    assert_eq!(bank.masks().len(), 9);
    assert!(
        inspection
            .model45_occupancy(64, 0, 0)
            .expect("model-45 selector 64 pixel is in bounds")
    );

    let error = inspection
        .level2_reset_session()
        .expect_err("v4 inspection must not construct a partial type-3 session");
    assert!(
        error.to_string().contains("banishment-state"),
        "v4 rejection must identify the missing authoritative carrier: {error}"
    );
}

#[test]
fn level_two_v4_package_open_rejects_selector64_after_all_manifest_hashes_refresh() {
    let temp = TempLevel2V4Package::copy_from(&level_two_v4_fixture_path());
    tamper_level_two_v4_selector_64_and_refresh_manifest(&temp.package);
    let error = NativeWorld::inspect_level2(&temp.package)
        .expect_err("dual-refresh selector-64 tamper must fail before session construction");
    assert!(
        error.to_string().contains("collision-mask bank"),
        "source identity must be the rejecting gate: {error}"
    );
}

#[test]
fn level_two_fresh_common_package_without_reset_is_explicitly_inspectable() {
    let source = level_two_v3_fixture_path();
    let root = std::env::temp_dir().join(format!(
        "mc2-level2-fresh-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&root).unwrap();
    for (name, _) in LEVEL2_PACKAGE_MEMBERS {
        fs::copy(source.join(name), root.join(name)).unwrap();
    }
    let manifest_path = root.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(source.join("manifest.json")).unwrap()).unwrap();
    manifest.as_object_mut().unwrap().remove("reset_projection");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let inspection = NativeWorld::inspect_level2(&root).expect("fresh Level 2 package");
    assert_eq!(inspection.metadata().external_level, 2);
    assert_eq!(
        inspection
            .reset_state()
            .expect("Level 2 frame-zero reset")
            .entity_index,
        111
    );
    let error = inspection.mission_unimplemented().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("mission behavior is unsupported")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn level_two_reset_session_matches_authoritative_order_and_isolation() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let template = inspection
        .mission_content_template()
        .expect("Level 2 content template");
    let wizard = template.wizard_map_settings();
    assert_eq!(wizard.len(), 8);
    assert_eq!(
        (
            wizard[0].aggression,
            wizard[0].reflexes,
            wizard[0].perception,
            wizard[0].life
        ),
        (203, 15, 28, 4)
    );
    assert_eq!(
        (
            wizard[1].aggression,
            wizard[1].reflexes,
            wizard[1].perception,
            wizard[1].life
        ),
        (243, 39, 90, 79)
    );
    assert_eq!(&wizard[0].starting_spells[..4], &[1, 1, 1, 1]);
    assert_eq!(&wizard[1].starting_spells[..4], &[1, 1, 0, 1]);
    assert_eq!(&wizard[1].blocked_spells[..5], &[0, 0, 1, 1, 1]);
    assert_eq!(wizard[0].record_bytes().len(), 110);
    let expected_wizard_hashes = [
        "922ab4bf6c34f644190a441cee4a187c37b8732cc5ba89997c8b241d950e7a84",
        "569d4f28bd524b619dfc9eb665f20b4817895cf53d14fe4efe8b4caf7abd250c",
        "a4921745a0b53cc469ea7040da8a3fbad53a04d5493c24f2ebd4dabf9071bc1b",
        "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
        "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
        "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
        "e1fb0004db6b25ec87db76ccbb5771c41864dfbcfd08f802832043d6676172e1",
        "a7a8b0d420d86ff48e6b99a869b7eb0690947089f79e6e9944aecb1bb2a0ba6a",
    ];
    for (settings, expected_hash) in wizard.iter().zip(expected_wizard_hashes) {
        assert_eq!(
            format!("{:x}", Sha256::digest(settings.record_bytes())),
            expected_hash
        );
    }

    let active: Vec<u16> = inspection
        .entities()
        .iter()
        .filter(|entity| entity.active)
        .map(|entity| entity.index)
        .collect();
    assert_eq!(active, (1_u16..=114).collect::<Vec<_>>());
    assert_eq!(
        (
            inspection.entities()[111].class,
            inspection.entities()[111].model
        ),
        (3, 0)
    );
    assert_eq!(
        (
            inspection.entities()[112].class,
            inspection.entities()[112].model
        ),
        (3, 1)
    );
    assert_eq!(
        (
            inspection.entities()[113].class,
            inspection.entities()[113].model,
            inspection.entities()[113].action
        ),
        (15, 0, 0)
    );
    assert_eq!(
        (
            inspection.entities()[114].class,
            inspection.entities()[114].model,
            inspection.entities()[114].action
        ),
        (15, 1, 3)
    );
    assert_eq!(inspection.players()[0].entity_index, 111);
    assert_eq!(inspection.players()[1].entity_index, 112);
    assert_eq!(
        &inspection.players()[1].enabled_spell_holders[..2],
        &[113, 114]
    );

    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let mut second = inspection
        .level2_reset_session()
        .expect("independent reset session");
    let state_hash = first.state_hash();
    let terrain_hash = first.terrain_hash();
    let mission_hash = first.mission().state_hash();
    assert_eq!(state_hash, 0x57c3_7985_e90c_696c);
    // GOLDEN-CHANGE: v4 zero-normalizes the inactive non-Cave terrain
    // plane instead of hashing uninitialized host memory.
    assert_eq!(terrain_hash, 0x9d76_0dcf_39ba_f22a);
    for _ in 0..19 {
        first.step_outer_turn(PlayerCommand::default(), 0);
        second.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(first.state_hash(), second.state_hash());
    assert_eq!(first.terrain_hash(), second.terrain_hash());
    let stepped_state_hash = second.state_hash();
    let stepped_terrain_hash = second.terrain_hash();
    let stepped_mission_hash = second.mission().state_hash();
    first.state.position.x = first.state.position.x.wrapping_add(1);
    first
        .mission
        .set_source_slot_active(1, false)
        .expect("source slot mutation");
    assert_eq!(second.state_hash(), stepped_state_hash);
    assert_eq!(second.terrain_hash(), stepped_terrain_hash);
    assert_eq!(second.mission().state_hash(), stepped_mission_hash);
    first.reset();
    assert_eq!(first.state_hash(), state_hash);
    assert_eq!(first.terrain_hash(), terrain_hash);
    assert_eq!(first.mission().state_hash(), mission_hash);
}

#[test]
fn level_two_reset_state_matches_preserved_frame_zero_projection() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
    );
    let bytes = fs::read(root.join("level2/sequence-002285FF-00356038.bin"))
        .expect("preserved Level 2 regression state");
    let sequence = RegressionStateSequence::new(&bytes).expect("Level 2 regression sequence");
    let frame = sequence.frame(0).expect("Level 2 frame zero");
    let expected = frame.project_player_flight(frame.local_player_slot());
    assert_eq!(inspection.reset_state(), Some(&expected));
}

#[test]
fn level_two_reset_session_matches_all_nineteen_preserved_idle_frames() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
    );
    let bytes = fs::read(root.join("level2/sequence-002285FF-00356038.bin"))
        .expect("preserved Level 2 regression state");
    let sequence = RegressionStateSequence::new(&bytes).expect("Level 2 regression sequence");
    let player_slot = sequence
        .frame(0)
        .expect("Level 2 frame zero")
        .local_player_slot();
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    for target_frame in 1..sequence.frame_count() {
        let after = sequence
            .frame(target_frame)
            .expect("preserved Level 2 frame");
        let decoded = after.decoded_flight_input(player_slot);
        decoded.apply(&mut session.state);
        let profile = session.profile();
        let terrain = session.canonical_terrain().clone();
        let core = session.campaign_state_mut().core_mut();
        crate::step_entity_pass_with_context(
            &mut core.state,
            decoded.command,
            profile,
            &terrain,
            &mut core.context,
        );
        let mut expected = after.project_player_flight(player_slot);
        expected.tick = session.state.tick;
        assert_eq!(session.state, expected, "Level 2 frame {target_frame}");
    }
}

#[test]
fn level_two_shared_day_flight_uses_level2_profile() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mission_profile = inspection
        .mission_content_profile()
        .expect("qualified Level 2 mission profile");
    assert!(mission_profile.is_level2());
    assert_eq!(mission_profile, MissionContentProfile::LEVEL2);
    assert!(!MissionContentProfile::LEVEL1.is_level2());
    assert!(MissionContentProfile::from_metadata(2, 0, 0).is_err());
    assert!(MissionContentProfile::from_metadata(2, 1, 1).is_err());
    assert_eq!(inspection.metadata().external_level, 2);
    assert_eq!(inspection.metadata().internal_level, 1);
    assert_eq!(inspection.runtime().player_count, 2);
    assert_eq!(inspection.runtime().local_player_slot, 0);
    assert!(!inspection.players()[0].is_ai);
    assert!(inspection.players()[1].is_ai);
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 reset session");
    let regression_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../../upstream/magic-carpet-2-hd/remc2-regression-test/memimages/regressions",
    );
    let terrain_sequence = fs::read(regression_root.join("level2/sequence-002285FF-002DC4E0.bin"))
        .expect("preserved Level 2 terrain sequence");
    let frame_bytes = 0x70000;
    assert!(terrain_sequence.len() >= frame_bytes);
    let oracle_frame = &terrain_sequence[..frame_bytes];
    let oracle = LegacyTerrain::from_regression_sequence(&terrain_sequence, 0, false)
        .expect("Level 2 Day terrain oracle");
    let plane =
        |index: usize| &oracle_frame[index * LEGACY_MAP_CELLS..(index + 1) * LEGACY_MAP_CELLS];
    assert_eq!(
        session.canonical_terrain().planes().terrain_types.as_ref(),
        plane(0)
    );
    assert_eq!(
        session.canonical_terrain().planes().ground_heights.as_ref(),
        plane(1)
    );
    assert_eq!(
        session.canonical_terrain().planes().shading.as_ref(),
        plane(2)
    );
    assert_eq!(
        session.canonical_terrain().planes().angles.as_ref(),
        plane(3)
    );
    // Day does not query the cave ceiling. The authored six-plane reset
    // preserves its secondary-height plane, but that plane is not part of
    // the Day frame-0 comparison (the preserved sequence's plane 4 is not
    // the authored package's secondary-height encoding).
    let expected_entity_indices: Vec<i16> = oracle_frame[5 * LEGACY_MAP_CELLS..]
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    assert_eq!(
        session.canonical_terrain().planes().entity_indices.as_ref(),
        expected_entity_indices.as_slice()
    );
    assert_eq!(inspection.metadata().map_type, MapType::Day);
    assert_eq!(session.profile(), FlightProfile::OUTDOOR);
    assert_eq!(
        session.canonical_terrain().planes().terrain_types.len(),
        256 * 256
    );
    assert_eq!(
        session.canonical_terrain().planes().ground_heights.len(),
        256 * 256
    );
    assert_eq!(
        session.canonical_terrain().planes().shading.len(),
        256 * 256
    );
    assert_eq!(session.canonical_terrain().planes().angles.len(), 256 * 256);
    assert_eq!(
        session.canonical_terrain().planes().secondary_heights.len(),
        256 * 256
    );
    assert_eq!(
        session.canonical_terrain().planes().entity_indices.len(),
        256 * 256
    );
    assert_eq!(
        session
            .canonical_terrain()
            .ceiling_height(session.state.position),
        None
    );

    // Day uses the same shared terrain queries as Level 1; compare the
    // immutable package terrain against the mutable session projection at
    // non-trivial wrapped map points.
    for position in [
        Position {
            x: 128,
            y: 32_896,
            z: 0,
        },
        Position {
            x: 38_912,
            y: 26_880,
            z: 0,
        },
        Position {
            x: 65_408,
            y: 26_880,
            z: 0,
        },
    ] {
        assert_eq!(
            session.canonical_terrain().ground_height(position),
            oracle.ground_height(position)
        );
        assert_eq!(
            session.canonical_terrain().terrain_type(position),
            oracle.terrain_type(position)
        );
        assert_eq!(
            session.canonical_terrain().material_mask(position),
            oracle.material_mask(position)
        );
    }
    assert_eq!(
        session.canonical_terrain().terrain_type(Position {
            x: 128,
            y: 32_896,
            z: 0
        }),
        69
    );
    assert_eq!(
        session.canonical_terrain().planes().ground_heights[0 + 128 * 256],
        2
    );

    let mut camera = TerrainSlopeCamera::default();
    let mut oracle_camera = TerrainSlopeCamera::default();
    assert_eq!(
        camera.update(session.canonical_terrain().ground_heights(), 0x0080, 0x8080),
        oracle_camera.update(oracle.ground_heights(), 0x0080, 0x8080)
    );
    assert_eq!(
        camera.update(session.canonical_terrain().ground_heights(), 0x0080, 0x8080),
        oracle_camera.update(oracle.ground_heights(), 0x0080, 0x8080)
    );

    // The Level 2 local wizard uses the same outdoor input/flight pass;
    // non-zero forward input is observable in the authoritative state.
    let before = session.state;
    let baseline_campaign_hash = session.campaign_state_hash();
    let evidence = session.step_outer_turn(
        PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    assert_eq!(evidence.len(), 1);
    assert_eq!(session.state.target_speed, 16);
    assert_eq!(session.state.speed_changed_flag, 1);
    assert_eq!(session.state.tick, before.tick + 1);
    assert_ne!(session.campaign_state_hash(), baseline_campaign_hash);
    let stepped_state = session.state;
    let stepped_campaign_hash = session.campaign_state_hash();

    let mut fresh = inspection
        .level2_reset_session()
        .expect("independent Level 2 reset session");
    fresh.step_outer_turn(
        PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    assert_eq!(fresh.state, stepped_state);
    assert_eq!(fresh.campaign_state_hash(), stepped_campaign_hash);

    session.reset();
    assert_eq!(session.state, before);
    assert_eq!(session.campaign_state_hash(), baseline_campaign_hash);
    session.step_outer_turn(
        PlayerCommand {
            flags: 1,
            ..PlayerCommand::default()
        },
        0,
    );
    assert_eq!(session.state, stepped_state);
    assert_eq!(session.campaign_state_hash(), stepped_campaign_hash);
}

#[test]
fn level_two_player_input_publishes_source_pitch_and_respects_body_skip() {
    let mut session = level_two_campaign_session();
    let player = usize::from(session.state.entity_index);
    session.step_outer_turn(
        PlayerCommand {
            pitch: 64,
            ..PlayerCommand::default()
        },
        0,
    );
    assert_ne!(session.state.display_pitch, 0);
    assert_eq!(session.entities()[player].pitch, session.state.display_pitch);
    assert_eq!(
        session.shared_spells().player_entity().pitch,
        session.state.display_pitch
    );

    // sub_5D530 returns before writing source pitch when its body-skip
    // latch is set. A different display value must not leak into the row.
    let row = &mut session.session.state.live.economy.entities[player];
    row.set_pitch(77);
    row.set_control_flags(row.control_flags | 0x800);
    session.state.display_pitch = 222;
    session.state.pitch_accumulator = 222;
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(session.entities()[player].pitch, 77);
    assert_eq!(session.shared_spells().player_entity().pitch, 77);
    assert_eq!(session.entities()[player].control_flags & 0x800, 0);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert_ne!(session.state.display_pitch, 77);
    assert_eq!(session.entities()[player].pitch, session.state.display_pitch);
    assert_eq!(
        session.shared_spells().player_entity().pitch,
        session.state.display_pitch
    );
}

#[test]
fn level_two_ordinary_reset_binds_census_shared_lifecycle_and_isolation() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 ordinary reset session");
    let second = inspection
        .level2_reset_session()
        .expect("independent Level 2 ordinary reset session");
    assert_eq!(first.ordinary().houses().len(), 22);
    assert_eq!(first.ordinary().villagers().len(), 29);
    assert_eq!(first.ordinary().dolmens().len(), 3);
    assert_eq!(first.ordinary().population_census(), 29);
    assert_eq!(
        first
            .ordinary()
            .villagers()
            .iter()
            .map(|villager| villager.entity_index)
            .collect::<Vec<_>>(),
        LEVEL_TWO_VILLAGER_INDICES
    );
    let baseline_hash = first.ordinary().reset_hash();
    assert_eq!(baseline_hash, second.ordinary().reset_hash());

    let house_phase = first.ordinary().houses()[0].phase;
    assert!(first.ordinary_mut().step_house_action_52(1).is_empty());
    assert_eq!(
        first.ordinary().houses()[0].phase,
        house_phase.wrapping_add(1)
    );
    first
        .ordinary_mut()
        .inject_house_damage(1, 1, 111)
        .expect("house damage mailbox");
    assert!(
        first
            .ordinary_mut()
            .step_house_action_52(1)
            .iter()
            .any(|event| matches!(
                event,
                HouseStepEvent::Damaged {
                    entity_index: 1,
                    ..
                }
            ))
    );

    let villager_phase = first.ordinary().villagers()[0].phase;
    assert!(first.ordinary_mut().step_villager_action_105(77).is_empty());
    assert_eq!(
        first.ordinary().villagers()[0].phase,
        villager_phase.wrapping_add(1)
    );
    first
        .ordinary_mut()
        .inject_villager_damage(77, 2_000, 111)
        .expect("villager damage mailbox");
    first.ordinary_mut().step_villager_action_105(77);
    assert_eq!(first.ordinary().villagers()[0].action, 108);
    first.ordinary_mut().step_villager_action_105(77);
    assert!(!first.ordinary().villagers()[0].active);
    assert_ne!(
        first.ordinary().villagers()[0].control_flags & 0x0000_0400,
        0
    );
    // Combat death is not the movement-exhaustion branch that decrements
    // the source class-5 census; the counter remains at reset value.
    assert_eq!(first.ordinary().population_census(), 29);

    // `move_villager` emits this exact effect after all four headings are
    // blocked. Feed that source-defined effect through the production
    // Level 2 sink and verify hash/reset isolation.
    let ordinary_hash = first.ordinary().reset_hash();
    first
        .ordinary
        .apply_villager_effects(&[VillagerStepEffect::PopulationCensusDecrement {
            source_entity_index: 77,
        }]);
    assert_eq!(first.ordinary().population_census(), 28);
    assert_ne!(first.ordinary().reset_hash(), ordinary_hash);

    let dolmen_before = first.ordinary().dolmens()[0].entity().clone();
    let baseline_flags = first.ordinary().player_interaction_flags();
    first.ordinary_mut().players[0].position = dolmen_before.position;
    let step = first
        .ordinary_mut()
        .step_dolmen_action_6(108)
        .expect("dolmen action 6");
    assert_eq!(step.previous_position, dolmen_before.position);
    assert_eq!(step.previous_control_flags, step.control_flags);
    assert_eq!(step.previous_phase, step.phase);
    assert_eq!(
        first.ordinary().dolmens()[0].entity().control_flags,
        dolmen_before.control_flags
    );
    assert_ne!(first.ordinary().player_interaction_flags(), baseline_flags);
    first.ordinary_mut().reset();
    assert_eq!(first.ordinary().reset_hash(), baseline_hash);
    assert_eq!(first.ordinary().population_census(), 29);
    assert_eq!(first.ordinary().player_interaction_flags(), baseline_flags);
    let dolmen_position = first.ordinary().dolmens()[0].entity().position;
    first.ordinary_mut().players[0].position = Position {
        x: dolmen_position.x.wrapping_add(0x4000),
        y: dolmen_position.y,
        z: dolmen_position.z,
    };
    let far_flags = first.ordinary().player_interaction_flags();
    first
        .ordinary_mut()
        .step_dolmen_action_6(108)
        .expect("far dolmen action 6");
    assert_eq!(first.ordinary().player_interaction_flags(), far_flags);
    first.ordinary_mut().reset();
    first.ordinary_mut().players[0].position = dolmen_position;
    first.ordinary_mut().players[0].life = -1;
    let dead_player_flags = first.ordinary().player_interaction_flags();
    first
        .ordinary_mut()
        .step_dolmen_action_6(108)
        .expect("dead-player dolmen action 6");
    assert_eq!(
        first.ordinary().player_interaction_flags(),
        dead_player_flags
    );
    assert_eq!(second.ordinary().reset_hash(), baseline_hash);
}

#[test]
fn level_two_goat_pool_is_production_wired_and_reset_isolated() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut first = inspection
        .level2_reset_session()
        .expect("Level 2 Goat production session");
    let mut second = inspection
        .level2_reset_session()
        .expect("isolated Level 2 Goat production session");
    assert_eq!(first.goats().len(), LEVEL_TWO_GOAT_INDICES.len());
    for (source_slot, expected_action) in [(39_u16, 11_u8), (71, 11)] {
        let goat = first
            .goats()
            .iter()
            .find(|goat| goat.entity_index == source_slot)
            .expect("authored Level 2 Goat row");
        assert_eq!(goat.action, expected_action);
        assert_eq!(goat.partner_entity_index, Some(66));
    }
    let baseline_hash = first.goat_hash();
    let baseline_rows = first.goats().to_vec();
    assert_eq!(first.goat_outer_tick(), 0);
    let baseline_phase = first
        .goats()
        .iter()
        .filter(|goat| matches!(goat.entity_index, 39 | 71))
        .map(|goat| (goat.entity_index, goat.phase))
        .collect::<BTreeMap<_, _>>();

    first.step_outer_turn(PlayerCommand::default(), 0);
    second.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(first.goat_hash(), second.goat_hash());
    assert_ne!(first.goat_hash(), baseline_hash);
    for (source_slot, previous_phase) in baseline_phase {
        let phase = first
            .goats()
            .iter()
            .find(|goat| goat.entity_index == source_slot)
            .expect("stepped Level 2 Goat row")
            .phase;
        assert_eq!(phase, previous_phase.wrapping_add(1));
    }

    let goat_slots = first
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::Goat { source_slot, .. } => Some(*source_slot),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(goat_slots.windows(2).all(|window| window[0] <= window[1]));
    assert!(goat_slots.contains(&39));
    assert!(goat_slots.contains(&71));
    let all_slots = first
        .dispatch_events()
        .iter()
        .filter_map(|event| match event {
            Mc2DispatchEvent::SharedPickup { source_slot, .. }
            | Mc2DispatchEvent::SpellAcquisition { source_slot, .. }
            | Mc2DispatchEvent::House { source_slot, .. }
            | Mc2DispatchEvent::Villager { source_slot, .. }
            | Mc2DispatchEvent::Castle { source_slot, .. }
            | Mc2DispatchEvent::Error { source_slot, .. }
            | Mc2DispatchEvent::DynamicArena { source_slot, .. }
            | Mc2DispatchEvent::Economy { source_slot, .. }
            | Mc2DispatchEvent::Goat { source_slot, .. }
            | Mc2DispatchEvent::LevelExitSequenceStarted {
                trigger_entity_index: source_slot,
                ..
            }
            | Mc2DispatchEvent::LevelExitSequenceCompleted {
                player_entity_index: source_slot,
                ..
            } => Some(*source_slot),
            // This is constructor metadata emitted at the producer
            // boundary; the live source rows below carry cursor order.
            Mc2DispatchEvent::AuthoredGroupActivated { .. } => return None,
            Mc2DispatchEvent::BadStone { step, .. } => Some(step.source_slot),
            Mc2DispatchEvent::Dolmen { step, .. } => Some(step.source_slot),
        })
        .collect::<Vec<_>>();
    // A same-pass source callback may reclaim and reuse a lower slot; the
    // cursor's initial visit remains ascending, while later generations
    // can legitimately repeat that source index.
    let mut seen = BTreeSet::new();
    let first_slots = all_slots
        .iter()
        .copied()
        .filter(|slot| seen.insert(*slot))
        .collect::<Vec<_>>();
    assert!(first_slots.windows(2).all(|window| window[0] < window[1]));

    // Capture the complete 36-row production result.  Hash equality alone
    // could miss a compensating row swap, so replay compares every typed
    // Goat field as well as the source tick and composite family hash.
    let stepped_rows = first.goats().to_vec();
    let stepped_hash = first.goat_hash();
    let stepped_tick = first.goat_outer_tick();
    assert_eq!(stepped_rows, second.goats().to_vec());
    assert_eq!(stepped_hash, second.goat_hash());
    assert_eq!(stepped_tick, second.goat_outer_tick());
    assert_eq!(stepped_tick, 1);

    first.reset();
    second.reset();
    assert_eq!(first.goats(), baseline_rows.as_slice());
    assert_eq!(second.goats(), baseline_rows.as_slice());
    assert_eq!(first.goat_hash(), baseline_hash);
    assert_eq!(second.goat_hash(), baseline_hash);
    assert_eq!(first.goat_outer_tick(), 0);
    assert_eq!(second.goat_outer_tick(), 0);
    assert!(first.dispatch_events().is_empty());

    // An identical idle step after reset must reproduce the original
    // rows, hash, and tick—not merely the same event count.
    first.step_outer_turn(PlayerCommand::default(), 0);
    second.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(first.goats(), stepped_rows.as_slice());
    assert_eq!(second.goats(), stepped_rows.as_slice());
    assert_eq!(first.goat_hash(), stepped_hash);
    assert_eq!(second.goat_hash(), stepped_hash);
    assert_eq!(first.goat_outer_tick(), stepped_tick);
    assert_eq!(second.goat_outer_tick(), stepped_tick);
}

#[test]
fn level_two_death_burst_dynamic_row_dispatches_reclaims_and_resets() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut session = inspection
        .level2_reset_session()
        .expect("Level 2 death-burst session");
    let baseline_hash = session.economy().reset_hash();
    let burst_slot = session
        .economy_mut()
        .spawn_death_burst_from_death(901, Position::default())
        .expect("death-burst allocation");
    assert!(
        session
            .economy()
            .live_arena_slots()
            .contains(&(burst_slot, Level2DynamicArenaKind::DeathBurst))
    );
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&burst_slot)
    );

    for expected_life in [0, -1] {
        session.step_outer_turn(PlayerCommand::default(), 0);
        let burst = session
            .economy()
            .death_bursts()
            .iter()
            .find(|burst| burst.source_entity_index == burst_slot)
            .expect("the scheduled burst remains allocated");
        assert!(burst.active);
        assert_eq!(burst.life_ticks, expected_life);
        assert!(!session.economy().free_slots.contains(&burst_slot));
        assert!(session.economy().map_chains().is_linked(burst_slot));
        assert!(session.dispatch_events().iter().any(|event| matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::DeathBurst,
                ..
            } if *source_slot == burst_slot
        )));
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .economy()
            .death_bursts()
            .iter()
            .any(|burst| burst.source_entity_index == burst_slot
                && burst.active
                && burst.control_flags & 0x400 != 0)
    );
    assert!(!session.economy().free_slots.contains(&burst_slot));
    assert!(
        session
            .economy()
            .map_chains()
            .linked_entity_indices()
            .contains(&burst_slot)
    );
    assert_eq!(
        session
            .dispatch_events()
            .iter()
            .filter(|event| matches!(
                event,
                Mc2DispatchEvent::DynamicArena {
                    source_slot,
                    kind: Level2DynamicArenaKind::DeathBurst,
                    ..
                } if *source_slot == burst_slot
            ))
            .count(),
        1,
        "source model-1 burst is visited once on its terminal pass"
    );
    assert!(session.dispatch_events().iter().any(|event| {
        matches!(
            event,
            Mc2DispatchEvent::DynamicArena {
                source_slot,
                kind: Level2DynamicArenaKind::DeathBurst,
                ..
            } if *source_slot == burst_slot
        )
    }));
    assert!(session.economy().events().iter().any(|event| {
        matches!(
            event,
            Level2EconomyEvent::DeathBurstExpired { source_entity_index, .. }
                if *source_entity_index == burst_slot
        )
    }));
    assert_ne!(session.economy().reset_hash(), baseline_hash);

    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(session.economy().death_bursts().is_empty());
    assert!(session.economy().free_slots.contains(&burst_slot));
    assert!(!session.economy().map_chains().is_linked(burst_slot));
    session.reset();
    assert_eq!(session.economy().reset_hash(), baseline_hash);
}

#[test]
fn level_two_ordinary_census_rejects_wrong_rows_hashes_and_presentation() {
    let inspection =
        NativeWorld::inspect_level2(level_two_fixture_path()).expect("qualified Level 2 package");
    let mut entities = inspection.entities().to_vec();
    let presentation = inspection.presentation().clone();
    entities[77].action = 106;
    assert!(validate_level2_ordinary_census(&entities, &presentation).is_err());
    entities = inspection.entities().to_vec();
    entities[1].record[74] ^= 1;
    assert!(validate_level2_ordinary_census(&entities, &presentation).is_err());
    entities = inspection.entities().to_vec();
    entities[108].parameter_relation = Some(60);
    assert!(Level2Dolmen::from_entity(&entities[108], inspection.terrain_planes()).is_err());
    entities = inspection.entities().to_vec();
    entities[1].model = 44;
    assert!(HouseState::from_level_two_snapshot(&entities[1]).is_err());
    entities = inspection.entities().to_vec();
    entities[88] = entities[77].clone();
    assert!(
        VillagerFamily::from_level_two_entities(&entities, VillagerBehaviorConfig::LEVEL_TWO)
            .is_err()
    );
    let mut wrong_presentation = presentation;
    wrong_presentation.level_sprite_selectors[79] = 1;
    assert!(validate_level2_ordinary_census(&inspection.entities, &wrong_presentation).is_err());
    let mut wrong_house_presentation = inspection.presentation().clone();
    wrong_house_presentation.level_sprite_selectors[96] = 0;
    assert!(
        validate_level2_ordinary_census(&inspection.entities, &wrong_house_presentation).is_err()
    );
    let mut left = inspection.entities()[108].clone();
    let mut right = inspection.entities()[111].clone();
    left.position.x = u16::MAX - 5;
    right.position.y = left.position.y;
    right.position.z = left.position.z;
    right.position.x = 2;
    for entity in [&mut left, &mut right] {
        // sub_10630: source yaw is vertical offset, pitch/roll are X/Y
        // half widths, and fov is the vertical half height.
        for (offset, value) in [(86, 0_i16), (88, 16), (90, 16), (92, 16)] {
            entity.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    assert!(level2_source_intersects(&left, &right));
    right.position.x = left.position.x.wrapping_add(32);
    assert!(!level2_source_intersects(&left, &right), "exact X edge is outside");
    right.position.x = left.position.x.wrapping_add(31);
    assert!(level2_source_intersects(&left, &right), "wrapped X just inside");
    right.position.y = left.position.y.wrapping_add(32);
    assert!(!level2_source_intersects(&left, &right), "exact Y edge is outside");
    right.position.y = left.position.y;
    left.position.z = 100;
    right.position.z = 420;
    left.record[86..88].copy_from_slice(&120_i16.to_le_bytes());
    right.record[86..88].copy_from_slice(&(-200_i16).to_le_bytes());
    assert!(level2_source_intersects(&left, &right), "source yaw aligns vertical centres");
    right.position.z += 32;
    assert!(!level2_source_intersects(&left, &right));
}

#[test]
fn level_two_profile_rejects_cross_level_relabeling_and_level_one_loader() {
    let package = level_two_fixture_path();
    let root = std::env::temp_dir().join(format!(
        "mc2-level2-profile-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::copy(package.join("manifest.json"), &manifest_path).unwrap();
    for (name, _) in LEVEL2_V5_PACKAGE_MEMBERS {
        fs::copy(package.join(name), root.join(name)).unwrap();
    }
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["level"]["external"] = json!(1);
    manifest["level"]["internal"] = json!(0);
    manifest["level"]["map_type"] = json!({"id": 1, "name": "Night"});
    let mut identity = Vec::new();
    for (name, expected_size) in LEVEL2_V5_PACKAGE_MEMBERS {
        let bytes = fs::read(root.join(name)).unwrap();
        assert_eq!(bytes.len(), expected_size);
        let hash = sha256_hex(&bytes);
        identity.extend_from_slice(name.as_bytes());
        identity.push(0);
        identity.extend_from_slice(hash.as_bytes());
        identity.push(0);
        identity.extend_from_slice(expected_size.to_string().as_bytes());
        identity.push(b'\n');
    }
    manifest["package_sha256"] = json!(sha256_hex(&identity));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(NativeWorld::inspect_level2(&root).is_err());
    let (_, closure) = fixture_paths();
    assert!(NativeWorld::load(&root, closure).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn package_m_loads_the_exact_native_world() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load_without_owner_cache(package, closure)
        .expect("validated cache-free package M");
    assert_eq!(world.metadata.map_type, MapType::Night);
    assert_eq!(world.profile, FlightProfile::OUTDOOR);
    assert_eq!(
        world.terrain.ceiling_height(world.reset_state.position),
        None
    );
    assert_eq!(world.entities.len(), 1_000);
    assert_eq!(
        world.entities.iter().filter(|entity| entity.active).count(),
        177
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| {
                entity.active
                    && entity.control_flags & crate::level1_mission::SOURCE_REPLACEABLE_FLAG != 0
            })
            .count(),
        31
    );
    assert_eq!(world.players.len(), 8);
    assert_eq!(world.asset_refs.len(), 76);
    assert_eq!(world.runtime.objective_presentation_phase, 1);
    assert_eq!(world.runtime.objective_textbox_ticks, 0);
    assert_eq!(world.runtime.objectives.len(), 8);
    assert_eq!(
        world.runtime.objectives[0],
        RuntimeObjective {
            is_level_end: 0,
            text_selector: 0,
            done: 0,
            stage_selectors: [1, 1, 1, 1, 1, 0, 0, 0],
        }
    );
    assert_eq!(
        world.package_sha256(),
        "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
    );
    assert_eq!(
        world.closure_sha256(),
        "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719"
    );
    assert_eq!(state_hash(world.reset_state()), 0x9da4_bc91_af58_f3ce);
    assert_eq!(
        conformance_hash(world.reset_state(), world.reset_context()),
        0x77b9_6b82_492d_6f4b
    );
    assert_eq!(
        world.reset_state.position,
        Position {
            x: 19_840,
            y: 56_960,
            z: 5_024,
        }
    );
    assert_eq!(world.reset_state.entity_phase, 153);
    assert_eq!(world.reset_state.ambient_loop_5_distance, 2_048);
    assert_eq!(world.reset_state.ambient_loop_31_distance, 1_746);
    assert_eq!(
        world.presentation.particles[usize::from(MANA_CARRIER_RENDER_INDEX)],
        ParticleParameter {
            base_tmap: MANA_CARRIER_BASE_TMAP,
            word_2: 8_492,
            word_4: 42,
            speed: 545,
            rotation_speed: MANA_CARRIER_LOGICAL_HEIGHT,
            byte_10: 0,
            byte_11: -16,
            render_mode: MANA_CARRIER_RENDER_MODE,
            byte_13: 0,
        }
    );
    let (castle_blockers, building_blockers) = world.level1_mission.castle_placement_blockers();
    assert!(castle_blockers.is_empty());
    assert_eq!(building_blockers.len(), 47);
    let expected_buildings = world
        .entities
        .iter()
        .filter(|entity| entity.active && entity.class == 10 && entity.model == 45);
    for (blocker, entity) in building_blockers.iter().zip(expected_buildings) {
        assert_eq!(*blocker, entity.castle_placement_blocker());
    }
    let possession_targets = world.level1_mission.possession_targets();
    assert_eq!(possession_targets.len(), 41);
    assert!(possession_targets.iter().all(|target| {
        let entity = &world.entities[usize::from(target.entity_index)];
        !matches!(entity.level_gate, 17 | 51 | 52 | 53) && *target == entity.collision_target()
    }));

    let mut first_session = world.session();
    let second_session = world.session();
    let reset_lighting_hash = first_session.mission.dynamic_lighting_hash_v1();
    assert_eq!(
        (
            first_session.mission.dynamic_light_active_count(),
            second_session.mission.dynamic_light_active_count(),
            reset_lighting_hash,
        ),
        (0, 0, second_session.mission.dynamic_lighting_hash_v1()),
        "each production session starts from the same complete 50-slot lighting domain"
    );
    first_session
        .mission
        .spawn_death_burst_from_death(
            first_session.state.entity_index,
            first_session.state.position,
        )
        .expect("production death-burst light admission");
    assert_eq!(first_session.mission.dynamic_light_active_count(), 1);
    assert_eq!(
        (
            first_session.mission.dynamic_lights()[0].intensity,
            first_session.mission.dynamic_lights()[0].jitter_range,
            first_session.mission.dynamic_lights()[0].life_scaled,
        ),
        (
            crate::level1_mission::PLAYER_DEATH_BURST_DYNAMIC_LIGHT_RADIUS,
            crate::level1_mission::PLAYER_DEATH_BURST_DYNAMIC_LIGHT_KIND,
            true,
        )
    );
    assert_ne!(
        first_session.mission.dynamic_lighting_hash_v1(),
        reset_lighting_hash
    );
    assert_eq!(
        second_session.mission.dynamic_lighting_hash_v1(),
        reset_lighting_hash,
        "one production session cannot mutate another session's lighting RNG/history"
    );
    first_session.step(PlayerCommand::default());
    assert_eq!(
        first_session.mission.dynamic_light_active_count(),
        0,
        "production dispatch expires the one-tick burst before the post-update light service"
    );
    assert_eq!(
        first_session.mission.dynamic_lights()[0].history,
        [0; crate::dynamic_lighting::DYNAMIC_LIGHT_HISTORY_CELLS],
        "the production subtract/update/service schedule retains no stale one-tick burst footprint"
    );
    first_session.reset();
    assert_eq!(
        (
            first_session.mission.dynamic_light_active_count(),
            first_session.mission.dynamic_lighting_hash_v1(),
        ),
        (0, reset_lighting_hash),
        "production reset restores RNG, count, inactive payloads, and retained histories"
    );

    let scheduled = [0_u8, 1, 2].map(|speed_index| {
        let mut session = world.session();
        assert_eq!(
            session.mission.admit_linked_dynamic_light_for_test(
                session.state.entity_index,
                session.state.position,
            ),
            Some(0)
        );
        let evidence = session.step_with_speed_index(
            PlayerCommand {
                flags: 1,
                ..PlayerCommand::default()
            },
            speed_index,
        );
        assert_eq!(
            session.mission.dynamic_lighting_rng_for_test(),
            0x84ca_7cd2,
            "one live light consumes exactly one RNG draw after the final source pass"
        );
        assert_eq!(
            session.mission.dynamic_lights()[0].position,
            session.state.position,
            "light service must observe the final player position after every full pass"
        );
        (
            evidence.len(),
            session.state.position,
            session.state.target_speed,
            session.state.actual_speed,
            session.state.tick,
            session.mission.source_turn_counter(),
        )
    });
    assert_eq!(
        scheduled,
        [
            (
                1,
                Position {
                    x: 19_840,
                    y: 56_944,
                    z: 5_024,
                },
                16,
                16,
                1,
                2,
            ),
            (
                4,
                Position {
                    x: 19_840,
                    y: 56_800,
                    z: 5_028,
                },
                64,
                64,
                4,
                5,
            ),
            (
                8,
                Position {
                    x: 19_840,
                    y: 56_480,
                    z: 5_040,
                },
                80,
                80,
                8,
                9,
            ),
        ],
        "production 1/4/8 full source sweeps must preserve exact final state and position"
    );

    let mut threshold_world = world.clone();
    let threshold_goat_index = usize::from(LEVEL_ONE_GOAT_INDICES[0]);
    let initial_player_position = threshold_world.reset_state.position;
    let threshold_goat_position = Position {
        x: initial_player_position.x,
        y: initial_player_position.y.wrapping_sub(6_144),
        z: initial_player_position.z,
    };
    let mut threshold_goat = threshold_world.entities[threshold_goat_index]
        .clone()
        .with_record_byte(0x003d, 0)
        .with_record_byte(0x003e, 0);
    threshold_goat.position = threshold_goat_position;
    threshold_goat.action = 0;
    threshold_goat.control_flags &= !1;
    threshold_world.entities[threshold_goat_index] = threshold_goat;
    threshold_world.reset_live_world = LiveWorldState::from_entities(
        &threshold_world.entities,
        threshold_world.goat_stage_point,
        threshold_world.goat_config,
    )
    .expect("threshold-crossing production Goat family");
    assert_eq!(
        i32::from(initial_player_position.y)
            .wrapping_sub(i32::from(threshold_goat_position.y))
            .pow(2),
        0x0240_0000,
        "the outer-turn starting player position is exactly outside the strict Goat threshold"
    );
    assert!(
        i32::from(56_944_u16)
            .wrapping_sub(i32::from(threshold_goat_position.y))
            .pow(2)
            < 0x0240_0000,
        "pass-one movement crosses into the strict Goat threshold"
    );
    let threshold_crossings = [1_u8, 2].map(|speed_index| {
        let mut threshold_session = threshold_world.session();
        threshold_session.step_with_speed_index(
            PlayerCommand {
                flags: 1,
                ..PlayerCommand::default()
            },
            speed_index,
        );
        let threshold_goat = threshold_session
            .live_world
            .goats()
            .iter()
            .find(|goat| goat.entity_index == LEVEL_ONE_GOAT_INDICES[0])
            .expect("threshold Goat remains live");
        (
            threshold_session.state.position,
            threshold_goat.position,
            threshold_goat.proximity_countdown,
            threshold_goat.proximity_subcounter,
        )
    });
    assert_eq!(
        threshold_crossings,
        [
            (
                Position {
                    x: 19_840,
                    y: 56_800,
                    z: 5_028,
                },
                threshold_goat_position,
                14,
                0,
            ),
            (
                Position {
                    x: 19_840,
                    y: 56_480,
                    z: 5_040,
                },
                threshold_goat_position,
                10,
                0,
            ),
        ],
        "speed-index 1/2 later passes must observe pass-one player movement at Goat begin-turn proximity"
    );
}

#[test]
fn qualified_owner_cache_adds_descriptor_backed_audio_and_goat_tmap_states() {
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut refs = Vec::new();
    let cache = append_level_one_cache_assets(
        &private.join("generated/level1-unity-asset-cache"),
        "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
        "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
        &mut refs,
    )
    .expect("qualified owner audio cache");
    let sound = cache.sound_asset_refs;
    let music = cache.music_asset_ref;
    let goat_tmaps = cache.goat_tmap_bank;
    let castle = cache
        .castle_stage_one_footprint
        .expect("qualified Castle BUILD footprint");
    assert_eq!((castle.index, castle.width, castle.height), (1, 8, 8));
    assert_eq!(castle.cells().len(), 64);
    assert_eq!(refs.len(), 33);
    assert_eq!(
        sound.keys().copied().collect::<Vec<_>>(),
        [
            1, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31, 33, 34, 37, 39, 40,
            41, 43, 44, 46, 54, 55, 56, 57, 63
        ]
    );
    assert_eq!(sound[&12], 8);
    assert_eq!(
        refs[8].asset_id,
        "owner_cache:audio/sound-bank-1-822.json#sound_id=12"
    );
    assert_eq!(music, Some(32));
    assert_eq!(
        refs[32].content_address,
        "sha256:03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"
    );
    let goat_tmaps = goat_tmaps.expect("qualified Goat TMAP bank");
    assert_eq!(
        std::array::from_fn(|ordinal| goat_tmaps.selectors()[ordinal].selector),
        GOAT_TMAP_SELECTORS
    );
    assert_eq!(
        std::array::from_fn(|ordinal| { goat_tmaps.selectors()[0].states[ordinal].stream_cursor }),
        [490, 818, 1146, 1472, 1812]
    );
    assert_eq!(goat_tmaps.selectors()[1].states[0].pixels.len(), 32 * 22);
}

#[test]
fn qualified_goat_tmap_clocks_materialize_every_exact_pixel_state_and_replay() {
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let cache = append_level_one_cache_assets(
        &private.join("generated/level1-unity-asset-cache"),
        "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8",
        "878123908f6de05f684f4cd1d73cff8d2ff08701cb6fdd12b7165bfcacc75719",
        &mut Vec::new(),
    )
    .expect("qualified owner cache");
    let bank = cache.goat_tmap_bank.expect("qualified Goat TMAP bank");
    let expected_hashes = [
        [
            "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
            "502c7e18c327f09578c7ebcd31c351cd2c537bae9482c3f7630a7035be130190",
            "b22359a1aebc1f7afea7e5aa7a4e8c5bdfc5b238ca9a7f162e110470b3350103",
            "2011b3fb136f072ca7dcdae7199b2e8b9dbbdb71bdaa812b1bd659b695c51d12",
            "cd5e773ab7c4e4e88536c5a690a8a1794a1b67361b402ab8365e9df3e59a1e9d",
        ],
        [
            "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
            "4068c077eb4f66e39ae3bd13202462b477a418aa7781822d09f0feb3337765c7",
            "d5d9ccc838119171cb7ac06315e6e6994e72c058cf40b3b9a9b6fb6709e8931f",
            "f0f656d86c51628b5f0aac90932b79b448f64ef1ef38b41fcffc9d275df71fb1",
            "acea2351981a1fd9aefaf7a2713bc00c4c2b2461dd087b9a2621e7fa3fb819a3",
        ],
        [
            "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
            "897331a6a567a6f64be563755ba33a5ab1e75e0a1d77d5a1fc9a67ca8657d44a",
            "22d9b6b2bad1a7fef49afcb05284c513f14200852c5cbaad097f684a0197f940",
            "752525ceb8c9a097f0676def3bdeb58258bd093ce7efacf3fe170dc7ec4cbc84",
            "7d081998c98ba0aa2bb57d4956926f3b8c80b31ae6ee481f8c9174551d6db722",
        ],
    ];
    let expected_ordinals = [0_u8, 1, 2, 3, 4, 1];
    let mut clocks = GoatTmapClocks::from_bank(&bank);
    let reset_pixels: [Box<[u8]>; GOAT_TMAP_SELECTORS.len()] =
        std::array::from_fn(|ordinal| clocks.states()[ordinal].pixels().into());

    for (step, expected_ordinal) in expected_ordinals.into_iter().enumerate() {
        for (selector_ordinal, clock) in clocks.states().iter().enumerate() {
            assert_eq!(clock.state_ordinal(), expected_ordinal);
            assert_eq!(
                sha256_hex(clock.pixels()),
                expected_hashes[selector_ordinal][usize::from(expected_ordinal)]
            );
        }
        if expected_ordinal == 4 {
            for (selector_ordinal, clock) in clocks.states().iter().enumerate() {
                assert_eq!(clock.pixels(), reset_pixels[selector_ordinal].as_ref());
            }
        }
        if step + 1 < expected_ordinals.len() {
            clocks
                .latch_draw_attempts(0b111)
                .expect("three qualified selectors");
            clocks.service_unpaused_outer_turn(&bank);
        }
    }

    let expected_replay = clocks.clone();
    clocks.reset(&bank);
    for _ in 0..5 {
        clocks
            .latch_draw_attempts(0b111)
            .expect("three qualified selectors");
        clocks.service_unpaused_outer_turn(&bank);
    }
    assert_eq!(clocks, expected_replay);
    assert_eq!(clocks.hash_v1(), expected_replay.hash_v1());
}

#[test]
fn authority_is_narrow_and_explicit() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    assert_eq!(
        world.entities[152].authority,
        EntityAuthority::PlayerFlightLive
    );
    assert_eq!(
        world.entities[153].authority,
        EntityAuthority::SpellActivationLive
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.authority == EntityAuthority::PlayerFlightLive)
            .count(),
        1
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.authority == EntityAuthority::SpellActivationLive)
            .count(),
        1
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.authority == EntityAuthority::FrozenObserved)
            .count(),
        114
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.authority == EntityAuthority::GoatLive)
            .count(),
        LEVEL_ONE_GOAT_INDICES.len()
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.authority == EntityAuthority::VillagerLive)
            .count(),
        LEVEL_ONE_VILLAGER_INDICES.len()
    );
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.authority == EntityAuthority::Inactive)
            .count(),
        823
    );
    let session = world.session();
    let dispositions = session
        .source_sprite_dispositions()
        .expect("startup-valid live Villagers have typed source dispositions");
    for source_entity_index in LEVEL_ONE_VILLAGER_INDICES {
        assert!(
            dispositions.iter().any(|disposition| matches!(
                disposition,
                SourceSpriteDisposition::Legacy {
                    source_entity_index: index,
                    family: SourceSpriteLegacyFamily::Resident,
                } if *index == source_entity_index
            )),
            "live Villager slot {source_entity_index} must use its authority-aware resident renderer"
        );
    }
    for source_entity_index in LEVEL_ONE_GOAT_INDICES {
        assert!(
            dispositions.iter().any(|disposition| matches!(
                disposition,
                SourceSpriteDisposition::Legacy {
                    source_entity_index: index,
                    family: SourceSpriteLegacyFamily::Goat,
                } if *index == source_entity_index
            )),
            "live Goat slot {source_entity_index} must use its authority-aware Goat renderer"
        );
    }
}

#[test]
fn sessions_are_independent_and_reset_to_package_projection() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let mut first = world.session();
    let second = world.session();
    assert_eq!(
        conformance_hash(world.reset_state(), world.reset_context()),
        0x77b9_6b82_492d_6f4b,
        "the preserved package reset contract must not absorb presentation state"
    );
    assert_eq!(
        conformance_hash(&first.state, &first.context),
        0xe7ec_6bc1_8b5f_e775,
        "the additive complete Level 1 hand/audio session contract is independently pinned"
    );
    assert_eq!(
        first.context.effects,
        Some(FlightEffectsContext {
            local_player_index: 0,
            periodic_spell_effect: None,
            ..FlightEffectsContext::default()
        })
    );
    first.step(PlayerCommand {
        flags: 1,
        ..PlayerCommand::default()
    });
    assert_ne!(first.state, second.state);
    first.reset();
    assert_eq!(first.state, *world.reset_state());
    assert_eq!(first.context, world.session_context());
}

#[test]
fn level_one_package_excludes_the_family_four_periodic_sound_branch() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let player = &world.players[usize::from(world.runtime.local_player_slot)];
    assert_eq!(player.enabled_spell_holders[4], 0);
    assert_eq!(
        world
            .entities
            .iter()
            .filter(|entity| entity.active && entity.class == 15)
            .map(|entity| entity.model)
            .collect::<Vec<_>>(),
        [0, 1],
        "the canonical Level 1 package exposes only Fireball and Possession holders"
    );

    let mut session = world.session();
    assert_eq!(
        session
            .context
            .spell_fire
            .as_ref()
            .unwrap()
            .enabled_holder_indices[4],
        0
    );
    assert_eq!(
        session
            .context
            .effects
            .as_ref()
            .unwrap()
            .periodic_spell_effect,
        None
    );
    session.state.entity_phase = 0;
    session.step(PlayerCommand::default());
    assert!(session.context.events.iter().all(|event| !matches!(
        event,
        crate::FlightEvent::PrepareSound {
            audio_site: crate::MissionAudioSite::PeriodicSpellExcluded,
            ..
        }
    )));
}

#[test]
fn neutral_session_replay_matches_original_idle_source_projection() {
    let (package, closure) = fixture_paths();
    let world = NativeWorld::load(package, closure).expect("validated package M");
    let sequence =
        RegressionStateSequence::new(OWNED_LEVEL_ONE_STATE_SEQUENCE).expect("state sequence");
    let mut session = world.session();
    for frame_index in 0..sequence.frame_count() {
        if frame_index > 0 {
            session.step(PlayerCommand::default());
        }
        if matches!(frame_index, 0 | 8 | 19) {
            let frame = sequence.frame(frame_index).expect("captured frame");
            let original = frame.project_player_flight(frame.local_player_slot());
            assert_eq!(
                source_projection_hash(&session.state),
                state_hash(&original),
                "source projection at idle frame {frame_index}"
            );
        }
    }
}

#[test]
fn corrupt_package_hash_version_relation_and_bounds_fail_closed() {
    let (package, closure) = fixture_paths();
    for mutation in [
        Mutation::ManifestSchema,
        Mutation::EntityMagic,
        Mutation::ResetMapping,
    ] {
        let temp = TempPackage::copy_from(&package, &closure);
        temp.mutate(mutation);
        assert!(
            NativeWorld::load(&temp.package, &temp.closure).is_err(),
            "mutation {mutation:?} must fail"
        );
    }
}

#[test]
fn actual_level14_stage_variable_null_preserves_sentinel_and_pointer_bounds() {
    let package = std::env::var_os("MC_LEVEL14_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj/level14"),
        PathBuf::from,
    );
    let bytes = fs::read(package.join("runtime.bin")).expect("whole retained Level14 runtime");
    let offset = 392 + 5 * 12;
    assert_eq!(&bytes[offset..offset + 12], &[3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        "actual StageVar5 is an unresolved kind3 direct pointer");
    let mut runtime = parse_runtime(&bytes).expect("source-valid null pointer must materialize");
    assert_eq!(runtime.stage_variables[5].relation, RuntimeRelation::Null);
    assert_eq!(runtime.bytes(), bytes.as_slice(), "the raw save/hash carrier remains exact");
    let point_offset = 392 + 12;
    assert_eq!(runtime.stage_variables[1].relation, RuntimeRelation::AxisScalar(
        u32::from_le_bytes(bytes[point_offset + 8..point_offset + 12].try_into().unwrap())));

    for (value, slot) in [(1_u32, 0_u16), (1000, 999)] {
        let mut configured = bytes.clone();
        configured[offset + 8..offset + 12].copy_from_slice(&value.to_le_bytes());
        let decoded = parse_runtime(&configured).unwrap();
        assert_eq!(decoded.stage_variables[5].relation, RuntimeRelation::LiveEntity(slot));
        assert_eq!(decoded.bytes(), configured.as_slice());
    }
    let mut invalid = bytes.clone();
    invalid[offset + 8..offset + 12].copy_from_slice(&1001_u32.to_le_bytes());
    assert!(parse_runtime(&invalid).is_err(), "the pointer upper bound is unchanged");
    invalid[offset + 4] = 2;
    invalid[offset + 8..offset + 12].fill(0);
    assert!(parse_runtime(&invalid).is_err(), "unknown StageVar pointer domains still fail");

    // Configured subsequent sub12100 publication uses the canonical carrier;
    // this is not a claim that authored511 binds the unrelated StageVar5.
    assert!(!runtime.bind_authored_stage_variable_entity(5, 0));
    assert!(runtime.bind_authored_stage_variable_entity(5, 42));
    assert_eq!(runtime.stage_variables[5].relation, RuntimeRelation::LiveEntity(42));
    assert_eq!(&runtime.bytes()[offset + 8..offset + 12], &43_u32.to_le_bytes());
}

#[test]
fn corrupt_component_version_relation_and_bounds_reach_their_parser_gates() {
    let (package, _) = fixture_paths();

    let mut players = fs::read(package.join("players.bin")).unwrap();
    players[4] ^= 0xff;
    assert!(
        parse_players(
            &players,
            OwnerPackageProfile::Level1,
            OwnerPackageSchema::V2
        )
        .is_err()
    );

    let mut entities = fs::read(package.join("entities.bin")).unwrap();
    let relation_offset = 12 + 152 * ENTITY_RECORD_BYTES + 164;
    entities[relation_offset..relation_offset + 4].copy_from_slice(&999_u32.to_le_bytes());
    assert!(parse_entities(&entities).is_err());

    let mut players = fs::read(package.join("players.bin")).unwrap();
    players[16..18].copy_from_slice(&1_000_u16.to_le_bytes());
    assert!(
        parse_players(
            &players,
            OwnerPackageProfile::Level1,
            OwnerPackageSchema::V2
        )
        .is_err()
    );

    let mut runtime = fs::read(package.join("runtime.bin")).unwrap();
    let stage_relation_offset = 192 + 6;
    runtime[stage_relation_offset] = 9;
    assert!(parse_runtime(&runtime).is_err());

    let mut presentation = fs::read(package.join("presentation.bin")).unwrap();
    presentation[4] ^= 0xff;
    assert!(parse_presentation(&presentation).is_err());
}

#[test]
fn corrupt_closure_hash_binding_and_unresolved_frame_use_fail_closed() {
    let (package, closure) = fixture_paths();
    for mutation in [
        ClosureMutation::Hash,
        ClosureMutation::PackageBinding,
        ClosureMutation::Unresolved,
        ClosureMutation::AssetOrdinal,
    ] {
        let temp = TempPackage::copy_from(&package, &closure);
        temp.mutate_closure(mutation);
        assert!(
            NativeWorld::load(&temp.package, &temp.closure).is_err(),
            "closure mutation {mutation:?} must fail"
        );
    }
}

#[test]
fn v1_or_incomplete_reset_is_rejected_instead_of_synthesizing_ambient() {
    let (package, closure) = fixture_paths();
    let temp = TempPackage::copy_from(&package, &closure);
    let manifest_path = temp.package.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["schema"] = json!("mc2-owner-level-export/v1");
    manifest["reset_projection"]["field_mappings"]
        .as_object_mut()
        .unwrap()
        .remove("ambient_loop_31_distance");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(NativeWorld::load(&temp.package, &temp.closure).is_err());
}
