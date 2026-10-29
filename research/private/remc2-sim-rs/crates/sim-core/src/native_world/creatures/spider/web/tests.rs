use super::*;

fn spawn() -> (
    SourceArena,
    SessionTerrain,
    Vec<ParticleParameter>,
    u16,
    u16,
) {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) =
        super::super::tests::mission_four();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let spider = super::super::mc2::allocate_authored(
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
    .unwrap();
    put(&mut arena.entities[usize::from(spider)], 154, player);
    (arena, terrain, particles, spider, player)
}

#[test]
fn mission_four_web_constructor_preserves_source_seed_pose_and_retained_particle() {
    let (mut arena, mut terrain, particles, spider, player) = spawn();
    let parent = arena.entities[usize::from(spider)].clone();
    let target = arena.entities[usize::from(player)].clone();
    let global_rng = arena.economy.global_rng;
    let slot = launch(&mut arena, &mut terrain, &particles, spider, player)
        .unwrap()
        .unwrap();
    let row = &arena.entities[usize::from(slot)];
    assert_eq!((row.class, row.model, row.action), (9, 21, 22));
    assert_eq!(
        (
            row.life,
            row.max_life,
            row.actual_speed,
            row.minimum_speed()
        ),
        (19, 19, 394, 394)
    );
    assert_eq!(row.control_flags, 4);
    assert_eq!(row.parameter_relation, Some(65));
    assert_eq!(row.entity_rng, slot.wrapping_add(global_rng as u16));
    assert_eq!(arena.economy.global_rng, global_rng);
    assert_eq!(
        (row.record[71], row.record[72], row.subspell_index),
        (10, 66, 780)
    );
    assert_eq!((row.source_id, word(row, 154)), (spider, player));
    assert_eq!((row.record[69], row.record[70]), (3, 0));
    assert_eq!(
        (row.yaw, row.pitch),
        projectile_guidance_angles(parent.position, target.position)
    );
    assert_eq!(
        row.position,
        Position {
            z: parent.position.z.wrapping_add(128),
            ..parent.position
        }
    );
    assert_eq!(
        (row.particle_parameter_index, row.record[97]),
        (Some(319), particles[319].animation_limit().unwrap())
    );
    assert_eq!(
        (row.array_pitch(), row.array_roll(), row.array_fov()),
        (256, 256, 512)
    );
    assert_eq!(row.array_yaw(), (particles[319].rotation_speed / 2) as i16);
    assert!(
        arena
            .economy
            .map_chains
            .entity_chain(
                row.position.x.to_be_bytes()[0],
                row.position.y.to_be_bytes()[0],
            )
            .contains(&slot)
    );
}

fn aim_at_player(arena: &mut SourceArena, terrain: &mut SessionTerrain, slot: u16, player: u16) {
    let body = &arena.entities[usize::from(player)];
    let mut position = centre(body.position, body.model, body.array_yaw());
    move_entity(&mut position, 0, 0, -RULES.web_speed);
    let row = &mut arena.entities[usize::from(slot)];
    row.set_position(position);
    row.set_yaw(0);
    row.set_pitch(0);
    put(row, 154, player);
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false).unwrap();
}

#[test]
fn mission_four_projectile_collision_produces_existing_web_status_and_contact() {
    let (mut arena, mut terrain, particles, spider, player) = spawn();
    let slot = launch(&mut arena, &mut terrain, &particles, spider, player)
        .unwrap()
        .unwrap();
    aim_at_player(&mut arena, &mut terrain, slot, player);
    let phase = arena.entities[usize::from(slot)].phase;
    let impact = step(
        &mut arena,
        &mut terrain,
        &particles,
        slot,
        Default::default(),
    )
    .unwrap()
    .impact_slot
    .unwrap();
    assert!(arena.entities[usize::from(slot)].drawing_disabled());
    assert_eq!(
        arena.entities[usize::from(slot)].phase,
        phase,
        "shared tail owns phase"
    );
    let body = arena.entities[usize::from(player)].clone();
    let row = &mut arena.entities[usize::from(impact)];
    assert_eq!(
        (row.class, row.model, row.action, row.subspell_index),
        (10, 66, 71, 780)
    );
    assert_eq!((row.source_id, word(row, 154)), (spider, player));
    let mut flight = crate::FlightState {
        entity_index: player,
        position: body.position,
        ..Default::default()
    };
    let definition =
        sim_definition::adapter::mc2_field_definition(FieldKind::Web, TilePosition { x: 0, y: 0 });
    let first =
        crate::world::fields::status::impact(row, definition, Some((&body, &mut flight)), 0)
            .unwrap();
    assert_eq!(
        (
            flight.immobilize_counter,
            flight.immobilize_subcounter,
            flight.recoil_speed
        ),
        (1, 10, -80)
    );
    assert_eq!(first.contacts.len(), 1);
    assert_eq!(
        (
            first.contacts[0].target,
            first.contacts[0].owner,
            first.contacts[0].payload
        ),
        (player, spider, 780)
    );
    assert_eq!(first.sounds.len(), 1);
    assert_eq!(first.sounds[0].0, player);
    assert!((54..=57).contains(&first.sounds[0].1));
    assert!(row.drawing_disabled());
    let rng = row.entity_rng;
    let refresh =
        crate::world::fields::status::impact(row, definition, Some((&body, &mut flight)), 0)
            .unwrap();
    assert!(refresh.contacts.is_empty() && refresh.sounds.is_empty());
    assert_eq!(row.entity_rng, rng);
}

#[test]
fn exhausted_impact_allocation_retains_projectile_for_next_visit() {
    let (mut arena, mut terrain, particles, spider, player) = spawn();
    let slot = launch(&mut arena, &mut terrain, &particles, spider, player)
        .unwrap()
        .unwrap();
    aim_at_player(&mut arena, &mut terrain, slot, player);
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
    assert_eq!(
        step(
            &mut arena,
            &mut terrain,
            &particles,
            slot,
            Default::default()
        )
        .unwrap()
        .impact_slot,
        None
    );
    assert!(!arena.entities[usize::from(slot)].drawing_disabled());
    assert_eq!(
        (
            arena.entities[usize::from(slot)].class,
            arena.entities[usize::from(slot)].model
        ),
        (9, 21)
    );
}

#[test]
fn full_arena_launch_reads_the_replaced_parent_position_after_new_event() {
    let (mut arena, mut terrain, particles, spider, player) = spawn();
    let old = arena.handle(spider).unwrap();
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    let row = &mut arena.entities[usize::from(spider)];
    row.set_control_flags(row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
    let slot = launch(&mut arena, &mut terrain, &particles, spider, player)
        .unwrap()
        .unwrap();
    assert_eq!(slot, spider);
    assert!(arena.resolve(old).is_none());
    let row = &arena.entities[usize::from(slot)];
    assert_eq!(row.position, Position { x: 0, y: 0, z: 128 });
    assert_eq!(
        word(row, 154),
        0,
        "NewEvent cleared the parent's target relation"
    );
    assert!(arena.economy.map_chains.entity_chain(0, 0).contains(&slot));
}

#[test]
fn source_water_contact_hides_even_when_splash_allocation_fails() {
    for exhausted in [false, true] {
        let (mut arena, mut terrain, particles, spider, player) = spawn();
        let slot = launch(&mut arena, &mut terrain, &particles, spider, player)
            .unwrap()
            .unwrap();
        let position = arena.entities[usize::from(slot)].position;
        let tile = crate::native_world::terrain_index(
            position.x.to_be_bytes()[0],
            position.y.to_be_bytes()[0],
        );
        terrain.planes.terrain_types[tile] = 0;
        let ground = terrain.ground_height(position);
        let row = &mut arena.entities[usize::from(slot)];
        row.set_position(Position {
            z: ground.wrapping_sub(1),
            ..position
        });
        row.set_actual_speed(0);
        put(row, 136, 0);
        put(row, 154, 0);
        if exhausted {
            for row in &mut arena.entities {
                row.set_control_flags(
                    row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
                );
            }
            arena.economy.exhaust_new_event_capacity_for_test();
            arena.rebuild_source_free_arena();
        }
        let outcome = step(
            &mut arena,
            &mut terrain,
            &particles,
            slot,
            Default::default(),
        )
        .unwrap();
        assert_eq!(outcome.impact_slot, None);
        assert!(arena.entities[usize::from(slot)].drawing_disabled());
        assert_eq!(arena.entities[usize::from(slot)].position.z, ground);
        assert_eq!(
            arena
                .entities
                .iter()
                .filter(|row| row.active && (row.class, row.model) == (10, 5))
                .count(),
            usize::from(!exhausted)
        );
    }
}

#[test]
fn mission_four_ordinary_scheduler_carries_web_to_actual_player_damage_and_reclaim() {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    );
    let mut session = crate::campaign::CampaignSession::open(
        root.join("level4"),
        crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
    )
    .unwrap()
    .simulation()
    .clone();
    let (_, _, _, _, _, feature) = super::super::tests::mission_four();
    let particles = session.session.definition.presentation.particles.clone();
    let (spider, player, projectile, life) = {
        let live = &mut session.session.state.live;
        let player = live.core.state.entity_index;
        assert_eq!(live.player_protection_ticks, 100);
        // Exercise damage after the source's initial protection interval. The
        // protected branch is checked separately below on the same producer.
        live.player_protection_ticks = 0;
        let position =
            feature.world_position(live.terrain.ground_height(feature.world_position(0)));
        let spider = super::super::mc2::allocate_authored(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            position,
            RULES.max_life,
            None,
            feature.index,
        )
        .unwrap()
        .unwrap();
        // Pin a source attack cadence after the actual entry. This is a
        // diagnostic encounter, not evidence of earning authored group6.
        let body = live.economy.entities[usize::from(player)].clone();
        let mut position = body.position;
        move_entity(&mut position, 0, 0, -RULES.web_speed);
        position.z = body.position.z.wrapping_sub(RULES.web_lift);
        let row = &mut live.economy.entities[usize::from(spider)];
        row.set_position(position);
        row.set_source_action(162);
        row.set_phase(35);
        row.set_control_flags(row.control_flags | 0x800);
        put(row, 154, player);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            spider,
            position,
            true,
            false,
        )
        .unwrap();
        let families = live.economy.rebuilt_class5_model_slots();
        let effect = super::super::mc2::step_row(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            spider,
            &[player],
            &families,
            Some((player, 0)),
        )
        .unwrap();
        assert_eq!(effect.web_launched_target, Some(player));
        let projectile = live
            .economy
            .entities
            .iter()
            .find(|row| row.active && row.class == 9 && row.model == 21 && row.source_id == spider)
            .unwrap()
            .index;
        aim_at_player(&mut live.economy, &mut live.terrain, projectile, player);
        // Prevent a second launch during this bounded consumer check.
        live.economy.entities[usize::from(spider)].set_source_action(160);
        (spider, player, projectile, live.player_life)
    };
    let evidence = session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(!evidence.is_empty(), "{:?}", session.dispatch_events());
    let live = &session.session.state.live;
    let impacts = live
        .economy
        .entities
        .iter()
        .filter(|row| row.active && row.class == 10 && row.model == 66 && word(row, 154) == player)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    assert_eq!(
        impacts.len(),
        1,
        "flight={:?} evidence={evidence:?} web_rows={:?}",
        live.core.state.position,
        live.economy
            .entities
            .iter()
            .filter(|row| row.active && matches!((row.class, row.model), (9, 21) | (10, 66)))
            .map(|row| (
                row.index,
                row.class,
                row.model,
                row.position,
                word(row, 154),
                row.phase,
                row.control_flags
            ))
            .collect::<Vec<_>>(),
    );
    let impact = impacts[0];
    // Initial ordinary cleanup frees a lower source address. A birth behind
    // the numeric cursor must wait until the following pass, without a
    // synthetic immediate call into the status handler.
    assert!(impact < projectile && impact < player);
    assert_eq!(live.core.state.immobilize_counter, 0);
    assert_eq!(live.player_life, life);
    assert_eq!(live.player_pending_damage, 0);
    assert!(!live.economy.entities[usize::from(impact)].drawing_disabled());
    assert_eq!(
        live.economy.entities[usize::from(impact)].phase,
        impact as u8
    );
    assert!(live.economy.entities[usize::from(projectile)].drawing_disabled());
    let evidence = session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(!evidence.is_empty(), "{:?}", session.dispatch_events());
    let live = &session.session.state.live;
    // Impact precedes the actual player, so its ordinary visit consumes the
    // contact mailbox and the first of ten immobilization ticks in this pass.
    assert_eq!(
        (
            live.core.state.immobilize_counter,
            live.core.state.immobilize_subcounter
        ),
        (1, 9)
    );
    assert_eq!(live.player_life, life - 780);
    assert_eq!(live.player_pending_attacker, 0);
    assert!(!live.economy.entities[usize::from(projectile)].active);
    assert!(live.economy.entities[usize::from(impact)].active);
    assert!(live.economy.entities[usize::from(impact)].drawing_disabled());
    assert_eq!(
        live.economy.entities[usize::from(impact)].phase,
        (impact as u8).wrapping_add(1)
    );
    let evidence = session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(!evidence.is_empty(), "{:?}", session.dispatch_events());
    assert!(!session.session.state.live.economy.entities[usize::from(impact)].active);

    // Exercise the same consumer from the ordinary authored group and the
    // numeric Spider callback. Position and cadence are controlled, but the
    // group constructor, attack, projectile and player visits are real.
    for protected in [false, true] {
        let mut scheduled = crate::campaign::CampaignSession::open(
            root.join("level4"),
            crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
        )
        .unwrap()
        .simulation()
        .clone();
        scheduled
            .activate_authored_group_prerequisite_for_test(6)
            .expect("actual Mission4 group6 constructs its actors");
        let spider = scheduled
            .creature_runtime_census()
            .roots_of_kind(crate::level_definition::CreatureKind::Spider)
            .find(|identity| {
                identity.origin
                    == crate::campaign::source_arena::CreatureRootOrigin::AuthoredRecord {
                        authored_record: 137,
                    }
            })
            .expect("group6 constructs the generation-bound record137 Spider")
            .handle
            .source_slot;
        let (player, life) = {
            let live = &mut scheduled.session.state.live;
            let player = live.core.state.entity_index;
            let body = &live.economy.entities[usize::from(player)];
            let mut position = centre(body.position, body.model, body.array_yaw());
            move_entity(&mut position, 0, 0, -RULES.web_speed);
            position.z = position.z.wrapping_sub(RULES.web_lift);
            let row = &mut live.economy.entities[usize::from(spider)];
            row.set_position(position);
            row.set_source_action(162);
            row.set_phase(35);
            row.set_control_flags(row.control_flags | 0x800);
            put(row, 154, player);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                spider,
                position,
                true,
                false,
            )
            .unwrap();
            live.player_protection_ticks = if protected { 100 } else { 0 };
            (player, live.player_life)
        };
        scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !scheduled
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "scheduled Spider attack must not roll back: {:?}",
            scheduled.dispatch_events()
        );
        let mut projectile = None;
        let mut impact = None;
        let mut immobilized = false;
        for pass in 0..13 {
            let live = &scheduled.session.state.live;
            for row in live.economy.entities.iter().filter(|row| row.active) {
                if (row.class, row.model, row.action) == (9, 21, 22) && row.source_id == spider {
                    projectile = Some((row.index, live.economy.economy.slot_generation(row.index)));
                }
                if (row.class, row.model, row.action) == (10, 66, 71)
                    && row.source_id == spider
                    && word(row, 154) == player
                {
                    impact = Some((row.index, live.economy.economy.slot_generation(row.index)));
                }
            }
            immobilized |= live.core.state.immobilize_counter != 0;
            if pass == 0 {
                scheduled.session.state.live.economy.entities[usize::from(spider)]
                    .set_source_action(160);
            }
            scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
            assert!(
                !scheduled.dispatch_events().iter().any(|event| matches!(
                    event,
                    crate::native_world::Mc2DispatchEvent::Error { .. }
                )),
                "scheduled web lifecycle must not roll back: {:?}",
                scheduled.dispatch_events()
            );
        }
        assert!(
            immobilized,
            "scheduled record137 attack reaches actual player immobilization"
        );
        assert_eq!(
            scheduled.session.state.live.player_life,
            life - if protected { 0 } else { 780 }
        );
        assert_eq!(
            (
                scheduled.session.state.live.core.state.immobilize_counter,
                scheduled
                    .session
                    .state
                    .live
                    .core
                    .state
                    .immobilize_subcounter
            ),
            (0, 0),
            "real movement upkeep expires the web"
        );
        let live = &scheduled.session.state.live;
        for (slot, generation) in [
            projectile.expect("scheduled Spider produced projectile9/21"),
            impact.expect("guided collision produced status10/66"),
        ] {
            assert!(
                !live.economy.entities[usize::from(slot)].active
                    || live.economy.economy.slot_generation(slot) != generation,
                "the source row is reclaimed or its address was reused"
            );
        }
    }
}
