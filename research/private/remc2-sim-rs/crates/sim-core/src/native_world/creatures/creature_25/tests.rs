use super::*;

fn terrain() -> SessionTerrain {
    crate::native_world::tests::level_two_campaign_session()
        .canonical_terrain()
        .clone()
}

fn target(slot: u16, class: u8, model: u8, position: Position) -> Target {
    Target {
        slot,
        source_id: slot,
        class,
        model,
        life: 1000,
        position,
        yaw: Some(0),
        roll: Some(0),
        pitch_extent: Some(0),
        roll_extent: Some(0),
        attacker: Some(0),
        target: Some(0),
        stage_index: Some(0),
        stage_mode: Some(0),
        parent: Some(0),
        stage_relation: Some(0),
        next: Some(0),
        ..Target::default()
    }
}

fn cymmerian(action: u8, position: Position) -> EntitySnapshot {
    let mut row = crate::native_world::cave_features::new_event(7, 19, position);
    row.class = SOURCE_CLASS;
    row.model = SOURCE_MODEL;
    row.record[67] = SOURCE_CLASS;
    row.record[68] = SOURCE_MODEL;
    row.record[61] = 64;
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[164..168].copy_from_slice(&(u32::from(DESCRIPTOR) + 1).to_le_bytes());
    row.set_source_overlap_half_extents(RULES.collision_half_width, RULES.collision_half_width);
    row.record[92..94].copy_from_slice(&RULES.collision_half_height.to_le_bytes());
    row.set_source_action(action);
    row.set_max_life(RULES.max_life);
    row.set_life(RULES.max_life);
    row.set_actual_speed(0);
    row.set_phase(RULES.scan_period);
    row
}

fn mission_eighteen_root() -> std::path::PathBuf {
    std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    )
}

fn summoned_source_parts() -> (
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Vec<crate::native_world::ParticleParameter>,
    u16,
    u16,
    Position,
) {
    source_parts(18)
}

fn source_parts(
    mission: u8,
) -> (
    crate::campaign::source_arena::SourceArena,
    crate::native_world::RuntimeSnapshot,
    SessionTerrain,
    Vec<crate::native_world::ParticleParameter>,
    u16,
    u16,
    Position,
) {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join(format!("level{mission}")),
        crate::campaign::spec::CampaignSelection::ordinary(u16::from(mission)).unwrap(),
    )
    .expect("decode owner-data Mission 18 package");
    let entities = crate::native_world::parse_entities(package.member("entities.bin").unwrap())
        .unwrap()
        .into_boxed_slice();
    let runtime =
        crate::native_world::parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation =
        crate::native_world::parse_presentation(package.member("presentation.bin").unwrap())
            .unwrap();
    let terrain = SessionTerrain::from_planes(
        crate::native_world::MapType::Night,
        crate::native_world::parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        0,
    );
    let player = entities
        .iter()
        .find(|entity| entity.active && entity.class == 3 && entity.model == 0)
        .expect("Mission 18 retains a class3/model0 owner");
    let parent_slot = player.index;
    let owner_id = player.source_id;
    let position = player.position;
    let economy = crate::native_world::Level2EconomyState::from_parts_with_castle_stage_footprints(
        terrain.clone(),
        &entities,
        player.mana,
        player.model,
        0,
        0,
        runtime.random_state,
        Default::default(),
        runtime.constructor_counters(),
    )
    .unwrap();
    (
        crate::campaign::source_arena::SourceArena::new(entities, economy),
        runtime,
        terrain,
        presentation.particles.to_vec(),
        parent_slot,
        owner_id,
        position,
    )
}

fn family_with_cymmerian(slot: u16) -> [Vec<u16>; 29] {
    let mut families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    families[usize::from(SOURCE_MODEL)].push(slot);
    families
}

#[test]
fn source_constructor_uses_canonical_descriptor92_fields() {
    let particle = crate::native_world::ParticleParameter {
        base_tmap: 0,
        word_2: 0,
        word_4: 0,
        speed: 90,
        rotation_speed: 52,
        byte_10: 0,
        byte_11: 0,
        render_mode: 0,
        byte_13: 0,
    };
    let particles = vec![particle; usize::from(PARTICLE) + 1];
    let row = mc2::construct_row(
        7,
        0x1234,
        Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        },
        9,
        &particles,
    )
    .unwrap();

    assert_eq!(
        (row.class, row.model, row.action),
        (SOURCE_CLASS, SOURCE_MODEL, 201)
    );
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(
        (row.max_life, row.life, row.mana),
        (RULES.max_life, RULES.max_life, RULES.initial_mana)
    );
    assert_eq!(row.subspell_index, RULES.attack_damage);
    assert_eq!(row.phase, 9);
    assert_eq!(row.array_pitch(), RULES.collision_half_width);
    assert_eq!(row.array_roll(), RULES.collision_half_width);
    assert_eq!(row.array_fov(), RULES.collision_half_height);
}

#[test]
fn ordinary_seek_acquires_a_live_target_and_moves_to_action202() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    // The source scans the frozen class-3 chain, not every nearby entity.
    let targets = [target(41, 5, 18, position), target(42, 3, 0, position)];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 202);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 42);
    assert!(effects.attack.is_none());
}

#[test]
fn ordinary_seek_keeps_searching_when_no_target_is_in_source_range() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let targets = [target(
        42,
        3,
        0,
        Position {
            x: position.x.wrapping_add(RULES.target_range as u16 + 1),
            ..position
        },
    )];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 201);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
}

#[test]
fn authored_follow_leader_attacks_only_its_in_range_target() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let mut runtime = session.session.state.live.core.runtime.clone();
    runtime.stage_variables[1].flags = 0;
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::LiveEntity(42);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for (offset, expected_action) in [(0_u16, 202), (RULES.target_range as u16 + 100, 207)] {
        let mut leader = target(42, 5, 18, position);
        leader.target = Some(43);
        let enemy = target(
            43,
            3,
            0,
            Position {
                x: position.x.wrapping_add(offset),
                ..position
            },
        );
        let targets = [leader, enemy];
        let mut row = cymmerian(207, position);
        row.record[76] = 1;
        row.record[77] = 4;
        row.set_phase(8);
        let context = Context {
            terrain,
            targets: &targets,
            class3: &[43],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        assert_eq!(
            stage::step_action_207(
                &mut Actor { row: &mut row },
                &context,
                &runtime,
                &families,
                true,
                &mut Effects::default(),
            )
            .unwrap(),
            None
        );
        assert_eq!(row.action, expected_action);
        if expected_action == 202 {
            assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 43);
            assert_eq!(row.record[77], 10);
        }
    }

    // The ordinary Level22 route also binds StageVar8. 8 and 9 share
    // sub_1E1C0: a clear mailbox moves and wanders on phase 0, an enemy hit
    // enters the real attack action, and a same-family hit does not.
    for mode in [8_u8, 9] {
        let mut row = cymmerian(207, position);
        row.record[77] = mode;
        row.set_phase(0);
        row.set_control_flags(row.control_flags | 0x800);
        let old_rng = row.entity_rng;
        let old_roll = row.source_roll();
        let context = Context {
            terrain,
            targets: &[],
            class3: &[],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        let next_rng = level2_next_rng16(old_rng);
        assert_eq!(row.action, 207);
        assert_eq!(row.entity_rng, next_rng);
        assert_eq!(
            row.source_roll(),
            old_roll.wrapping_add((next_rng % 113 + 142) as i16) & 2047
        );

        for (attacker_model, damage, action, next_mode) in [
            (18, 100, 202, 10),
            (SOURCE_MODEL, 100, 207, mode),
            (18, RULES.max_life + 1, 204, mode),
        ] {
            let targets = [target(42, SOURCE_CLASS, attacker_model, position)];
            let context = Context {
                terrain,
                targets: &targets,
                class3: &[],
                model25: &[],
                reader: None,
                read_error: std::cell::Cell::new(None),
            };
            let mut row = cymmerian(207, position);
            row.record[77] = mode;
            row.record[98..102].copy_from_slice(&damage.to_le_bytes());
            row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
            stage::step_action_207(
                &mut Actor { row: &mut row },
                &context,
                &runtime,
                &families,
                true,
                &mut Effects::default(),
            )
            .unwrap();
            assert_eq!((row.action, row.record[77]), (action, next_mode));
            assert_eq!(row.life, RULES.max_life - damage);
            assert_eq!(u16::from_le_bytes([row.record[102], row.record[103]]), 0);
            if action == 202 {
                assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 42);
            }
        }
    }

    let ground = terrain.ground_height(position);
    let raised = Position {
        z: ground + 512,
        ..position
    };
    for (descriptor, expected_z) in [(DESCRIPTOR, ground + 480), (SPLIT_DESCRIPTOR, ground + 256)] {
        let targets = [target(42, SOURCE_CLASS, 18, raised)];
        let context = Context {
            terrain,
            targets: &targets,
            class3: &[],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        let mut row = cymmerian(207, raised);
        row.parameter_relation = Some(descriptor);
        row.record[77] = 8;
        row.record[98..102].copy_from_slice(&100_i32.to_le_bytes());
        row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!(
            row.position.z, expected_z,
            "hit uses its current descriptor's descent"
        );
    }
}

#[test]
fn stage_follow_modes_choose_leader_only_when_requested() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let mut runtime = session.session.state.live.core.runtime.clone();
    runtime.stage_variables[1].flags = 0;
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::LiveEntity(42);
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let leader = target(42, 5, 18, position);
    let context = Context {
        terrain,
        targets: &[leader],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for (mode, action, expected_target) in [(3, 202, 42), (4, 207, 0), (5, 207, 0)] {
        let mut row = cymmerian(207, position);
        row.record[76] = 1;
        row.record[77] = mode;
        row.set_phase(8);
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!((row.action, word(&row, 154)), (action, expected_target));
    }
    let mut unreadable_leader = leader;
    unreadable_leader.target = None;
    let context = Context {
        terrain,
        targets: &[unreadable_leader],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    let mut row = cymmerian(207, position);
    row.record[76] = 1;
    row.record[77] = 4;
    row.set_phase(8);
    let error = stage::step_action_207(
        &mut Actor { row: &mut row },
        &context,
        &runtime,
        &families,
        true,
        &mut Effects::default(),
    )
    .unwrap_err();
    assert!(error.contains("target 42 lacks current target"));
}

#[test]
fn stage_guard_acquires_only_visible_class3_on_scan_cadence() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let position = session.state.position;
    let terrain = session.canonical_terrain();
    let mut runtime = session.session.state.live.core.runtime.clone();
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::AxisScalar(
        u32::from(position.x) | (u32::from(position.y) << 16),
    );
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    for (hidden, phase, expected) in [(false, 20, 202), (true, 20, 207), (false, 19, 207)] {
        let mut enemy = target(42, 3, 0, position);
        if hidden {
            enemy.flags |= 0x20;
        }
        let targets = [enemy];
        let context = Context {
            terrain,
            targets: &targets,
            class3: &[42],
            model25: &[],
            reader: None,
            read_error: std::cell::Cell::new(None),
        };
        let mut row = cymmerian(207, position);
        row.record[76] = 1;
        row.record[77] = 2;
        row.set_phase(phase);
        row.set_yaw(crate::heading_for_positions(position, position) as i16);
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            &runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!(row.action, expected);
        assert_eq!(word(&row, 154), if expected == 202 { 42 } else { 0 });
    }
}

#[test]
fn stage_default_preserves_actor_but_attack_mode_selects_source_action() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let position = session.state.position;
    let runtime = &session.session.state.live.core.runtime;
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let context = Context {
        terrain: session.canonical_terrain(),
        targets: &[],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for (mode, action) in [(0, 207), (11, 207), (15, 207), (10, 202)] {
        let mut row = cymmerian(207, position);
        row.record[77] = mode;
        put(&mut row, 154, 42);
        let rng = row.entity_rng;
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            &families,
            true,
            &mut Effects::default(),
        )
        .unwrap();
        assert_eq!(
            (row.action, row.record[77], word(&row, 154), row.entity_rng),
            (action, mode, 42, rng)
        );
    }
}

#[test]
fn stage_morph_tracks_current_parent_and_hides_without_it() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let runtime = &session.session.state.live.core.runtime;
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut parent = target(42, 3, 0, Position { z: 900, ..position });
    parent.yaw = Some(123);
    parent.roll = Some(321);
    let context = Context {
        terrain,
        targets: &[parent],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for current in [true, false] {
        let mut row = cymmerian(207, position);
        row.record[77] = 12;
        put(&mut row, 44, 42);
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            &families,
            current,
            &mut Effects::default(),
        )
        .unwrap();
        if current {
            assert_eq!(
                (row.position.z, row.yaw, row.source_roll()),
                (388, 123, 321)
            );
            assert_eq!(row.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        } else {
            assert_ne!(row.control_flags & ENTITY_DRAW_DISABLED_BIT, 0);
        }
    }
    parent.position.z = 200;
    let context = Context {
        terrain,
        targets: &[parent],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    let mut row = cymmerian(207, position);
    row.record[77] = 12;
    put(&mut row, 44, 42);
    stage::step_action_207(
        &mut Actor { row: &mut row },
        &context,
        runtime,
        &families,
        true,
        &mut Effects::default(),
    )
    .unwrap();
    assert_eq!(row.position.z, 0);
}

#[test]
fn alliance_attacks_parent_target_then_expires_on_parent_loss() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let terrain = session.canonical_terrain();
    let position = session.state.position;
    let runtime = &session.session.state.live.core.runtime;
    let families: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let mut parent = target(42, 3, 0, position);
    parent.target = Some(43);
    let enemy = target(43, 5, 18, position);
    let targets = [parent, enemy];
    let context = Context {
        terrain,
        targets: &targets,
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    for current in [true, false] {
        let mut row = cymmerian(207, position);
        row.record[77] = 14;
        put(&mut row, 44, 42);
        put(&mut row, 50, 40);
        put(&mut row, 52, 60);
        put(&mut row, 78, 3);
        let mut effects = Effects::default();
        stage::step_action_207(
            &mut Actor { row: &mut row },
            &context,
            runtime,
            &families,
            current,
            &mut effects,
        )
        .unwrap();
        if current {
            assert_eq!((row.action, word(&row, 154), word(&row, 50)), (202, 43, 39));
            assert_eq!(effects.feedback, Some((42, 24)));
        } else {
            assert_eq!((row.record[77], row.record[76], word(&row, 44)), (10, 3, 0));
            assert_eq!(effects.feedback, None);
        }
    }
}

#[test]
fn ordinary_seek_waits_for_descriptor92_scan_cadence() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let targets = [target(42, 3, 0, position)];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    row.set_phase(RULES.scan_period.wrapping_sub(1));
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 201);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
}

#[test]
fn split_body_uses_descriptor95_scan_period_and_target_range() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: position.x.wrapping_add(3500),
        ..position
    };
    let targets = [target(42, 3, 0, target_position)];
    let class3 = [42];
    let mut row = cymmerian(201, position);
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.set_source_overlap_half_extents(RULES.split_half_extent, RULES.split_half_extent);
    row.record[92..94].copy_from_slice(&RULES.split_half_extent.to_le_bytes());
    row.set_phase(RULES.split_scan_period);
    row.set_yaw(heading_for_positions(position, target_position) as i16);
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.action, 202);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 42);
}

#[test]
fn ordinary_pursuit_requests_one_class9_model0_projectile_for_live_target() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: position.x.wrapping_add(1000),
        ..position
    };
    let targets = [target(42, 3, 0, target_position)];
    let class3 = [42];
    let mut row = cymmerian(202, position);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    let attack = effects
        .attack
        .expect("live in-range pursuit creates a shot");
    assert_eq!(
        (attack.target, attack.target_class, attack.target_model),
        (42, 3, 0)
    );
    assert_eq!(attack.parameter, RULES.projectile_damage);
    assert_eq!(
        attack.position.z,
        row.position.z.wrapping_add(row.array_fov())
    );
}

#[test]
fn ordinary_pursuit_drops_a_target_outside_descriptor92_range() {
    let terrain = terrain();
    let position = Position {
        x: 20_000,
        y: 20_000,
        z: 400,
    };
    let target_position = Position {
        x: position.x.wrapping_add(RULES.target_range as u16 + 1),
        ..position
    };
    let targets = [target(42, 3, 0, target_position)];
    let class3 = [42];
    let mut row = cymmerian(202, position);
    row.parameter_relation = Some(DESCRIPTOR);
    row.record[154..156].copy_from_slice(&42_u16.to_le_bytes());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert!(effects.attack.is_none());
    // sub_1C310 returns to base+1 directly when the target is out of range.
    assert_eq!(row.action, 201);
}

#[test]
fn ordinary_subspell_counter_and_state_timer_follow_source_fields() {
    let terrain = terrain();
    let targets = [];
    let class3 = [];
    let mut row = cymmerian(200, Position::default());
    row.set_subspell_index(RULES.attack_damage);
    row.record[74] = 3;
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert_eq!(row.subspell_index, RULES.attack_damage - 1);
    assert_eq!(word(&row, 46), RULES.attack_damage - 1);
    assert_eq!(row.source_dword_10(), 100);
    assert_eq!(row.action, 200);
    assert_eq!(row.record[74], 8);
}

#[test]
fn protected_landing_discards_damage_through_the_transition_to_state3() {
    let terrain = terrain();
    let mut row = cymmerian(
        200,
        Position {
            x: 20_000,
            y: 20_000,
            z: 400,
        },
    );
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.record[74] = 1;
    row.set_max_life(RULES.split_max_life);
    row.set_life(RULES.split_max_life);
    row.set_subspell_index(RULES.split_attack_damage);
    let mut context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    // Source28860 sets52 then decrements on the first visit. All53 visits,
    // including the final2->3 transition, restore life and clear word62.
    for visit in 0..53 {
        row.record[98..102].copy_from_slice(&1000_i32.to_le_bytes());
        row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
        step(&mut Actor { row: &mut row }, &mut context).unwrap();
        assert_eq!(row.life, RULES.split_max_life, "protected visit{visit}");
        assert_eq!(word(&row, 102), 0, "protected visit{visit}");
        assert_eq!(row.action, 200);
        assert_eq!(row.record[74], if visit == 52 { 3 } else { 2 });
        assert_eq!(word(&row, 46), RULES.split_attack_damage - visit - 1);
    }
    // No stale attacker survives protection. A newly delivered hit after
    // landing must still take the ordinary small-body prekill route.
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.life, RULES.split_max_life);
    row.record[98..102].copy_from_slice(&1000_i32.to_le_bytes());
    row.record[102..104].copy_from_slice(&42_u16.to_le_bytes());
    step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(row.action, 204);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert!(effects.prekill && !effects.split);
}

#[test]
fn water_visual_change_above_floor_preserves_speed_but_entry_and_land_set_it() {
    let (_, _, terrain, particles, _, _, _) = source_parts(18);
    let sample = |water: bool| {
        (0_u16..=255)
            .find_map(|y| {
                (0_u16..=255).find_map(|x| {
                    let mut point = Position {
                        x: x * 256 + 128,
                        y: y * 256 + 128,
                        z: 0,
                    };
                    ((terrain.material_mask(point) == crate::WATER_AMBIENCE_MASK) == water).then(
                        || {
                            point.z = terrain.ground_height(point) + 512;
                            point
                        },
                    )
                })
            })
            .expect("unchanged owner Mission18 has both water and land samples")
    };
    let water = sample(true);
    let land = sample(false);
    let mut row = cymmerian(200, water);
    row.parameter_relation = Some(SPLIT_DESCRIPTOR);
    row.record[74] = 4;
    row.set_source_dword_10(100);
    row.set_subspell_index(RULES.split_attack_damage);
    row.set_phase(1);
    put(&mut row, 136, RULES.split_minimum_speed as u16);
    row.set_actual_speed(123);
    crate::world::effects::bind_particle(&mut row, SPLIT_PARTICLE, &particles).unwrap();
    let mut context = Context {
        terrain: &terrain,
        targets: &[],
        class3: &[],
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };
    // Branch controls: position this existing small-body shape above real
    // water, keep byte46 unchanged, and use source movement-skip bit0x800 so
    // the callback cannot cross onto another tile. No terrain is rewritten.
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.visual, Some(313));
    assert!(
        !effects.speed_changed,
        "source visual-only branch leaves v26 clear"
    );
    assert_eq!((word(&row, 136), row.actual_speed), (35, 123));

    crate::world::effects::bind_particle(&mut row, 313, &particles).unwrap();
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.visual, Some(SPLIT_PARTICLE));
    assert!(effects.speed_changed);
    assert_eq!((word(&row, 136), row.actual_speed), (35, 35));

    crate::world::effects::bind_particle(&mut row, SPLIT_PARTICLE, &particles).unwrap();
    row.set_position(land);
    row.set_control_flags(row.control_flags | 0x800);
    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();
    assert_eq!(effects.visual, Some(313));
    assert!(effects.speed_changed);
    assert_eq!((word(&row, 136), row.actual_speed), (60, 60));
}

#[test]
fn ordinary_death_callback_requests_three_children_before_parent_kill() {
    let terrain = terrain();
    let mut row = cymmerian(204, Position::default());
    let targets = [];
    let class3 = [];
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let effects = step(&mut Actor { row: &mut row }, &mut context).unwrap();

    assert!(effects.split);
    assert_eq!(effects.population_increments, 3);
    assert_eq!(row.action, 205);
}

#[test]
fn action206_is_rejected_because_the_pinned_source_has_no_handler() {
    let terrain = terrain();
    let targets = [];
    let class3 = [];
    let mut row = cymmerian(206, Position::default());
    let mut context = Context {
        terrain: &terrain,
        targets: &targets,
        class3: &class3,
        model25: &[],
        reader: None,
        read_error: std::cell::Cell::new(None),
    };

    let error = step(&mut Actor { row: &mut row }, &mut context).unwrap_err();

    assert!(error.contains("action206 has no implementation"));
    assert_eq!(row.action, 206);
}

#[test]
fn army_ring_allocates_a_real_summoned_cymmerian_with_holder_provenance() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder's source slot");
    let counter_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
    let initial_counter = runtime.bytes[counter_at];

    let handle = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        holder_slot,
    )
    .unwrap()
    .expect("ordinary Army ring has source capacity");

    let row = &arena.entities[usize::from(handle.source_slot)];
    assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 207));
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.particle_parameter_index, Some(PARTICLE));
    assert_eq!((row.max_life, row.life), (RULES.max_life, RULES.max_life));
    assert_eq!(row.actual_speed, RULES.minimum_speed);
    assert_eq!(row.subspell_index, RULES.attack_damage);
    assert_eq!(row.phase, initial_counter);
    assert_eq!(row.record[74], 0, "ordinary constructor keeps byte46 clear");
    assert_eq!(
        row.record[77],
        crate::native_world::summon_army::SUMMONED_STAGE_MODE
    );
    assert_eq!(
        u16::from_le_bytes([row.record[44], row.record[45]]),
        parent_slot
    );
    assert_eq!(
        u16::from_le_bytes([row.record[30], row.record[31]]),
        owner_id
    );
    assert_eq!(row.source_id, owner_id);
    assert_eq!(
        u16::from_le_bytes([row.record[50], row.record[51]]),
        crate::native_world::summon_army::SUMMONED_REMAINING_LIFETIME
    );
    assert_eq!(row.active_value, 250);
    assert_eq!(u16::from_le_bytes([row.record[154], row.record[155]]), 0);
    assert_eq!((row.mana, row.max_mana, row.mana_cost), (0, 0, 0));
    assert!(row.record[144..148].iter().all(|byte| *byte == 0));
    assert_eq!(row.position, position);
    assert_eq!(runtime.bytes[counter_at], initial_counter.wrapping_add(1));

    let identity = arena
        .imported_creature_runtime_identity(handle.source_slot, &std::collections::BTreeMap::new())
        .expect("summoned Cymmerian has live raw-creature identity");
    assert_eq!(identity.handle, handle);
    assert_eq!(
        identity.origin,
        crate::campaign::source_arena::CreatureRootOrigin::SummonedSpell { holder_slot }
    );
}

#[test]
fn summoned_cymmerian_follows_reused_parent_until_it_dies() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    let holder = arena.allocate_source_slot().unwrap();
    let child = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        holder,
    )
    .unwrap()
    .unwrap();
    let parent = arena.handle(parent_slot).unwrap();
    assert_eq!(arena.creature_parent(child), Some(parent));

    let mut freed = arena.clone();
    let mut freed_runtime = runtime.clone();
    let mut freed_terrain = terrain.clone();
    freed.entities[usize::from(parent_slot)].set_life(0);
    freed.entities[usize::from(parent_slot)].reclaim_source_row();
    freed.economy.release_slot(parent_slot, None);
    let families = family_with_cymmerian(child.source_slot);
    mc2::step_row(
        &mut freed, &mut freed_runtime, &mut freed_terrain, &particles, child.source_slot,
        &[], &families, crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert!(!freed.entities[usize::from(child.source_slot)].drawing_disabled());

    arena.economy.release_slot(parent_slot, None);
    assert_eq!(
        arena.allocate_source_slot_with_terrain(&mut terrain),
        Some(parent_slot)
    );
    let mut replacement = crate::native_world::cave_features::new_event(parent_slot, 5, position);
    replacement.class = 3;
    replacement.model = 0;
    replacement.record[67] = 3;
    replacement.set_life(1000);
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot), Some(parent));
    assert_eq!(arena.creature_parent(child), None);

    let families = family_with_cymmerian(child.source_slot);
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        child.source_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(!arena.entities[usize::from(child.source_slot)].drawing_disabled());
    arena.entities[usize::from(parent_slot)].set_life(-1);
    mc2::step_row(
        &mut arena, &mut runtime, &mut terrain, &particles, child.source_slot,
        &[], &families, crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert!(arena.entities[usize::from(child.source_slot)].drawing_disabled());
}

#[test]
fn authored_cymmerian_alliance_impact_follows_current_parent_until_death() {
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        mission_eighteen_root().join("level10"),
        crate::campaign::spec::CampaignSelection::ordinary(10).unwrap(),
    )
    .expect("decode real Mission 10 authored records");
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|row| row.kind == 5 && row.subtype == 25)
        .expect("Mission 10 contains an authored Cymmerian");
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, _, _) = source_parts(10);
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        feature.index,
    )
    .unwrap()
    .expect("authored Cymmerian allocation");
    let child = arena.handle(slot).unwrap();
    let parent = arena.handle(parent_slot).unwrap();
    // The same authored body visits the real source route prepass. Its
    // target relation survives a live player and clears when that carrier dies.
    for dead in [false, true] {
        let mut route_arena = arena.clone();
        let mut route_runtime = runtime.clone();
        let row = &mut route_arena.entities[usize::from(slot)];
        row.set_source_action(207);
        row.record[76] = 1;
        row.record[77] = 3;
        put(row, 78, parent_slot);
        route_arena.entities[usize::from(parent_slot)].set_life(if dead { -1 } else { 1000 });
        route_runtime.stage_variables[1].kind = 3;
        route_runtime.stage_variables[1].flags = 2;
        route_runtime.stage_variables[1].relation =
            crate::native_world::RuntimeRelation::AxisScalar(3);
        mc2::prepare_source_pass(
            &mut route_arena,
            &mut route_runtime,
            &[slot],
            crate::native_world::source_target::Context::default(),
        )
        .unwrap();
        assert_eq!(
            word(&route_arena.entities[usize::from(slot)], 78),
            if dead { 0 } else { parent_slot }
        );
    }
    let old_parent = arena
        .entities
        .iter()
        .find(|row| {
            row.active
                && row.index != 0
                && row.index != parent_slot
                && row.index != slot
                && arena.handle(row.index).is_some()
        })
        .and_then(|row| arena.handle(row.index))
        .expect("owner package has a prior live source parent");
    assert!(arena.bind_creature_parent(child, old_parent));
    let impact_slot =
        crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain)
            .expect("allocate Alliance's real impact carrier");
    let impact = &mut arena.entities[usize::from(impact_slot)];
    impact.source_id = parent_slot;
    impact.record[30..32].copy_from_slice(&parent_slot.to_le_bytes());
    impact.record[74] = 32;
    impact.subspell_index = 40;
    impact.record[154..156].copy_from_slice(&slot.to_le_bytes());
    let outcome =
        crate::native_world::alliance_spell::step_impact(&mut arena, impact_slot, &[slot]).unwrap();
    assert_eq!(outcome.affected_entities, vec![slot]);
    assert_eq!(arena.creature_parent(child), Some(parent));
    assert_eq!(arena.entities[usize::from(slot)].record[77], 14);
    arena.entities[usize::from(slot)].set_phase(1);
    let families = family_with_cymmerian(slot);
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(word(&arena.entities[usize::from(slot)], 50), 39);
    assert_eq!(arena.entities[usize::from(slot)].record[77], 14);

    arena.economy.release_slot(parent_slot, None);
    assert_eq!(
        arena.allocate_source_slot_with_terrain(&mut terrain),
        Some(parent_slot)
    );
    let mut replacement = crate::native_world::cave_features::new_event(parent_slot, 5, position);
    replacement.class = 3;
    replacement.record[67] = 3;
    replacement.set_life(1000);
    arena.entities[usize::from(parent_slot)] = replacement;
    assert_ne!(arena.handle(parent_slot), Some(parent));
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(arena.entities[usize::from(slot)].record[77], 14);
    assert_eq!(word(&arena.entities[usize::from(slot)], 44), parent_slot);
    arena.entities[usize::from(parent_slot)].set_life(0);
    mc2::step_row(
        &mut arena, &mut runtime, &mut terrain, &particles, slot, &[parent_slot], &families,
        crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert_eq!(arena.entities[usize::from(slot)].record[77], 10);
    assert_eq!(word(&arena.entities[usize::from(slot)], 44), 0);
}

/// Continue the existing public Army cast against a real Night rival. This
/// remains separate from the baseline cast's expiry assertions.
pub(crate) fn assert_night_army_cymmerian_lifetime(
    campaign: &crate::campaign::CampaignSession,
    player: u16,
    holder: u16,
) {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::native_world::Mc2DispatchEvent;
    use std::collections::{BTreeMap, BTreeSet};

    let mut attack = campaign.clone();
    let current = attack.simulation();
    let owner_id = current.entities()[usize::from(player)].source_id;
    let rival = current
        .entities()
        .iter()
        .find(|row| {
            row.active
                && row.class == 3
                && row.model == 1
                && row.life > 0
                && row.source_id != owner_id
        })
        .expect("Night 7 owner data contains an opposing living rival");
    let rival_slot = rival.index;
    let position = Position {
        x: rival.position.x.wrapping_add(1024),
        y: rival.position.y,
        z: rival.position.z.saturating_add(512),
    };
    attack
        .simulation_mut()
        .set_level2_entity_position(player, position)
        .unwrap();
    let parent_handle = attack.simulation().source_arena().handle(player).unwrap();
    let mut pursuing = None;
    for turn in 0..480 {
        attack.step(
            if turn == 0 {
                PlayerCommand {
                    opcode: 6,
                    argument_1: 19,
                    flags: 0x40,
                    ..PlayerCommand::default()
                }
            } else {
                PlayerCommand::default()
            },
            0,
        );
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army pursuit dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        pursuing = arena.entities.iter().find_map(|row| {
            if !row.active
                || (row.class, row.model, row.action) != (5, 25, 202)
                || u16::from_le_bytes([row.record[154], row.record[155]]) != rival_slot
            {
                return None;
            }
            let identity = arena.imported_creature_runtime_identity(row.index, &BTreeMap::new())?;
            (identity.origin
                == CreatureRootOrigin::SummonedSpell {
                    holder_slot: holder,
                }
                && arena.creature_parent_binding(identity.handle) == Some(parent_handle))
            .then_some(identity.handle)
        });
        if pursuing.is_some() {
            break;
        }
    }
    let parent = pursuing.expect("actual Army Cymmerian must pursue the real rival");
    eprintln!("Night7 Army Cymmerian pursuit: parent={parent:?}, rival={rival_slot}");
    let phase = attack.simulation().source_arena().entities[usize::from(parent.source_slot)].phase;
    crate::native_world::area_damage::contact(
        attack.simulation_mut().source_arena_mut(),
        None,
        parent.source_slot,
        u16::MAX,
        rival_slot,
    )
    .unwrap();
    attack.step(PlayerCommand::default(), 0);
    let session = attack.simulation();
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "Night Army damage dispatch failed: {:?}",
        session.dispatch_events()
    );
    let Some(SourceEntityView::Imported(row)) = session.source_arena().resolve(parent) else {
        panic!("damaged Cymmerian must retain its original source generation");
    };
    assert_eq!(
        row.life, 0,
        "source28C60 clamps the lethal attack callback tail"
    );
    assert_eq!(row.action, 204, "the real hit selects the death callback");
    assert_eq!(
        row.phase,
        phase.wrapping_add(1),
        "the same-pass source tail runs after the hit"
    );
    assert!(
        session
            .source_arena()
            .imported_creature_runtime_identity(parent.source_slot, &BTreeMap::new())
            .expect("the original dying root retains its semantic identity")
            .living
    );

    let mut children = BTreeMap::new();
    let mut parent_hidden = false;
    let mut parent_reclaimed = false;
    for _ in 0..96 {
        attack.step(PlayerCommand::default(), 0);
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army split dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        if let Some(SourceEntityView::Imported(row)) = arena.resolve(parent) {
            parent_hidden |= row.drawing_disabled();
        } else {
            parent_reclaimed |= parent_hidden;
        }
        for row in arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 25)
        {
            let Some(identity) =
                arena.imported_creature_runtime_identity(row.index, &BTreeMap::new())
            else {
                continue;
            };
            if identity.origin
                == (CreatureRootOrigin::CreatureConversion {
                    producer_slot: parent.source_slot,
                })
            {
                children.insert(identity.handle.source_slot, identity.handle);
            }
        }
        if children.len() == 3 && parent_reclaimed {
            break;
        }
    }
    assert_eq!(
        children.len(),
        3,
        "real large death creates its three generation-bound children"
    );
    assert!(
        parent_hidden && parent_reclaimed,
        "parent hides before following-pass reclaim"
    );

    let mut landed = BTreeSet::new();
    for _ in 0..160 {
        attack.step(PlayerCommand::default(), 0);
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army child landing dispatch failed: {:?}",
            session.dispatch_events()
        );
        for (&slot, child) in &children {
            let Some(SourceEntityView::Imported(row)) = session.source_arena().resolve(*child)
            else {
                panic!("split child {slot} must survive to landing");
            };
            assert_eq!((row.class, row.model), (5, 25));
            let identity = session
                .source_arena()
                .imported_creature_runtime_identity(slot, &BTreeMap::new())
                .expect("the real split child retains its semantic root");
            assert_eq!(identity.handle, *child);
            assert!(
                identity.living,
                "a live split child remains in the living-root consumer"
            );
            if row.record[74] >= 3 {
                landed.insert(slot);
            }
        }
        if landed.len() == children.len() {
            break;
        }
    }
    assert_eq!(
        landed.len(),
        children.len(),
        "all three small children land and enter their acting state"
    );
    let phases = children
        .iter()
        .map(|(&slot, child)| {
            let Some(SourceEntityView::Imported(row)) =
                attack.simulation().source_arena().resolve(*child)
            else {
                panic!("landed child {slot} lost its source generation");
            };
            (slot, row.phase)
        })
        .collect::<BTreeMap<_, _>>();
    attack.step(PlayerCommand::default(), 0);
    let session = attack.simulation();
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "Night Army post-landing dispatch failed: {:?}",
        session.dispatch_events()
    );
    for (&slot, child) in &children {
        let Some(SourceEntityView::Imported(row)) = session.source_arena().resolve(*child) else {
            panic!("landed child {slot} must receive its next source visit");
        };
        assert_eq!(
            row.phase,
            phases[&slot].wrapping_add(1),
            "child {slot} acts after landing"
        );
    }
    for child in children.values() {
        crate::native_world::area_damage::contact(
            attack.simulation_mut().source_arena_mut(),
            None,
            child.source_slot,
            u16::MAX,
            rival_slot,
        )
        .unwrap();
    }
    let mut hidden = BTreeSet::new();
    let mut reclaimed = BTreeSet::new();
    for _ in 0..160 {
        attack.step(PlayerCommand::default(), 0);
        let session = attack.simulation();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Night Army child death dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        for (&slot, child) in &children {
            match arena.resolve(*child) {
                Some(SourceEntityView::Imported(row)) if row.drawing_disabled() => {
                    let identity = arena
                        .imported_creature_runtime_identity(slot, &BTreeMap::new())
                        .expect("hidden child remains a source root until reclaim");
                    assert!(
                        !identity.living,
                        "hidden child is excluded from living roots"
                    );
                    hidden.insert(slot);
                }
                None if hidden.contains(&slot) => {
                    reclaimed.insert(slot);
                }
                _ => {}
            }
        }
        for row in arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 25)
        {
            if let Some(identity) =
                arena.imported_creature_runtime_identity(row.index, &BTreeMap::new())
            {
                assert!(
                    !children.keys().any(|slot| identity.origin
                        == (CreatureRootOrigin::CreatureConversion {
                            producer_slot: *slot
                        })),
                    "small children must not split again"
                );
            }
        }
        if reclaimed.len() == children.len() {
            break;
        }
    }
    assert_eq!(
        hidden.len(),
        children.len(),
        "all small children hide in their death pass"
    );
    assert_eq!(
        reclaimed.len(),
        children.len(),
        "all small children reclaim on a later pass"
    );
    eprintln!(
        "Night7 Army Cymmerian split, landed, hid and reclaimed: parent={parent:?}, children={children:?}"
    );
}

#[test]
fn level_ten_buildings_release_switch_group_into_cymmerian_guard() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityView};
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::{FireballProjectileOrigin, Mc2DispatchEvent};
    use std::collections::{BTreeMap, BTreeSet};

    let campaign = CampaignSession::open(
        mission_eighteen_root().join("level10"),
        CampaignSelection::ordinary(10).unwrap(),
    )
    .expect("real Level 10 campaign package");
    let mut session = campaign.simulation().clone();
    let player = session.state.entity_index;
    let owner = usize::from(session.players()[0].player_color_index());
    let switch = &session.entities()[1];
    assert_eq!(
        (switch.class, switch.model, switch.action, switch.source_id),
        (11, 32, 32, 8)
    );
    assert_eq!(session.runtime().stages[0].kind, 9);
    assert_eq!(session.runtime().objectives[owner].stage_selectors[0], 1);
    let initial = session.campaign_building_objective_targets(0);
    assert_eq!(
        initial.len(),
        6,
        "owner data starts with six standing chain-23 buildings"
    );
    let kept = (initial[0].1.x, initial[0].1.y);
    let expected_group = session
        .mission()
        .authored_features()
        .iter()
        .filter(|feature| {
            feature.index != 0
                && feature.index < 1200
                && feature.kind != 0
                && feature.activation_group == 8
        })
        .map(|feature| feature.index)
        .collect::<Vec<_>>();
    assert_eq!(expected_group.len(), 25);
    let mut damaged = BTreeSet::new();
    let mut only_kept = false;

    // Apply real source mailbox damage and allow the ordinary House callbacks
    // to construct each 23 -> 24 -> 28 successor. One House remains intact.
    for _ in 0..256 {
        let targets = session.campaign_building_objective_targets(0);
        if targets.iter().all(|(_, p)| (p.x, p.y) == kept) && !targets.is_empty() {
            only_kept = true;
            break;
        }
        for (slot, position) in targets {
            if (position.x, position.y) == kept {
                continue;
            }
            let ready = session
                .session
                .state
                .live
                .ordinary
                .houses()
                .iter()
                .any(|house| house.entity_index == slot && house.active && house.action == 52);
            if !ready {
                continue;
            }
            let handle = session
                .source_arena()
                .handle(slot)
                .expect("live House source handle");
            if damaged.insert((slot, handle.generation)) {
                assert!(
                    session
                        .session
                        .state
                        .live
                        .ordinary
                        .houses
                        .submit_source_damage(slot, u16::MAX, player)
                );
            }
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "ordinary House cleanup dispatch failed: {:?}",
            session.dispatch_events()
        );
        assert!(
            !session.dispatch_events().iter().any(|event| matches!(
                event,
                Mc2DispatchEvent::AuthoredGroupActivated { group: 8, .. }
            )),
            "a surviving source House must keep the objective switch inert"
        );
    }
    assert!(
        only_kept,
        "other actual House destruction chains must finish"
    );
    assert_eq!(session.runtime().objectives[owner].stage_selectors[0], 1);

    let mut activated = false;
    for _ in 0..256 {
        for (slot, _) in session.campaign_building_objective_targets(0) {
            let ready = session
                .session
                .state
                .live
                .ordinary
                .houses()
                .iter()
                .any(|house| house.entity_index == slot && house.active && house.action == 52);
            if !ready {
                continue;
            }
            let handle = session
                .source_arena()
                .handle(slot)
                .expect("live House source handle");
            if damaged.insert((slot, handle.generation)) {
                assert!(
                    session
                        .session
                        .state
                        .live
                        .ordinary
                        .houses
                        .submit_source_damage(slot, u16::MAX, player)
                );
            }
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "ordinary switch/group dispatch failed: {:?}",
            session.dispatch_events()
        );
        if let Some(indices) = session
            .dispatch_events()
            .iter()
            .find_map(|event| match event {
                Mc2DispatchEvent::AuthoredGroupActivated {
                    source_slot: 1,
                    group: 8,
                    authored_indices,
                    ..
                } => Some(authored_indices),
                _ => None,
            })
        {
            assert_eq!(indices.as_ref(), expected_group.as_slice());
            assert_eq!(indices[0], 31);
            activated = true;
            break;
        }
    }
    assert!(
        activated,
        "naturally completed building objective must fire the real group-8 switch"
    );
    assert_eq!(session.runtime().objectives[owner].stage_selectors[0], 2);
    let empty_bindings = BTreeMap::new();
    let guard = session
        .entities()
        .iter()
        .find_map(|row| {
            let identity = session
                .source_arena()
                .imported_creature_runtime_identity(row.index, &empty_bindings)?;
            (identity.origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 31,
                })
            .then_some(identity.handle)
        })
        .expect("source row31 produced its live generation-bound Cymmerian");
    let row = &session.entities()[usize::from(guard.source_slot)];
    assert_eq!((row.class, row.model, row.record[76]), (5, 25, 1));
    assert!(
        matches!(row.record[77], 2 | 10),
        "row31 uses its authored guard route"
    );

    // Place the actual player inside the guard's view cone; callbacks and
    // projectile allocation then run only through the ordinary scheduler.
    let position = row.position;
    let yaw = row.yaw;
    let actor_id = row.source_id;
    let candidates = [
        (2048_i16, 0_i16),
        (-2048, 0),
        (0, 2048),
        (0, -2048),
        (1448, 1448),
        (1448, -1448),
        (-1448, 1448),
        (-1448, -1448),
    ];
    let point = candidates
        .into_iter()
        .map(|(dx, dy)| Position {
            x: position.x.wrapping_add_signed(dx),
            y: position.y.wrapping_add_signed(dy),
            z: position.z,
        })
        .find(|point| {
            angle_distance(i32::from(yaw), heading_for_positions(position, *point))
                < RULES.target_half_angle
        })
        .expect("a bounded nearby position lies in the guard's source view cone");
    session.set_level2_entity_position(player, point).unwrap();
    let mut fired = false;
    let mut prepared_pursuit_audio = false;
    // The actual initial phase is1: scan20 acquires the target, then
    // pursuit at40 emits the first shot. The previous32-visit deadline
    // ended at33, before that second source cadence. Preserve both scans.
    let acquisition_and_attack_visits = 2 * usize::from(RULES.scan_period) + 1;
    for _ in 0..acquisition_and_attack_visits {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "ordinary guard/projectile dispatch failed: {:?}",
            session.dispatch_events()
        );
        fired |= session
            .economy()
            .fireball_projectiles()
            .iter()
            .any(|projectile| {
                projectile.origin == FireballProjectileOrigin::Cymmerian
                    && projectile.owner_entity_index == actor_id
                    && projectile.target_source_entity_index == player
            });
        prepared_pursuit_audio |= session.effect_journal().records().iter().any(|record| {
            matches!(record.effect,
                crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                    entity_index, player_index: -1, sound_id: 37, ..
                }) if entity_index == guard.source_slot
            ) && record.audio.is_some()
        });
        if fired {
            break;
        }
    }
    assert!(
        fired,
        "the real row31 guard must reach its ordinary projectile consumer"
    );
    assert!(
        prepared_pursuit_audio,
        "the actual guard must publish prepared audio consumed by native playback, not only an economy receipt"
    );

    // A real Castle is the source-required target for the small bodies'
    // action200 state5/6/7 combat. Its location and the later lethal hits are
    // explicit check controls; the row31 producer and all callbacks stay real.
    let castle_position = session.entities()[usize::from(guard.source_slot)].position;
    let castle = session
        .economy_mut()
        .build_castle(castle_position, player)
        .expect("Castle combat prerequisite for the actual authored split");
    let phase = session.entities()[usize::from(guard.source_slot)].phase;
    crate::native_world::area_damage::contact(
        session.source_arena_mut(),
        None,
        guard.source_slot,
        u16::MAX,
        player,
    )
    .unwrap();
    session.step_outer_turn(PlayerCommand::default(), 0);
    let row = &session.entities()[usize::from(guard.source_slot)];
    assert_eq!((row.action, row.life), (204, 0));
    assert_eq!(row.phase, phase.wrapping_add(1));
    assert!(
        session
            .source_arena()
            .imported_creature_runtime_identity(guard.source_slot, &empty_bindings,)
            .unwrap()
            .living,
        "source28C60 retains the zero-life root until split"
    );
    assert!(session.source_arena().rebuilt_class5_model_slots()[25].contains(&guard.source_slot));

    let mut children = BTreeMap::new();
    let mut landed = BTreeSet::new();
    let mut parent_hidden = false;
    let mut parent_reclaimed = false;
    let mut small_hit = false;
    for _ in 0..320 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "authored split/combat dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        match arena.resolve(guard) {
            Some(SourceEntityView::Imported(row)) => parent_hidden |= row.drawing_disabled(),
            None => parent_reclaimed |= parent_hidden,
            _ => panic!("authored Cymmerian was replaced before natural reclaim"),
        }
        for row in arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && row.model == 25)
        {
            let Some(identity) =
                arena.imported_creature_runtime_identity(row.index, &empty_bindings)
            else {
                continue;
            };
            if identity.origin
                == (CreatureRootOrigin::CreatureConversion {
                    producer_slot: guard.source_slot,
                })
            {
                children.insert(row.index, identity.handle);
                assert_eq!(
                    row.action, 200,
                    "small body stays in its source-specific action200"
                );
                assert!(identity.living);
                assert_eq!(
                    word(row, 46),
                    row.subspell_index,
                    "small countdown raw bytes stay current"
                );
                if row.record[74] >= 3 {
                    landed.insert(row.index);
                }
            }
        }
        let target = arena
            .economy
            .castle_at(castle)
            .expect("Castle survives the bounded contact check");
        small_hit |= target.pending_damage > 0
            && children.keys().any(|&slot| {
                arena.entities[usize::from(slot)].source_id == target.pending_attacker_entity_index
            });
        if children.len() == 3 && landed.len() == 3 && parent_reclaimed && small_hit {
            break;
        }
    }
    assert_eq!(
        children.len(),
        3,
        "the ordinary large death creates three real small bodies"
    );
    assert_eq!(
        landed.len(),
        3,
        "all small bodies finish their unshortened landing"
    );
    assert!(parent_hidden && parent_reclaimed);
    assert!(
        small_hit,
        "a landed small body must damage the actual Castle mailbox"
    );

    for child in children.values() {
        crate::native_world::area_damage::contact(
            session.source_arena_mut(),
            None,
            child.source_slot,
            u16::MAX,
            player,
        )
        .unwrap();
    }
    let mut hidden = BTreeSet::new();
    let mut reclaimed = BTreeSet::new();
    for _ in 0..64 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "authored small-body death dispatch failed: {:?}",
            session.dispatch_events()
        );
        let arena = session.source_arena();
        for (&slot, &child) in &children {
            match arena.resolve(child) {
                Some(SourceEntityView::Imported(row)) if row.drawing_disabled() => {
                    assert!(
                        !arena
                            .imported_creature_runtime_identity(slot, &empty_bindings)
                            .unwrap()
                            .living
                    );
                    hidden.insert(slot);
                }
                None if hidden.contains(&slot) => {
                    reclaimed.insert(slot);
                }
                _ => {}
            }
        }
        assert!(
            !arena
                .entities
                .iter()
                .filter(|row| row.active && row.class == 5 && row.model == 25)
                .filter_map(
                    |row| arena.imported_creature_runtime_identity(row.index, &empty_bindings)
                )
                .any(|identity| children.keys().any(|&slot| identity.origin
                    == (CreatureRootOrigin::CreatureConversion {
                        producer_slot: slot
                    }))),
            "small children must not create another split generation"
        );
        if reclaimed.len() == 3 {
            break;
        }
    }
    assert_eq!(
        (hidden.len(), reclaimed.len()),
        (3, 3),
        "small death hides before following-pass reclaim"
    );
    let frozen = session.source_arena().rebuilt_class5_model_slots();
    assert!(!frozen[25].contains(&guard.source_slot));
    assert!(
        children.keys().all(|slot| !frozen[25].contains(slot)),
        "the source absence consumer drops the entire retired lineage"
    );
    eprintln!(
        "Level10 row31 ordinary Cymmerian combat/split/landing/Castle damage/death/reclaim: parent={guard:?}, children={children:?}"
    );
}

#[test]
fn army_action207_consumes_parent_timer_and_stage13_expiry_products() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    let holder_slot = arena
        .allocate_source_slot()
        .expect("reserve the Army holder source slot");
    let slot = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        holder_slot,
    )
    .unwrap()
    .unwrap()
    .source_slot;

    // sub_16FC0 selects from the pass-start class-3 chain without a life or
    // visibility test; the selected record is validated on the next visit.
    let mut selected = arena.clone();
    let mut selected_runtime = runtime.clone();
    let mut selected_terrain = terrain.clone();
    let target_slot = selected.allocate_source_slot_with_terrain(&mut selected_terrain).unwrap();
    let mut target = crate::native_world::cave_features::new_event(target_slot, 5, position);
    target.class = 3;
    target.model = 0;
    target.record[67] = 3;
    target.source_id = owner_id.wrapping_add(1);
    target.set_life(-1);
    selected.entities[usize::from(target_slot)] = target;
    selected.entities[usize::from(slot)].set_phase(0);
    mc2::step_row(
        &mut selected, &mut selected_runtime, &mut selected_terrain, &particles, slot,
        &[target_slot], &family_with_cymmerian(slot),
        crate::native_world::source_target::Context::default(),
    ).unwrap();
    assert_eq!(u16::from_le_bytes(selected.entities[usize::from(slot)].record[154..156].try_into().unwrap()), target_slot);
    assert_eq!(u16::from_le_bytes(selected.entities[usize::from(slot)].record[50..52].try_into().unwrap()), 249);

    arena.entities[usize::from(slot)].set_phase(1);
    let families = family_with_cymmerian(slot);
    let effects = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(
        !effects.stage_callback,
        "the leaf consumes action207 itself"
    );
    assert_eq!(
        u16::from_le_bytes([
            arena.entities[usize::from(slot)].record[50],
            arena.entities[usize::from(slot)].record[51],
        ]),
        245,
        "StageVar13 decrements the 250 timer, then applies its no-target four-tick penalty"
    );

    arena.entities[usize::from(slot)].record[44..46].copy_from_slice(&u16::MAX.to_le_bytes());
    arena.entities[usize::from(slot)].record[50..52].copy_from_slice(&1_u16.to_le_bytes());
    let effects = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[parent_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(
        !effects.stage_callback,
        "expired callback is not returned for root replay"
    );
    assert!(
        arena.entities[usize::from(slot)].control_flags
            & crate::native_world::ENTITY_DRAW_DISABLED_BIT
            != 0,
        "invalid parent expires and hides the summoned child"
    );
    assert!(
        arena
            .entities
            .iter()
            .any(|row| row.active && row.class == 10 && row.model == 73)
    );
    assert!(
        arena
            .entities
            .iter()
            .any(|row| row.active && row.class == 10 && row.model == 0)
    );
}

#[test]
fn ordinary_step_row_materializes_one_cymmerian_class9_model0_attack() {
    for light_enabled in [true, false] {
        let (mut arena, mut runtime, mut terrain, particles, _parent_slot, _owner_id, _position) =
            summoned_source_parts();
        arena.dynamic_lighting_enabled = light_enabled;
        let target = arena
            .entities
            .iter()
            .find(|row| row.active && row.class == 3 && row.model == 0 && row.life > 0)
            .expect("Mission18 retains a live class3/model0 target")
            .clone();
        let slot = mc2::allocate_authored(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            target.position,
            RULES.max_life,
            None,
            0,
        )
        .unwrap()
        .unwrap();
        {
            let row = &mut arena.entities[usize::from(slot)];
            row.set_source_action(202);
            row.set_phase(RULES.scan_period);
            row.record[154..156].copy_from_slice(&target.index.to_le_bytes());
        }
        let families = family_with_cymmerian(slot);
        let effects = mc2::step_row(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            slot,
            &[target.index],
            &families,
            crate::native_world::source_target::Context::default(),
        )
        .unwrap();
        assert!(
            effects.attack.is_none(),
            "the adapter consumes the attack intent"
        );
        let projectile = arena
            .economy
            .fireball_projectiles()
            .iter()
            .find(|projectile| {
                projectile.origin == crate::native_world::FireballProjectileOrigin::Cymmerian
            })
            .expect("action202 publishes a real Fireball carrier");
        assert_eq!((projectile.source_class, projectile.source_model), (9, 0));
        assert_eq!(
            (
                projectile.target_source_entity_index,
                projectile.target_class,
                projectile.target_model
            ),
            (target.index, target.class, target.model)
        );
        assert_eq!(projectile.source_subspell_index, RULES.projectile_damage);
        assert_eq!(projectile.control_flags & 0x0008_0000 != 0, light_enabled);
        let constructor_position = arena.entities[usize::from(slot)].position;
        let mut launched_position = constructor_position;
        launched_position.z = launched_position
            .z
            .wrapping_add(arena.entities[usize::from(slot)].array_fov());
        assert_eq!(projectile.position, launched_position);
        let projectile_slot = projectile.source_entity_index;
        let linked_lights = arena
            .dynamic_lighting
            .active_slots()
            .filter(|(_, light)| light.linked && light.source_entity_index == projectile_slot)
            .count();
        assert_eq!(
            linked_lights,
            if light_enabled { 1 } else { 0 },
            "Night Fireball light admission"
        );
        if light_enabled {
            let (_, light) = arena
                .dynamic_lighting
                .active_slots()
                .find(|(_, light)| light.linked && light.source_entity_index == projectile_slot)
                .unwrap();
            assert_eq!(light.position, constructor_position);
        }
    }
}

#[test]
fn ordinary_split_publishes_generation_bound_cymmerian_children() {
    let (mut arena, mut runtime, mut terrain, particles, attacker_slot, _owner_id, position) =
        summoned_source_parts();
    let parent_slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        RULES.max_life,
        None,
        0,
    )
    .unwrap()
    .unwrap();
    // Deliver damage through the real source mailbox; the ordinary seek
    // callback must select the death action before the split is visited.
    assert_eq!(arena.entities[usize::from(parent_slot)].action, 201);
    crate::native_world::area_damage::contact(
        &mut arena,
        None,
        parent_slot,
        u16::MAX,
        attacker_slot,
    )
    .unwrap();
    let families = family_with_cymmerian(parent_slot);
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    // The old negative-life oracle omitted sub28C30's wrapper. The source
    // selects204 and then restores0, retaining the next frozen living root.
    assert_eq!(arena.entities[usize::from(parent_slot)].life, 0);
    assert_eq!(word(&arena.entities[usize::from(parent_slot)], 12), 0);
    assert!(arena.rebuilt_class5_model_presence()[usize::from(SOURCE_MODEL)]);
    assert!(
        arena
            .imported_creature_runtime_identity(parent_slot, &std::collections::BTreeMap::new())
            .unwrap()
            .living
    );
    assert_eq!(arena.entities[usize::from(parent_slot)].action, 204);
    let parent_id = arena.entities[usize::from(parent_slot)].source_id;
    let mut exhausted = arena.clone();
    let mut exhausted_runtime = runtime.clone();
    let mut exhausted_terrain = terrain.clone();
    exhausted.finish_source_group();
    while exhausted.economy.free_slot_count() != 0 {
        exhausted
            .allocate_source_slot_with_terrain(&mut exhausted_terrain)
            .expect("consume the actual source free stack");
    }
    let parent_rng = exhausted.entities[usize::from(parent_slot)].entity_rng;
    let exhausted_population = exhausted.economy.source_creature_population;
    assert!(exhausted.entities[usize::from(parent_slot)].mana > 0);
    let families = family_with_cymmerian(parent_slot);
    mc2::step_row(
        &mut exhausted,
        &mut exhausted_runtime,
        &mut exhausted_terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(exhausted.entities[usize::from(parent_slot)].action, 205);
    assert_eq!(
        exhausted.economy.source_creature_population,
        exhausted_population + 3,
        "source global population increases by three even when split allocation fails"
    );
    assert_eq!(
        exhausted.entities[usize::from(parent_slot)].entity_rng,
        crate::native_world::level2_next_rng16(parent_rng),
        "the failed mana allocation still consumes the source RNG draw"
    );
    assert!(
        !exhausted.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                parent_entity_index, ..
            } if *parent_entity_index == parent_slot
        )),
        "failed source allocation does not publish a split burst"
    );

    // Two free source slots exercise the partially allocated branch: the
    // third NewEvent fails, so the source does not award its remainder.
    let mut partial = arena.clone();
    let mut partial_runtime = runtime.clone();
    let mut partial_terrain = terrain.clone();
    partial.finish_source_group();
    for row in &mut partial.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    assert!(partial.economy.typed_replacement_candidate().is_none());
    partial.rebuild_source_free_arena();
    while partial.economy.free_slot_count() > 2 {
        partial
            .allocate_source_slot_with_terrain(&mut partial_terrain)
            .expect("reserve the source free stack down to two slots");
    }
    partial.entities[usize::from(parent_slot)].set_mana(100);
    let population_before_partial = partial.economy.source_creature_population;
    mc2::step_row(
        &mut partial,
        &mut partial_runtime,
        &mut partial_terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    let partial_children = partial
        .entities
        .iter()
        .filter_map(|row| {
            let identity = partial.imported_creature_runtime_identity(
                row.index,
                &std::collections::BTreeMap::new(),
            )?;
            (identity.origin
                == (crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion {
                    producer_slot: parent_slot,
                }))
            .then_some(row.mana)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        partial_children,
        [33, 33],
        "a failed final attempt drops the source remainder"
    );
    assert_eq!(
        partial.economy.source_creature_population,
        population_before_partial + 3,
        "source population increments by three despite partial allocation"
    );

    // Exercise NewEvent's frozen fallback with this actual source address.
    // Model 25 does not normally set the replaceable flag; this is a bounded
    // allocator control, not a claim that an ordinary death takes this path.
    let mut replaced = arena.clone();
    let mut replaced_runtime = runtime.clone();
    let mut replaced_terrain = terrain.clone();
    for row in &mut replaced.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    assert!(replaced.economy.typed_replacement_candidate().is_none());
    let parent = &mut replaced.entities[usize::from(parent_slot)];
    parent.set_control_flags(parent.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    let original = replaced.handle(parent_slot).unwrap();
    replaced.rebuild_source_free_arena();
    while replaced.economy.free_slot_count() != 0 {
        replaced
            .allocate_source_slot_with_terrain(&mut replaced_terrain)
            .expect("reserve the real free stack before the frozen fallback");
    }
    mc2::step_row(
        &mut replaced,
        &mut replaced_runtime,
        &mut replaced_terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert!(replaced.resolve(original).is_none());
    let sphere = replaced
        .economy
        .spheres()
        .iter()
        .find(|sphere| sphere.active && sphere.source_entity_index == parent_slot)
        .expect("mana conversion replaced the dying source at its actual address");
    assert_eq!(
        sphere.action, 205,
        "the split tail writes the current source address"
    );
    assert_eq!(sphere.owner_entity_index, 0);
    assert!(
        !replaced.economy.events().iter().any(|event| matches!(
            event,
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                parent_entity_index, ..
            } if *parent_entity_index == parent_slot
        )),
        "the exhausted frozen list cannot manufacture a later burst"
    );
    let existing = arena
        .entities
        .iter()
        .filter(|row| row.active)
        .map(|row| row.index)
        .collect::<std::collections::BTreeSet<_>>();
    let families = family_with_cymmerian(parent_slot);

    let population = arena.economy.source_creature_population;
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(
        arena.economy.source_creature_population,
        population + 3,
        "large source death increases the global population by three"
    );

    let children = arena
        .entities
        .iter()
        .filter(|row| {
            row.active
                && row.class == SOURCE_CLASS
                && row.model == SOURCE_MODEL
                && row.parameter_relation == Some(SPLIT_DESCRIPTOR)
                && !existing.contains(&row.index)
        })
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert_eq!(children.len(), usize::from(RULES.split_count));
    let burst_slot = arena
        .economy
        .events()
        .iter()
        .find_map(|event| match event {
            crate::native_world::Level2EconomyEvent::DeathBurstSpawned {
                source_entity_index,
                parent_entity_index,
                id_entity_index,
            } if *parent_entity_index == parent_slot && *id_entity_index == parent_id => {
                Some(*source_entity_index)
            }
            _ => None,
        })
        .expect("the actual split publishes its source death burst");
    let burst = &arena.entities[usize::from(burst_slot)];
    assert_eq!((burst.class, burst.model, burst.action), (10, 1, 1));
    assert_eq!(burst.source_id, parent_id);
    assert_eq!(arena.entities[usize::from(parent_slot)].action, 205);
    let denominator = arena.economy.source_creature_population;
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        parent_slot,
        &[],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(
        arena.economy.source_creature_population, denominator,
        "source28EC0 hides the large body without reducing the global denominator"
    );
    assert!(arena.entities[usize::from(parent_slot)].drawing_disabled());
    for child_slot in children {
        let identity = arena
            .imported_creature_runtime_identity(child_slot, &std::collections::BTreeMap::new())
            .expect("split child has generation-bound Cymmerian identity");
        assert_eq!(identity.handle.source_slot, child_slot);
        assert_eq!(
            identity.origin,
            crate::campaign::source_arena::CreatureRootOrigin::CreatureConversion {
                producer_slot: parent_slot
            }
        );
    }
}

#[test]
fn special_castle_contact_commits_damage_inside_cymmerian_adapter() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, _owner_id, position) =
        summoned_source_parts();
    let built_castle_slot = arena
        .economy
        .build_castle(position, parent_slot, &terrain)
        .expect("source Castle constructor for Mission18 owner");
    let castle = arena
        .economy
        .castle()
        .expect("source Castle remains live after construction");
    assert_eq!(castle.source_entity_index, built_castle_slot);
    assert_eq!((castle.source_class, castle.source_model), (3, 2));
    let player_slot = castle.owner_entity_index;
    let player = arena.entities[usize::from(player_slot)].clone();
    let slot = mc2::allocate_authored(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        castle.position,
        RULES.max_life,
        None,
        0,
    )
    .unwrap()
    .unwrap();
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_source_action(200);
        row.record[74] = 6;
        row.record[154..156].copy_from_slice(&player_slot.to_le_bytes());
    }
    let families = family_with_cymmerian(slot);
    let previous_damage = castle.pending_damage;
    assert_eq!(
        castle.pending_attacker_entity_index, 0,
        "Mission18 Castle contact mailbox starts clear"
    );
    let _ = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    let after = arena.economy.castle().expect("Castle remains live");
    assert_eq!(after.pending_damage, previous_damage + 60);
    assert_eq!(
        after.pending_attacker_entity_index,
        arena.entities[usize::from(slot)].source_id
    );
    assert!(player.life > 0);

    let latched_damage = after.pending_damage;
    {
        let row = &mut arena.entities[usize::from(slot)];
        row.set_position(castle.position);
        row.record[74] = 7;
    }
    let _ = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player_slot],
        &families,
        crate::native_world::source_target::Context::default(),
    )
    .unwrap();
    assert_eq!(
        arena.economy.castle().unwrap().pending_damage,
        latched_damage,
        "a latched Castle attacker replaces rather than adds the Cymmerian hit"
    );
    assert!(
        arena
            .economy
            .apply_source_castle_contact(castle.source_entity_index, 0, 9)
    );
    assert_eq!(arena.economy.castle().unwrap().pending_damage, 9);
    assert!(
        arena
            .economy
            .apply_source_castle_contact(castle.source_entity_index, 0, 5)
    );
    let zero_owner = arena.economy.castle().unwrap();
    assert_eq!(
        (
            zero_owner.pending_damage,
            zero_owner.pending_attacker_entity_index
        ),
        (14, 0)
    );
}

#[test]
fn army_ring_reports_none_when_the_shared_source_allocator_is_exhausted() {
    let (mut arena, mut runtime, mut terrain, particles, parent_slot, owner_id, position) =
        summoned_source_parts();
    while arena.allocate_source_slot().is_some() {}
    let counter_at = crate::native_world::RUNTIME_BYTES + usize::from(SOURCE_MODEL);
    let counter = runtime.bytes[counter_at];

    let result = mc2::allocate_summoned(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        position,
        parent_slot,
        owner_id,
        parent_slot,
    )
    .unwrap();

    assert_eq!(result, None);
    assert_eq!(runtime.bytes[counter_at], counter);
}
