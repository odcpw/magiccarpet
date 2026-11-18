use super::{status, timed, transport, wind};
use crate::{
    FlightContext, FlightEffectsContext, FlightProfile, FlightState, FlightWorld, PlayerCommand,
    Position, Tile,
    level_definition::{FieldDefinition, FieldKind, TilePosition},
    native_world::EntitySnapshot,
    step_turn_with_context,
    world::terrain::cave_features::new_event,
};

struct Ground;
impl FlightWorld for Ground {
    fn ground_height(&self, _: Position) -> i16 {
        0
    }
    fn ceiling_height(&self, _: Position) -> Option<i16> {
        None
    }
    fn material_mask(&self, _: Position) -> u32 {
        0
    }
    fn angle_flags(&self, _: Tile) -> u8 {
        0
    }
}

fn player() -> (EntitySnapshot, FlightState) {
    let position = Position {
        x: 10_000,
        y: 10_000,
        z: 2_000,
    };
    let mut body = new_event(100, 77, position);
    body.class = 3;
    body.model = 0;
    body.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
    body.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    (
        body,
        FlightState {
            entity_index: 100,
            position,
            actual_speed: 64,
            target_speed: 64,
            ..FlightState::default()
        },
    )
}

fn impact(kind: status::Kind) -> EntitySnapshot {
    let mut row = new_event(120, 13, Position { x: 0, y: 0, z: 0 });
    let (model, action) = kind.source_selector();
    row.class = 10;
    row.model = model;
    row.set_source_action(action);
    row.record[154..156].copy_from_slice(&100_u16.to_le_bytes());
    row
}

fn definition(kind: FieldKind) -> FieldDefinition {
    sim_definition::adapter::mc2_field_definition(kind, TilePosition { x: 0, y: 0 })
}

fn campaign() -> crate::native_world::Mc2Simulation {
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
        },
        std::path::PathBuf::from,
    );
    crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .unwrap()
    .simulation()
    .clone()
}

fn scheduled_impact(session: &mut crate::native_world::Mc2Simulation, kind: FieldKind) -> u16 {
    let live = &mut session.session.state.live;
    let player = live.core.state.entity_index;
    let slot = status::construct(
        &mut live.economy,
        &mut live.terrain,
        live.core.state.position,
        definition(kind),
    )
    .unwrap()
    .unwrap();
    // This is the projectile producer's real target relation, not a callback
    // invocation or a replacement player carrier.
    live.economy.entities[usize::from(slot)].record[154..156]
        .copy_from_slice(&player.to_le_bytes());
    assert!(slot > player, "pin impact after the local player visit");
    slot
}

#[test]
fn configured_timed_controller_and_bare_allocator_preserve_source_lifecycle() {
    use crate::native_world::Mc2DispatchEvent;

    let mut negative = new_event(120, 0, Position::default());
    negative.set_life(i32::MIN);
    negative.set_source_dword_10(i32::MAX);
    timed::step(&mut negative);
    assert_eq!(negative.life, i32::MAX);
    assert_eq!(negative.source_dword_10(), i32::MIN);
    assert!(negative.drawing_disabled());
    assert_eq!(
        negative.control_flags & 2,
        0,
        "negative old life returns before flag2"
    );

    let mut session = campaign();
    let properties = definition(FieldKind::TimedController);
    assert_eq!(
        properties,
        FieldDefinition::TimedController {
            lifetime: 300,
            half_width: 512,
            half_height: 512,
        }
    );
    let (blank, controller, short_lived, position) = {
        let live = &mut session.session.state.live;
        let position = live.core.state.position;
        let next_free = *live
            .economy
            .economy
            .free_source_slots()
            .last()
            .expect("configured package has a genuine free NewEvent slot");
        let expected_rng = live.economy.economy.new_event_rng(next_free);
        live.economy
            .economy
            .source_rng
            .insert(next_free, expected_rng.wrapping_add(1));
        let blank = timed::construct_blank(&mut live.economy, &mut live.terrain).unwrap();
        assert_eq!(blank, next_free);
        let blank_row = &live.economy.entities[usize::from(blank)];
        assert_eq!(
            (blank_row.class, blank_row.model, blank_row.action),
            (0, 0, 0),
            "selector35 returns the blank NewEvent without stamping a field"
        );
        assert_eq!(blank_row.source_id, blank);
        assert_eq!(blank_row.position, Position::default());
        assert_eq!((blank_row.max_life, blank_row.life), (300, 0));
        assert_eq!((blank_row.control_flags, blank_row.actual_speed), (8, 16));
        assert_eq!(blank_row.phase, blank as u8);
        assert_eq!(blank_row.particle_parameter_index, Some(0));
        assert_eq!(
            blank_row.entity_rng,
            live.economy.economy.new_event_rng(blank)
        );
        assert_eq!(
            live.economy.economy.source_rng.get(&blank),
            Some(&expected_rng),
            "NewEvent replaces the stale RNG projection for its actual generation"
        );
        assert!(!live.economy.economy.map_chains.is_linked(blank));

        let controller = timed::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            None,
            properties,
        )
        .unwrap()
        .unwrap();
        let row = &live.economy.entities[usize::from(controller)];
        assert_eq!((row.class, row.model, row.action), (10, 33, 35));
        assert_eq!((row.max_life, row.life), (300, 300));
        assert_eq!(
            (row.array_pitch(), row.array_roll(), row.array_fov()),
            (512, 512, 512)
        );
        assert_eq!(row.control_flags & 8, 0);
        assert_eq!(row.position, position);
        assert!(!live.economy.economy.map_chains.is_linked(controller));

        let short_lived = timed::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            None,
            FieldDefinition::TimedController {
                lifetime: 0,
                half_width: 512,
                half_height: 512,
            },
        )
        .unwrap()
        .unwrap();
        (
            blank,
            live.economy.handle(controller).unwrap(),
            live.economy.handle(short_lived).unwrap(),
            position,
        )
    };
    assert_ne!(blank, controller.source_slot);

    // This configured lifetime exercises the actual source scheduler, its
    // same-pass tail and the following reclaim; it is not an ordinary spawn.
    for (turn, life, hidden) in [(0, -1, false), (1, -2, true)] {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let arena = session.source_arena();
        assert_eq!(arena.handle(short_lived.source_slot), Some(short_lived));
        let row = &arena.entities[usize::from(short_lived.source_slot)];
        assert_eq!(row.life, life);
        assert_eq!(row.source_dword_10(), turn + 1);
        assert_eq!(row.control_flags & 2, 2);
        assert_eq!(row.drawing_disabled(), hidden);
        assert_eq!(row.position, position);
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
    );
    assert!(session.source_arena().resolve(short_lived).is_none());

    // Keep the real source-default300 controller running naturally. The
    // preceding three outer turns have already advanced it three times.
    for visits in 4_i32..=302 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            session
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let arena = session.source_arena();
        assert_eq!(arena.handle(controller.source_slot), Some(controller));
        let row = &arena.entities[usize::from(controller.source_slot)];
        assert_eq!(row.life, 300 - visits);
        assert_eq!(row.source_dword_10(), visits);
        assert_eq!(
            row.phase,
            (controller.source_slot as u8).wrapping_add(visits as u8)
        );
        assert_eq!(row.control_flags & 2, 2);
        assert_eq!(row.drawing_disabled(), visits == 302);
        assert!(!arena.economy.map_chains.is_linked(controller.source_slot));
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        session
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, Mc2DispatchEvent::Error { .. }))
    );
    assert!(session.source_arena().resolve(controller).is_none());

    // The real allocator failure must not stamp a fake model33 or model35.
    let live = &session.session.state.live;
    let mut arena = live.economy.clone();
    let mut terrain = live.terrain.clone();
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.finish_source_group();
    assert!(timed::construct_blank(&mut arena, &mut terrain).is_none());
    assert!(
        timed::construct(&mut arena, &mut terrain, position, None, properties)
            .unwrap()
            .is_none()
    );
}

fn campaign_step(session: &mut crate::native_world::Mc2Simulation) {
    assert!(
        !session
            .step_outer_turn(PlayerCommand::default(), 0)
            .is_empty()
    );
}

#[test]
fn skeleton_controller_schedules_owned_births_and_respects_owner_cap() {
    use super::skeleton_summon::{self, Birth};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::level_definition::CreatureKind;
    use crate::native_world::Mc2DispatchEvent;

    // Controlled source-constructor placement on retained Level2 terrain;
    // this exercises the numeric scheduler, not an earned mission event.
    let mut session = campaign();
    let (particle_index, particle) = session
        .session
        .definition
        .presentation
        .particles
        .iter()
        .copied()
        .enumerate()
        .find(|(index, particle)| *index != 41 && particle.animation_limit().is_some())
        .map(|(index, particle)| (u16::try_from(index).unwrap(), particle))
        .expect("retained presentation has another supported particle");
    let properties = FieldDefinition::SkeletonSummon {
        life: 9,
        initial_budget: 60_000,
        scheduled_budget: 96,
        max_births: 4,
        owner_cap: 2,
        spawn_radius: 0,
        particle_index,
    };
    let particles = session.session.definition.presentation.particles.clone();
    let descriptor = session.session.definition.presentation.entity_parameters[80];
    let material_mask = u32::from_le_bytes(descriptor[20..24].try_into().unwrap());
    let ground_clearance = i16::from_le_bytes(descriptor[12..14].try_into().unwrap());
    let live = &session.session.state.live;
    let candidates = (0_u16..=255).flat_map(|x| {
        (0_u16..=255).map(move |y| Position {
            x: (x << 8) + 128,
            y: (y << 8) + 128,
            z: 0,
        })
    });
    let blocked = |mut position: Position| {
        position.z = live.terrain.ground_height(position);
        live.terrain.material_mask(position) & !material_mask != 0
            || live.terrain.cave_collision_blocked(position)
            || live
                .terrain
                .ceiling_height(position)
                .is_some_and(|ceiling| {
                    i32::from(live.terrain.ground_height(position))
                        + 128
                        + i32::from(ground_clearance)
                        > i32::from(ceiling)
                })
    };
    let invalid_position = candidates.clone().find(|position| blocked(*position));
    let mut position = candidates
        .filter(|position| !blocked(*position))
        .find(|position| {
            // Prove the real constructor accepts this retained terrain point
            // on an isolated copy before using it in the scheduled case.
            let mut arena = live.economy.clone();
            let mut terrain = live.terrain.clone();
            let Some(controller) = skeleton_summon::construct(
                &mut arena,
                &mut terrain,
                *position,
                properties,
                particle,
            )
            .unwrap() else {
                return false;
            };
            crate::native_world::creatures::skeleton::allocate_from_controller(
                &mut arena,
                &mut terrain,
                &particles,
                &descriptor,
                Birth {
                    position: *position,
                    controller_slot: controller,
                    owner: live.core.state.entity_index,
                    mana: 0,
                    yaw: 0,
                },
            )
            .unwrap()
        })
        .expect("retained Level2 terrain admits the real Skeleton constructor");
    position.z = live.terrain.ground_height(position);
    let (controller, handle, owner, position, phase, tile) = {
        let live = &mut session.session.state.live;
        let owner = live.core.state.entity_index;
        let controller = skeleton_summon::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            properties,
            particle,
        )
        .unwrap()
        .expect("source controller allocation");
        let source = &mut live.economy.entities[usize::from(controller)];
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
        assert_eq!(
            (
                source.class,
                source.model,
                source.action,
                source.life,
                source.subspell_index
            ),
            (10, 36, 38, 9, 60_000)
        );
        assert_eq!(source.particle_parameter_index, Some(particle_index));
        let phase = source.phase;
        let handle = live.economy.handle(controller).unwrap();
        assert_eq!(
            skeleton_summon::definition(&live.economy, controller),
            Some(properties)
        );
        let tile = live.economy.map_chains.tile_of(controller);
        (controller, handle, owner, position, phase, tile)
    };

    // Source budget10000 divided among three births gives each Skeleton one
    // mana. The source owner totals are frozen for this pass and include the
    // new raw actors at the next membership boundary.
    let mut mana_case = session.clone();
    let mut empty_case = session.clone();
    let mut mana_properties = properties;
    let FieldDefinition::SkeletonSummon {
        scheduled_budget,
        max_births,
        owner_cap,
        ..
    } = &mut mana_properties
    else {
        unreachable!("this controller has Skeleton summon properties");
    };
    *scheduled_budget = 10_000;
    *max_births = 3;
    *owner_cap = 3;
    skeleton_summon::bind_definition(
        &mut mana_case.session.state.live.economy,
        controller,
        mana_properties,
    )
    .unwrap();
    let mut empty_properties = mana_properties;
    let FieldDefinition::SkeletonSummon { max_births, .. } = &mut empty_properties else {
        unreachable!("this controller has Skeleton summon properties");
    };
    *max_births = 0;
    skeleton_summon::bind_definition(
        &mut empty_case.session.state.live.economy,
        controller,
        empty_properties,
    )
    .unwrap();
    for candidate in [&mut mana_case, &mut empty_case] {
        candidate.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !candidate
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "mana controller must not roll back: {:?}",
            candidate.dispatch_events()
        );
    }
    let spawned = mana_case
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Skeleton)
        .filter(|identity| {
            identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
        })
        .map(|identity| identity.handle)
        .collect::<Vec<_>>();
    assert_eq!(spawned.len(), 3);
    assert!(spawned.iter().all(|handle| {
        mana_case.session.state.live.economy.entities[usize::from(handle.source_slot)].mana == 1
    }));
    let totals = |candidate: &crate::native_world::Mc2Simulation| {
        let economy = &candidate.session.state.live.economy.economy;
        (
            economy.owner_mana_global(),
            economy.owner_max_mana_capacity(owner),
            candidate
                .session
                .state
                .live
                .shared_spells
                .player_entity()
                .max_mana,
        )
    };
    assert_eq!(
        totals(&mana_case),
        totals(&empty_case),
        "new births do not alter the frozen source totals in the same pass"
    );
    for candidate in [&mut mana_case, &mut empty_case] {
        candidate.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !candidate
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "mana membership boundary must not roll back: {:?}",
            candidate.dispatch_events()
        );
    }
    let (global, capacity, player_max) = totals(&mana_case);
    let (empty_global, empty_capacity, empty_player_max) = totals(&empty_case);
    assert_eq!(global, empty_global + 3);
    assert_eq!(capacity, empty_capacity + 3);
    assert_eq!(player_max, empty_player_max + 3);

    // sub_36D50 uses those capacities to choose the owner of merged spheres.
    // Give a real rival an identical threshold in both controlled branches:
    // only the three newly counted Skeleton mana distinguish the outcome.
    let rival = mana_case
        .players()
        .iter()
        .map(|player| player.entity_index)
        .find(|&slot| {
            slot != 0
                && slot != owner
                && mana_case
                    .session
                    .state
                    .live
                    .economy
                    .entities
                    .get(usize::from(slot))
                    .is_some_and(|row| row.active && row.class == 3)
        })
        .expect("retained Level2 has another active player");
    let arbitrate = |mut candidate: crate::native_world::Mc2Simulation| {
        let live = &mut candidate.session.state.live;
        let economy = &mut live.economy.economy;
        economy.set_owner_max_mana(rival, empty_capacity.checked_add(1).unwrap());
        let rng = live.economy.entities[usize::from(controller)].entity_rng;
        let target = economy
            .spawn_mana_sphere_from_death(controller, position, 0, rng, 1, rival, &live.terrain)
            .expect("rival source sphere allocation") as u16;
        let source = economy
            .spawn_mana_sphere_from_death(controller, position, 0, rng, 1, owner, &live.terrain)
            .expect("player source sphere allocation") as u16;
        assert!(economy.merge_mana_spheres(target, source));
        economy
            .spheres()
            .iter()
            .find(|sphere| sphere.active && sphere.source_entity_index == target)
            .expect("merged sphere remains live")
            .player_entity_index
    };
    assert_eq!(arbitrate(empty_case.clone()), rival);
    assert_eq!(arbitrate(mana_case.clone()), owner);

    // A genuine lethal Fireball mailbox runs the Skeleton's own death
    // callback. Its one mana becomes an owned sphere; it must not disappear
    // or remain duplicated in the reclaimed actor.
    let mut dying = mana_case.clone();
    let previous_spheres = dying
        .session
        .state
        .live
        .economy
        .economy
        .spheres()
        .iter()
        .filter(|sphere| sphere.active)
        .map(|sphere| {
            (
                sphere.source_entity_index,
                dying
                    .session
                    .state
                    .live
                    .economy
                    .economy
                    .slot_generation(sphere.source_entity_index),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    let target = spawned[0];
    let countdown = dying.session.state.live.economy.entities[usize::from(target.source_slot)]
        .source_dword_10()
        .max(0) as usize;
    dying
        .stage_imported_skeleton_fireball_damage_for_test(target.source_slot, rival, 1_500)
        .expect("the real Fireball damage pulse reaches the summoned Skeleton");
    let mut reclaimed = false;
    // Countdown action72 precedes the damage/death actions; death action77
    // runs on its source eight-phase cadence, then reclaim occurs next pass.
    for _ in 0..(countdown + 12) {
        dying.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !dying
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "Skeleton death must not roll back: {:?}",
            dying.dispatch_events()
        );
        let arena = &dying.session.state.live.economy;
        if !arena.entities[usize::from(target.source_slot)].active
            || arena.economy.slot_generation(target.source_slot) != target.generation
        {
            reclaimed = true;
            break;
        }
    }
    assert!(reclaimed, "lethally hit summoned Skeleton is reclaimed");
    let arena = &dying.session.state.live.economy;
    let sphere_mana = arena
        .economy
        .spheres()
        .iter()
        .filter(|sphere| {
            sphere.active
                && sphere.player_entity_index == owner
                && !previous_spheres.contains(&(
                    sphere.source_entity_index,
                    arena.economy.slot_generation(sphere.source_entity_index),
                ))
        })
        .map(|sphere| sphere.mana)
        .sum::<i32>();
    let remaining_mana = spawned
        .iter()
        .filter_map(|handle| {
            let row = &arena.entities[usize::from(handle.source_slot)];
            (row.active
                && !row.drawing_disabled()
                && arena.economy.slot_generation(handle.source_slot) == handle.generation)
                .then_some(row.mana)
        })
        .sum::<i32>();
    assert!(
        sphere_mana > 0,
        "actual Skeleton death creates an owned mana sphere"
    );
    assert_eq!(
        remaining_mana + sphere_mana,
        3,
        "death transfers mana without duplication"
    );

    // Isolate reclaim of the existing raw rows without running their death
    // callback: actual deaths transfer positive mana to new spheres instead.
    let mut removed = mana_case.clone();
    let mut retained = mana_case.clone();
    for handle in &spawned {
        let arena = &mut removed.session.state.live.economy;
        assert_eq!(
            arena.economy.slot_generation(handle.source_slot),
            handle.generation
        );
        arena.entities[usize::from(handle.source_slot)].disable_drawing();
    }
    assert_eq!(totals(&removed), totals(&retained));
    for candidate in [&mut removed, &mut retained] {
        candidate.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !candidate
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "raw Skeleton reclaim must not roll back: {:?}",
            candidate.dispatch_events()
        );
    }
    let (removed_global, removed_capacity, removed_player_max) = totals(&removed);
    let (retained_global, retained_capacity, retained_player_max) = totals(&retained);
    assert_eq!(removed_global + 3, retained_global);
    assert_eq!(removed_capacity + 3, retained_capacity);
    assert_eq!(removed_player_max + 3, retained_player_max);
    for handle in &spawned {
        let arena = &removed.session.state.live.economy;
        assert!(
            !arena.entities[usize::from(handle.source_slot)].active
                || arena.economy.slot_generation(handle.source_slot) != handle.generation
        );
    }

    if let Some(invalid_position) = invalid_position {
        let live = &session.session.state.live;
        let mut arena = live.economy.clone();
        let mut terrain = live.terrain.clone();
        let free_before = arena.economy.free_slot_count();
        let terrain_before = terrain.deterministic_hash();
        let state_before = arena.reset_hash_with_terrain(&terrain);
        assert!(
            !crate::native_world::creatures::skeleton::allocate_from_controller(
                &mut arena,
                &mut terrain,
                &particles,
                &descriptor,
                Birth {
                    position: invalid_position,
                    controller_slot: controller,
                    owner,
                    mana: 0,
                    yaw: 0,
                },
            )
            .unwrap(),
            "real constructor rejects an inadmissible retained terrain point"
        );
        assert_eq!(arena.economy.free_slot_count(), free_before);
        assert_eq!(terrain.deterministic_hash(), terrain_before);
        assert_ne!(arena.reset_hash_with_terrain(&terrain), state_before);
    }
    // The same pre-visit controller with a lower birth limit must produce
    // one actor even though its owner cap allows two.
    let mut limited = session.clone();
    let mut limited_properties = properties;
    let FieldDefinition::SkeletonSummon { max_births, .. } = &mut limited_properties else {
        unreachable!("this controller has Skeleton summon properties");
    };
    *max_births = 1;
    skeleton_summon::bind_definition(
        &mut limited.session.state.live.economy,
        controller,
        limited_properties,
    )
    .unwrap();
    limited.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !limited
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "limited controller must not roll back: {:?}",
        limited.dispatch_events()
    );
    assert_eq!(
        limited
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Skeleton)
            .filter(|identity| {
                identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
            })
            .count(),
        1,
        "the configured birth limit is independent of the owner cap; active raw model9 rows: {:?}",
        limited
            .session
            .state
            .live
            .economy
            .entities
            .iter()
            .filter(|row| row.active && (row.class, row.model) == (5, 9))
            .map(|row| (row.index, row.action, row.source_id, row.position))
            .collect::<Vec<_>>()
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "scheduled source36 must not roll back: {:?}",
        session.dispatch_events()
    );
    let summoned = {
        let live = &session.session.state.live;
        let root = &live.economy.entities[usize::from(controller)];
        assert!(root.active && root.drawing_disabled());
        assert_eq!(root.phase, phase.wrapping_add(1));
        assert_eq!(root.subspell_index, 96);
        assert_eq!(
            skeleton_summon::definition(&live.economy, controller),
            Some(properties)
        );
        assert!(live.economy.map_chains.is_linked(controller));
        let births = session
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Skeleton)
            .filter(|identity| {
                identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
            })
            .collect::<Vec<_>>();
        assert!(
            births.len() == 2,
            "controller produces real Skeletons; active raw model9 rows: {:?}, free slots: {}",
            live.economy
                .entities
                .iter()
                .filter(|row| row.active && (row.class, row.model) == (5, 9))
                .map(|row| (row.index, row.action, row.source_id, row.position))
                .collect::<Vec<_>>(),
            live.economy.economy.free_slot_count(),
        );
        let presentations = session.campaign_entity_presentations(0).unwrap();
        assert!(
            !presentations
                .iter()
                .any(|row| row.entity_index == controller),
            "the hidden controller is absent from the presentation consumer"
        );
        for birth in &births {
            let actor = &live.economy.entities[usize::from(birth.handle.source_slot)];
            assert_eq!(
                (
                    actor.class,
                    actor.model,
                    actor.source_id,
                    actor.source_player_entity_index()
                ),
                (5, 9, owner, owner)
            );
            assert_eq!(actor.max_life, 1_000);
            assert_eq!(
                (actor.position.x, actor.position.y),
                (position.x, position.y)
            );
            let presentation = presentations
                .iter()
                .find(|row| row.entity_index == birth.handle.source_slot)
                .expect("summoned Skeleton reaches the real presentation consumer");
            assert_eq!(
                (presentation.source_id, presentation.render_parameter_index),
                (owner, 220)
            );
        }
        births
            .iter()
            .map(|birth| {
                let actor = &live.economy.entities[usize::from(birth.handle.source_slot)];
                (birth.handle, actor.phase)
            })
            .collect::<Vec<_>>()
    };
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "controller reclaim must not roll back: {:?}",
        session.dispatch_events()
    );
    {
        let live = &session.session.state.live;
        assert!(
            !live.economy.entities[usize::from(controller)].active
                || live.economy.economy.slot_generation(controller) != handle.generation
        );
        assert_eq!(skeleton_summon::definition(&live.economy, controller), None);
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile)
        );
        assert!(
            session
                .creature_runtime_census()
                .roots_of_kind(CreatureKind::Skeleton)
                .any(|identity| identity.origin
                    == CreatureRootOrigin::FieldSummon { controller: handle }),
            "Skeletons do not inherit the controller's lifetime"
        );
        assert!(
            summoned.iter().any(|(handle, phase)| {
                let actor = &live.economy.entities[usize::from(handle.source_slot)];
                live.economy.economy.slot_generation(handle.source_slot) == handle.generation
                    && actor.active
                    && actor.phase == phase.wrapping_add(1)
            }),
            "a summoned Skeleton reaches its own scheduled actor callback"
        );
    }

    // The near-identical case populates the source-frozen owner list using
    // the real constructor. At the source cap, the same controller hides
    // without adding another owned Skeleton.
    let mut capped = campaign();
    let properties = definition(FieldKind::SkeletonSummon);
    let FieldDefinition::SkeletonSummon { particle_index, .. } = properties else {
        unreachable!("source adapter returns the Skeleton controller definition");
    };
    let particle = capped.session.definition.presentation.particles[usize::from(particle_index)];
    let (controller, handle, owner) = {
        let live = &mut capped.session.state.live;
        let owner = live.core.state.entity_index;
        let controller = skeleton_summon::construct(
            &mut live.economy,
            &mut live.terrain,
            position,
            properties,
            particle,
        )
        .unwrap()
        .expect("source controller allocation");
        let source = &mut live.economy.entities[usize::from(controller)];
        source.source_id = owner;
        source.record[30..32].copy_from_slice(&owner.to_le_bytes());
        let handle = live.economy.handle(controller).unwrap();
        for _ in 0..64 {
            assert!(
                crate::native_world::creatures::skeleton::allocate_from_controller(
                    &mut live.economy,
                    &mut live.terrain,
                    &particles,
                    &descriptor,
                    Birth {
                        position,
                        controller_slot: controller,
                        owner,
                        mana: 0,
                        yaw: 0
                    },
                )
                .unwrap(),
                "real retained terrain admits a Skeleton constructor"
            );
        }
        (controller, handle, owner)
    };
    let before = capped
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Skeleton)
        .filter(|identity| {
            identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
        })
        .count();
    assert_eq!(before, 64);
    capped.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !capped
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
        "capped controller must not roll back: {:?}",
        capped.dispatch_events()
    );
    let live = &capped.session.state.live;
    assert!(live.economy.entities[usize::from(controller)].drawing_disabled());
    let after = capped
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Skeleton)
        .filter(|identity| {
            identity.origin == CreatureRootOrigin::FieldSummon { controller: handle }
        })
        .collect::<Vec<_>>();
    assert_eq!(after.len(), before, "owner cap prevents all new births");
    assert!(after.iter().all(|identity| {
        live.economy.entities[usize::from(identity.handle.source_slot)].source_player_entity_index()
            == owner
    }));

    // A frozen model-9 list stores addresses, not lifetime handles. If an
    // address is reused, the owner-cap scan reads the current occupant's
    // word94, even when the retained raw record still contains an old value.
    use crate::native_world::creatures::skeleton::current_source_player_entity_index;
    let raw_slot = after[0].handle.source_slot;
    assert_eq!(
        current_source_player_entity_index(&live.economy, raw_slot).unwrap(),
        Some(owner)
    );
    let live = &mut capped.session.state.live;
    let stale_owner = owner.wrapping_add(1);
    let sphere = live
        .economy
        .economy
        .spawn_mana_sphere_from_death(999, position, 0, 0x1234, 500, owner, &live.terrain)
        .expect("death constructor allocates a typed sphere") as u16;
    live.economy.entities[usize::from(sphere)].record[152..154]
        .copy_from_slice(&stale_owner.to_le_bytes());
    assert_eq!(
        current_source_player_entity_index(&live.economy, sphere).unwrap(),
        Some(owner),
        "current sphere ownership wins over stale raw bytes"
    );
    let creature = live
        .economy
        .economy
        .allocate_authored_class5(999, 9, position, &live.terrain)
        .expect("ordinary constructor allocates a typed Skeleton");
    let creature_slot = creature.source_entity_index;
    let typed_owner = live.economy.economy.source_owner(creature_slot);
    live.economy.entities[usize::from(creature_slot)].record[152..154]
        .copy_from_slice(&typed_owner.wrapping_add(1).to_le_bytes());
    assert_eq!(
        current_source_player_entity_index(&live.economy, creature_slot).unwrap(),
        Some(typed_owner),
        "current typed creature ownership wins over stale raw bytes"
    );
}

#[test]
fn wind_projection_cleanup_does_not_retire_a_reused_generation() {
    let mut session = campaign();
    let particles = &session.session.definition.presentation.particles;
    let live = &mut session.session.state.live;
    let slot = super::construct_spell_wind(
        &mut live.economy,
        &mut live.terrain,
        particles,
        live.core.state.position,
    )
    .unwrap()
    .unwrap();
    let old_generation = live.economy.economy.slot_generation(slot);
    let wind = live
        .economy
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.source_entity_index == slot)
        .unwrap();
    wind.control_flags |= 0x400;
    live.economy.economy.claim_raw_replacement_slot(slot);
    assert_ne!(live.economy.economy.slot_generation(slot), old_generation);
    assert!(live.economy.entities[usize::from(slot)].active);
    super::retire_hidden_wind_projections(&mut live.economy, &mut live.terrain).unwrap();
    assert!(
        live.economy.entities[usize::from(slot)].active,
        "the old field must not retire the new occupant at the same address"
    );
}

#[test]
fn hidden_wind_waits_for_source_rebuild_not_bare_allocation() {
    let mut session = campaign();
    let particles = &session.session.definition.presentation.particles;
    let live = &mut session.session.state.live;
    let root = super::construct_spell_wind(
        &mut live.economy,
        &mut live.terrain,
        particles,
        live.core.state.position,
    )
    .unwrap()
    .unwrap();
    let children: Vec<_> = live
        .economy
        .economy
        .fields
        .model22_spawned_children
        .iter()
        .filter(|child| child.parent_entity_index == root)
        .map(|child| child.source_entity_index)
        .collect();
    assert_eq!(children.len(), 11);
    live.economy
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.source_entity_index == root)
        .unwrap()
        .life = 0;
    assert!(
        live.economy
            .economy
            .step_authored_model22_slot(root, &live.terrain)
    );
    let unrelated = live
        .economy
        .allocate_source_slot_with_terrain(&mut live.terrain)
        .unwrap();
    assert_ne!(unrelated, root);
    assert!(!children.contains(&unrelated));
    assert!(live.economy.map_chains.is_linked(root));
    assert!(
        children
            .iter()
            .all(|slot| live.economy.map_chains.is_linked(*slot))
    );
    let tiles: Vec<_> = std::iter::once(root)
        .chain(children.iter().copied())
        .map(|slot| live.economy.map_chains.tile_of(slot))
        .collect();

    // sub_4A1E0 explicitly calls sub_49F90, unlike ordinary NewEvent.
    live.economy
        .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
        .unwrap();
    assert!(!live.economy.entities[usize::from(root)].active);
    assert!(!live.economy.map_chains.is_linked(root));
    for slot in children {
        assert!(!live.economy.entities[usize::from(slot)].active);
        assert!(!live.economy.map_chains.is_linked(slot));
    }
    for tile in tiles {
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile),
            "source rebuild publishes the reclaimed tile head"
        );
    }
}

#[test]
fn authored_wind_rebuild_publishes_typed_chain_reclaim() {
    let mut session = campaign();
    let root = session
        .allocate_level2_authored_model22(110)
        .unwrap()
        .source_entity_index;
    let live = &mut session.session.state.live;
    let slots: Vec<_> = std::iter::once(root)
        .chain(
            live.economy
                .economy
                .fields
                .model22_spawned_children
                .iter()
                .filter(|child| child.parent_entity_index == root)
                .map(|child| child.source_entity_index),
        )
        .collect();
    assert_eq!(slots.len(), 12);
    let tiles: Vec<_> = slots
        .iter()
        .map(|slot| live.economy.map_chains.tile_of(*slot))
        .collect();
    for &tile in &tiles {
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile)
        );
    }
    live.economy
        .economy
        .fields
        .authored_model22
        .iter_mut()
        .find(|wind| wind.source_entity_index == root)
        .unwrap()
        .life = 0;
    assert!(
        live.economy
            .economy
            .step_authored_model22_slot(root, &live.terrain)
    );
    live.economy
        .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
        .unwrap();
    for slot in slots {
        assert!(!live.economy.map_chains.is_linked(slot));
    }
    for tile in tiles {
        assert_eq!(
            live.terrain.planes().entity_indices[usize::from(tile)] as u16,
            live.economy.map_chains.tile_head(tile)
        );
    }
}

#[test]
fn scheduled_authored_wind_publishes_old_and_new_map_heads() {
    let mut session = campaign();
    let root = session
        .allocate_level2_authored_model22(110)
        .unwrap()
        .source_entity_index;
    let slots: Vec<_> = std::iter::once(root)
        .chain(
            session
                .economy
                .model22_spawned_children()
                .iter()
                .filter(|child| child.parent_entity_index == root)
                .map(|child| child.source_entity_index),
        )
        .collect();
    let mut crossed = false;
    for _ in 0..4 {
        let before: Vec<_> = slots
            .iter()
            .map(|slot| session.economy.map_chains.tile_of(*slot))
            .collect();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "Wind scheduling must not roll back on a dispatch error"
        );
        let live = &session.session.state.live;
        for (&slot, &old_tile) in slots.iter().zip(&before) {
            let new_tile = live.economy.map_chains.tile_of(slot);
            crossed |= old_tile != new_tile;
            for tile in [old_tile, new_tile] {
                assert_eq!(
                    live.terrain.planes().entity_indices[usize::from(tile)] as u16,
                    live.economy.map_chains.tile_head(tile)
                );
            }
        }
    }
    assert!(crossed, "scheduled wind must cross a tile during the check");
}

#[test]
fn ordinary_level_two_wind_moves_player_and_naturally_reclaims_its_chain() {
    use crate::{FlightEvent, MissionAudioSite, campaign::journal::CampaignEffect};
    use crate::native_world::{Level2DynamicArenaKind, Mc2DispatchEvent};

    let mut session = campaign();
    let switch = session
        .level2_imported_switch_roots
        .iter()
        .find(|root| root.authored_index == 121 && root.target_group == 15)
        .copied()
        .expect("real group15 Wind producer");
    let player = session.state.entity_index;
    // Declared player-only approach to the actual imported proximity switch.
    session
        .set_level2_entity_position(player, switch.position)
        .unwrap();
    let mut activated = false;
    for _ in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        if session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot, group: 15, authored_indices, .. }
                if *source_slot == switch.source_slot && authored_indices.as_ref() == [122])) {
            activated = true;
            break;
        }
    }
    assert!(
        activated,
        "ordinary switch121 must publish authored group15"
    );
    let wind = *session
        .economy()
        .authored_model22()
        .iter()
        .find(|wind| wind.authored_index == 122)
        .expect("actual group15 Wind122");
    assert_eq!((wind.model, wind.action), (22, 22));
    let root = session
        .source_arena()
        .handle(wind.source_entity_index)
        .unwrap();
    let children: Vec<_> = session
        .economy()
        .model22_spawned_children()
        .iter()
        .filter(|child| child.active && child.parent_entity_index == root.source_slot)
        .map(|child| {
            session
                .source_arena()
                .handle(child.source_entity_index)
                .unwrap()
        })
        .collect();
    assert_eq!(
        children.len(),
        11,
        "ordinary Wind constructor publishes all linked children"
    );
    assert!(
        wind.life > 2,
        "the real authored lifetime must allow the encounter"
    );

    // Approach once at the live Wind axis. All actor and Wind state remains source-owned.
    let mut approach = wind.position;
    approach.z = session
        .canonical_terrain()
        .ground_height(approach)
        .wrapping_add(1_000);
    session
        .set_level2_entity_position(player, approach)
        .unwrap();
    assert!(
        root.source_slot > player,
        "the player visit precedes this Wind source visit"
    );
    let audio_start = session.effect_journal().records().len();
    let mut historical = session.clone();
    historical.set_replay_legacy_wind_audio(true);
    historical.step_outer_turn(PlayerCommand::default(), 0);
    let player_steps = session.step_outer_turn(PlayerCommand::default(), 0);
    assert_eq!(
        player_steps.len(),
        1,
        "speed index0 runs one actual player source visit"
    );
    assert!(session.dispatch_events().iter().any(|event| matches!(event,
        Mc2DispatchEvent::DynamicArena { source_slot, kind: Level2DynamicArenaKind::AuthoredModel22, .. }
            if *source_slot == root.source_slot)), "actual Wind callback runs");
    let live = &session.session.state.live;
    let flags = live.economy.entities[usize::from(player)].control_flags;
    let after_wind = live
        .economy
        .authored_model22()
        .iter()
        .find(|row| row.source_entity_index == root.source_slot)
        .unwrap();
    assert_eq!(historical.state.position, live.core.state.position);
    assert_eq!(historical.economy().authored_model22(), live.economy.authored_model22());
    assert!(!historical.effect_journal().records()[audio_start..].iter().any(|record| matches!(record.effect,
        CampaignEffect::Flight(FlightEvent::PrepareSound { sound_id: 49, .. }))),
        "the historical prefix omits only the Wind audio join, retaining ordinary Wind motion");
    assert_ne!(
        (live.core.state.position.x, live.core.state.position.y),
        (player_steps[0].committed.x, player_steps[0].committed.y),
        "the later Wind callback changes horizontal player position after ordinary flight"
    );
    assert_eq!(
        (live.core.state.position.x, live.core.state.position.y),
        (after_wind.axis_position.x, after_wind.axis_position.y),
        "the actual player lands at the current Wind axis through sub33340's inner band"
    );
    assert_ne!(flags & 0x1000_0000, 0, "Wind sets its actual airborne flag");
    assert_eq!(
        live.economy.entities[usize::from(player)].position,
        live.core.state.position
    );
    assert_eq!(
        live.shared_spells.player_entity().position,
        live.core.state.position
    );
    let audio = session.effect_journal().records()[audio_start..].iter()
        .filter(|record| matches!(record.effect, CampaignEffect::Flight(FlightEvent::PrepareSound {
            audio_site: MissionAudioSite::WhirlwindField, entity_index, player_index: -1, sound_id: 49,
        }) if entity_index == root.source_slot)).collect::<Vec<_>>();
    assert_eq!(audio.len(), 1, "one sound request follows the actual Wind update");
    let audio = audio[0].audio.expect("the Wind request carries its prepared source position");
    assert_eq!(audio.play_type, 4);
    assert_eq!(audio.command.rate_delta_percent, 0);
    assert_eq!(audio.command.sound_flags, 0);
    assert!(audio.command.audible);
    let listener = live.core.state.position;
    assert_eq!((audio.command.relative_x, audio.command.relative_y, audio.command.relative_z), (
        i32::from(after_wind.position.x) - i32::from(listener.x),
        i32::from(after_wind.position.y) - i32::from(listener.y),
        i32::from(after_wind.position.z) - i32::from(listener.z),
    ), "the moving field and the affected listener use their current source poses");

    // Retreat while the unmodified authored lifetime advances; return once
    // near its natural end so its real cleanup sees the same player carrier.
    let mut away = live.core.state.position;
    away.x = away.x.wrapping_add(0x8000);
    session.set_level2_entity_position(player, away).unwrap();
    let budget = usize::try_from(wind.life).unwrap() + 16;
    let mut returned = false;
    let mut hidden = false;
    let mut reclaimed = false;
    let mut reaffected_before_expiry = false;
    for _ in 0..budget {
        let current = session
            .economy()
            .authored_model22()
            .iter()
            .find(|row| row.source_entity_index == root.source_slot)
            .copied();
        if let Some(current) = current
            && current.life <= 2
            && !returned
        {
            let mut near = current.position;
            near.z = session
                .canonical_terrain()
                .ground_height(near)
                .wrapping_add(1_000);
            session.set_level2_entity_position(player, near).unwrap();
            returned = true;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. }))
        );
        let arena = session.source_arena();
        if returned && arena.entities[usize::from(player)].control_flags & 0x1000_0000 != 0 {
            reaffected_before_expiry = true;
        }
        if arena.resolve(root).is_none() {
            assert!(
                hidden,
                "the original Wind must hide before following-boundary reclaim"
            );
            assert!(
                children.iter().all(|child| arena.resolve(*child).is_none()),
                "the original linked generations are reclaimed"
            );
            reclaimed = true;
            break;
        }
        if let Some(row) = session
            .economy()
            .authored_model22()
            .iter()
            .find(|row| row.source_entity_index == root.source_slot)
            && row.control_flags & 0x400 != 0
        {
            assert!(returned);
            assert!(
                reaffected_before_expiry,
                "the actual player carried a Wind flag before cleanup"
            );
            assert_eq!(arena.handle(root.source_slot), Some(root));
            let player_flags = arena.entities[usize::from(player)].control_flags;
            assert_eq!(
                player_flags & 0x1000_0800,
                0,
                "Wind expiry clears its actual player flags"
            );
            assert!(session.field_source_rows().iter().any(|source| source.index
                == root.source_slot
                && source.control_flags & 0x400 != 0));
            let terminal = session.effect_journal().records().iter().filter(|record| matches!(record.effect,
                CampaignEffect::Flight(FlightEvent::EndLoop {
                    audio_site: MissionAudioSite::WhirlwindField, entity_index, player_index: -1, sound_id: 49,
                }) if entity_index == root.source_slot)).collect::<Vec<_>>();
            assert_eq!(terminal.len(), 1, "the terminal Wind visit requests one finite voice end");
            assert_eq!(terminal[0].audio.unwrap().play_type, 4);
            hidden = true;
        }
    }
    assert!(
        returned && hidden && reclaimed,
        "authored Wind must naturally expire and reclaim"
    );
    let events = session.effect_journal().records()[audio_start..].iter()
        .filter(|record| match record.effect {
            CampaignEffect::Flight(FlightEvent::PrepareSound { entity_index, sound_id: 49, .. }
                | FlightEvent::EndLoop { entity_index, sound_id: 49, .. }) => entity_index == root.source_slot,
            _ => false,
        }).map(|record| &record.effect).collect::<Vec<_>>();
    assert!(matches!(events.last(), Some(CampaignEffect::Flight(FlightEvent::EndLoop { .. }))));
    assert_eq!(events.iter().filter(|event| matches!(event, CampaignEffect::Flight(FlightEvent::EndLoop { .. }))).count(), 1,
        "reclaim does not reissue the terminal request or start the sound again");
}

#[test]
fn source_field_constructors_keep_horizontal_bounds_separate_from_height() {
    let mut session = campaign();
    let wind = session.allocate_level2_authored_model22(110).unwrap();
    let rows = session.field_source_rows();
    let root = rows
        .iter()
        .find(|row| row.index == wind.source_entity_index)
        .unwrap();
    assert_eq!((root.array_pitch(), root.array_fov()), (38, 31));
    assert_eq!(
        i16::from_le_bytes(root.record[90..92].try_into().unwrap()),
        38
    );
    let child = rows.iter().find(|row| row.model == 75).unwrap();
    assert_eq!(child.actual_speed, 50, "AddWind clones the parent's speed");
    assert_eq!(
        i16::from_le_bytes(child.record[90..92].try_into().unwrap()),
        child.array_pitch()
    );
    assert!(child.array_fov() > 0);
    let edge = Position {
        y: wind.position.y.wrapping_add(37),
        ..wind.position
    };
    assert!(
        wind::pulse_overlap(wind, edge, 0, 0),
        "vertical height31 cannot narrow the horizontal band38"
    );
    assert!(
        !wind::pulse_overlap(
            wind,
            Position {
                y: edge.y.wrapping_add(1),
                ..edge
            },
            0,
            0
        ),
        "source overlap remains strict at38"
    );

    let particle =
        session.session.definition.presentation.particles[usize::from(transport::PARTICLE)];
    let live = &mut session.session.state.live;
    let slot = transport::construct(
        &mut live.economy,
        &mut live.terrain,
        live.core.state.position,
        definition(FieldKind::Transport),
        particle,
    )
    .unwrap()
    .unwrap();
    let row = &live.economy.entities[usize::from(slot)];
    assert_eq!((row.array_pitch(), row.array_fov()), (256, 256));
    assert_eq!(
        i16::from_le_bytes(row.record[90..92].try_into().unwrap()),
        256
    );
}

#[test]
fn scheduled_slowing_reaches_player_upkeep_palette_and_reclaim() {
    let mut session = campaign();
    let slot = scheduled_impact(&mut session, FieldKind::Slowing);
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.core.state.speed_modifier, 1);
    assert_eq!(
        session.session.state.live.core.state.speed_modifier_counter,
        8
    );
    assert!(session.session.state.live.economy.entities[usize::from(slot)].drawing_disabled());
    campaign_step(&mut session);
    assert!(!session.session.state.live.economy.entities[usize::from(slot)].active);
    assert_eq!(
        session.session.state.live.core.state.speed_modifier_counter,
        7
    );
    let original = [96, 128, 160].repeat(256);
    let mut tinted = original.clone();
    session.field_palette().apply_rgb8(&mut tinted);
    assert_ne!(tinted, original, "actual frame palette consumes slowing");
    for _ in 0..12 {
        campaign_step(&mut session);
    }
    let state = &session.session.state.live.core.state;
    assert_eq!((state.speed_modifier, state.speed_modifier_counter), (0, 0));
    let mut recovered = original.clone();
    session.field_palette().apply_rgb8(&mut recovered);
    assert_eq!(
        recovered, original,
        "frame service finishes the recovery fade"
    );
}

#[test]
fn scheduled_web_damages_actual_player_once_and_refresh_only_recovers() {
    // The source starts the player with 100 protected visits. The same Web
    // contact is discarded by that prelude before normal damage consumption.
    let mut protected = campaign();
    let protected_life = protected.session.state.live.player_life;
    scheduled_impact(&mut protected, FieldKind::Web);
    campaign_step(&mut protected);
    campaign_step(&mut protected);
    assert_eq!(protected.session.state.live.player_life, protected_life);
    assert_eq!(protected.session.state.live.player_pending_attacker, 0);

    let mut session = campaign();
    // Exercise the ordinary post-protection visit, not the spawn grace period.
    session.session.state.live.player_protection_ticks = 0;
    let life = session.session.state.live.player_life;
    let first = scheduled_impact(&mut session, FieldKind::Web);
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.player_life, life);
    assert_eq!(session.session.state.live.player_pending_damage, 200);
    assert_eq!(session.session.state.live.player_pending_attacker, first);
    assert_eq!(
        session.session.state.live.core.state.immobilize_subcounter,
        10
    );
    let second = scheduled_impact(&mut session, FieldKind::Web);
    let rng = session.session.state.live.economy.entities[usize::from(second)].entity_rng;
    campaign_step(&mut session);
    assert_eq!(session.session.state.live.player_life, life - 200);
    assert_eq!(session.session.state.live.player_pending_attacker, 0);
    assert_eq!(
        session.session.state.live.core.state.immobilize_subcounter,
        10
    );
    assert_eq!(
        session.session.state.live.economy.entities[usize::from(second)].entity_rng,
        rng
    );
    for _ in 0..10 {
        campaign_step(&mut session);
    }
    let state = &session.session.state.live.core.state;
    assert_eq!(
        (state.immobilize_counter, state.immobilize_subcounter),
        (0, 0)
    );
    assert_eq!(session.session.state.live.player_life, life - 200);
    assert!(!session.session.state.live.economy.entities[usize::from(second)].active);
}

#[test]
fn slowing_impacts_stack_once_then_real_flight_recovers_all_stages() {
    let (body, mut state) = player();
    let mut source = impact(status::Kind::Slowing);
    for (stage, count) in [(1, 142), (2, 199), (3, 256)] {
        let result = status::impact(
            &mut source,
            definition(FieldKind::Slowing),
            Some((&body, &mut state)),
            0,
        )
        .unwrap();
        assert_eq!(state.speed_modifier, stage);
        assert_eq!(state.speed_modifier_counter, 8);
        assert_eq!(result.palette.unwrap().count, count);
        assert_eq!(result.sounds.len(), usize::from(stage == 1));
        assert!(result.contacts.is_empty());
    }
    let rng = source.entity_rng;
    state.recoil_speed = 12;
    let capped = status::impact(
        &mut source,
        definition(FieldKind::Slowing),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert!(capped.palette.is_none());
    assert_eq!(source.entity_rng, rng);
    assert_eq!(state.recoil_speed, 12);
    assert!(source.drawing_disabled());
    let mut context = FlightContext {
        effects: Some(FlightEffectsContext::default()),
        ..FlightContext::default()
    };
    for tick in 1..=24 {
        step_turn_with_context(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &Ground,
            &mut context,
        );
        assert_eq!(state.speed_modifier, 3 - tick / 8);
    }
    assert_eq!(state.speed_modifier_counter, 0);
    assert_eq!(context.effects.unwrap().palette_submode, 1);
    let before = state.position;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &Ground,
        &mut FlightContext::default(),
    );
    assert!(
        state.position.y < before.y,
        "ordinary translation resumes after expiry"
    );
}

#[test]
fn web_refreshes_without_duplicate_contact_and_real_movement_expires_it() {
    let (body, mut state) = player();
    let mut source = impact(status::Kind::Web);
    source.set_subspell_index(200);
    let first = status::impact(
        &mut source,
        definition(FieldKind::Web),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert_eq!((first.contacts.len(), first.contacts[0].payload), (1, 200));
    assert_eq!(
        (
            state.immobilize_counter,
            state.immobilize_subcounter,
            state.recoil_speed
        ),
        (1, 10, -80)
    );
    let rng = source.entity_rng;
    let refresh = status::impact(
        &mut source,
        definition(FieldKind::Web),
        Some((&body, &mut state)),
        0,
    )
    .unwrap();
    assert!(refresh.contacts.is_empty() && refresh.sounds.is_empty());
    assert_eq!(source.entity_rng, rng);
    state.recoil_speed = 0;
    let start = state.position;
    for tick in 1..=10 {
        step_turn_with_context(
            &mut state,
            PlayerCommand::default(),
            FlightProfile::OUTDOOR,
            &Ground,
            &mut FlightContext::default(),
        );
        assert_eq!(state.immobilize_counter, if tick < 10 { 1 } else { 0 });
        assert_eq!(
            state.position.y, start.y,
            "web consumes the actual translation branch"
        );
    }
    assert_eq!(state.position.z, start.z - 9 * 51 - 16);
    let before = state.position;
    step_turn_with_context(
        &mut state,
        PlayerCommand::default(),
        FlightProfile::OUTDOOR,
        &Ground,
        &mut FlightContext::default(),
    );
    assert!(state.position.y < before.y);
}

#[test]
fn transport_uses_facing_and_destination_and_life_one_hides_before_contact() {
    let (mut body, mut state) = player();
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y - 1,
            ..state.position
        },
    );
    source.record[88..90].copy_from_slice(&256_i16.to_le_bytes());
    source.record[90..92].copy_from_slice(&256_i16.to_le_bytes());
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());
    let mut rejected_source = source.clone();
    let mut rejected_body = body.clone();
    let mut rejected_player = state.clone();
    rejected_player.yaw = 170;
    let rejected = transport::step(
        &mut rejected_source,
        &mut rejected_body,
        &mut rejected_player,
        &Ground,
        384,
        0,
        &mut |_, _| {},
    );
    assert!(
        !rejected.transported,
        "facing170 is outside the strict gate"
    );
    assert_eq!(rejected_player.position, state.position);
    state.yaw = 169;
    let phase = source.phase;
    let outcome = transport::step(
        &mut source,
        &mut body,
        &mut state,
        &Ground,
        384,
        0,
        &mut |_, _| {},
    );
    assert!(outcome.transported);
    assert_eq!(
        state.position,
        Position {
            x: 30_000,
            y: 40_000,
            z: 384
        }
    );
    assert_eq!(body.position, state.position);
    assert_eq!(source.life, 0);
    assert_eq!(source.phase, phase, "common tail owns phase");
    assert_eq!(outcome.sounds, [(120, 21), (120, 22)]);
    source.set_life(1);
    let before = state.position;
    let outcome = transport::step(
        &mut source,
        &mut body,
        &mut state,
        &Ground,
        384,
        0,
        &mut |_, _| {},
    );
    assert!(!outcome.transported);
    assert_eq!(
        (source.life, source.drawing_disabled(), state.position),
        (0, true, before)
    );
    assert_eq!(outcome.sounds, [(120, 20)]);
}

#[test]
fn transport_uses_distinct_y_extent_and_unwrapped_vertical_distance() {
    let (mut body, mut state) = player();
    state.yaw = 0;
    body.set_source_overlap_half_extents(2, 10);
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y.wrapping_sub(29),
            ..state.position
        },
    );
    source.set_source_overlap_half_extents(2, 20);
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());

    let mut edge_source = source.clone();
    let mut edge_body = body.clone();
    let mut edge_state = state.clone();
    edge_source.set_position(Position {
        y: edge_source.position.y.wrapping_sub(1),
        ..edge_source.position
    });
    assert!(
        !transport::step(
            &mut edge_source,
            &mut edge_body,
            &mut edge_state,
            &Ground,
            384,
            0,
            &mut |_, _| {},
        )
        .transported
    );
    assert!(
        transport::step(
            &mut source,
            &mut body,
            &mut state,
            &Ground,
            384,
            0,
            &mut |_, _| {}
        )
        .transported
    );

    let (mut body, mut state) = player();
    state.yaw = 0;
    state.position.z = i16::MIN;
    body.set_position(state.position);
    let mut source = new_event(
        121,
        0,
        Position {
            z: i16::MAX,
            y: state.position.y.wrapping_sub(1),
            ..state.position
        },
    );
    source.set_source_overlap_half_extents(256, 256);
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    assert!(
        !transport::step(
            &mut source,
            &mut body,
            &mut state,
            &Ground,
            384,
            0,
            &mut |_, _| {}
        )
        .transported
    );
}

#[test]
fn transport_visits_each_player_before_finishing_its_single_lifetime_tick() {
    let (body, state) = player();
    let mut source = new_event(
        120,
        0,
        Position {
            y: state.position.y.wrapping_sub(1),
            ..state.position
        },
    );
    source.set_source_overlap_half_extents(256, 256);
    source.record[92..94].copy_from_slice(&256_i16.to_le_bytes());
    source.record[158..160].copy_from_slice(&30_000_u16.to_le_bytes());
    source.record[160..162].copy_from_slice(&40_000_u16.to_le_bytes());
    source.set_life(2);
    let original_height = source.position.z;
    let (mut outcome, visit_players) = transport::begin(&mut source, &mut |_, _| {});
    assert!(visit_players);
    assert_eq!(source.life, 1, "one source callback decrements life once");
    let mut local = body.clone();
    let mut rival = body.clone();
    let mut facing_away = body;
    assert!(
        transport::visit_player(
            &mut source,
            &mut local,
            state.position,
            0,
            &Ground,
            384,
            true,
            &mut outcome,
            &mut |_, _| {},
        )
        .is_some()
    );
    assert!(
        transport::visit_player(
            &mut source,
            &mut rival,
            state.position,
            0,
            &Ground,
            384,
            false,
            &mut outcome,
            &mut |_, _| {},
        )
        .is_some()
    );
    assert!(
        transport::visit_player(
            &mut source,
            &mut facing_away,
            state.position,
            170,
            &Ground,
            384,
            false,
            &mut outcome,
            &mut |_, _| {},
        )
        .is_none()
    );
    assert_eq!(rival.position, local.position);
    assert_eq!(facing_away.position, state.position);
    assert_eq!(
        source.position.z, original_height,
        "the source height changes only after all visits"
    );
    assert_eq!(outcome.sounds, [(120, 21), (120, 22), (120, 22)]);
    assert_eq!(
        outcome.palette,
        Some(super::PaletteRequest { mode: 6, count: 0 })
    );
    transport::finish(&mut source, &Ground);
    assert_eq!(source.position.z, 0);
}

#[test]
fn retained_ordinary_transport_moves_scheduled_player_and_publishes_map_heads() {
    use crate::campaign::journal::CampaignEffect;

    let level = std::env::var("MC_TRANSPORT_LEVEL").map_or(15, |value| {
        value.parse().expect("transport level is a number")
    });
    let package = std::env::var_os("MC_TRANSPORT_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261024-b/level15")
        },
        std::path::PathBuf::from,
    );
    let base = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(level).unwrap(),
    )
    .expect("retained ordinary package loads")
    .simulation()
    .clone();
    let source = base
        .economy
        .entities
        .iter()
        .find(|row| {
            row.active && (row.class, row.model, row.action) == (10, 34, 36) && row.life == 0
        })
        .expect("owner-authored reset contains a persistent transport")
        .clone();
    let source_handle = base.source_arena().handle(source.index).unwrap();
    if level == 15 {
        assert_ne!(
            source.control_flags & 2,
            0,
            "the retained Level15 packet already has the source first-update flag"
        );
    }
    let start_pending = source.control_flags & 2 == 0;
    let mut destination = transport::destination(&source);
    let parameter = source
        .parameter_relation
        .and_then(|index| {
            base.session
                .definition
                .presentation
                .entity_parameters
                .get(usize::from(index))
        })
        .expect("retained transport has its source parameter relation");
    destination.z = base
        .canonical_terrain()
        .ground_height(destination)
        .wrapping_add(i16::from_le_bytes([parameter[12], parameter[13]]));
    let contact = Position {
        y: source.position.y.wrapping_add(1),
        ..source.position
    };
    let heading = crate::heading_for_positions(contact, source.position) as i16;
    let ballistic = if level == 17 {
        let row = base
            .economy
            .entities
            .iter()
            .find(|row| row.active && (row.class, row.model, row.action) == (2, 7, 20))
            .expect("retained Level 17 contains its source ballistic action20");
        Some((
            row.index,
            row.phase,
            base.economy.map_chains.tile_of(row.index),
        ))
    } else {
        None
    };
    let rival = base
        .core
        .players
        .iter()
        .take(usize::from(base.core.runtime.player_count))
        .find(|player| player.is_ai && player.active_player_index >= 0 && player.entity_index != 0)
        .map(|player| player.entity_index);
    if base.core.runtime.player_count > 1 {
        assert!(
            rival.is_some(),
            "multi-wizard source package has a participating rival"
        );
    }
    let mut audible_rng = None;
    for (facing, expected_transport, muted, expires) in [
        (heading, true, false, false),
        ((heading + 1024) & 2047, false, false, false),
        (heading, true, true, false),
        (heading, false, false, true),
    ] {
        let mut session = base.clone();
        let player_slot;
        {
            let live = &mut session.session.state.live;
            // Only these two variants configure exceptional audio branches.
            // The ordinary facing pair keeps authored life0 and source flags.
            let transport = &mut live.economy.entities[usize::from(source.index)];
            if muted {
                transport.set_control_flags(transport.control_flags | 0x80);
            }
            if expires {
                transport.set_life(1);
            }
            player_slot = live.core.state.entity_index;
            live.core.state.position = contact;
            live.core.state.yaw = facing;
            live.core.state.actual_speed = 0;
            live.core.state.target_speed = 0;
            let body = &mut live.economy.entities[usize::from(player_slot)];
            body.set_position(contact);
            body.set_yaw(facing);
            crate::world::effects::sync_position(
                &mut live.economy,
                &mut live.terrain,
                player_slot,
                contact,
                true,
                false,
            )
            .unwrap();
            if let Some(rival_slot) = rival {
                let rival_body = &mut live.economy.entities[usize::from(rival_slot)];
                assert!(
                    rival_body.active,
                    "retained rival has an active source body"
                );
                rival_body.set_position(contact);
                rival_body.set_yaw(facing);
                crate::world::effects::sync_position(
                    &mut live.economy,
                    &mut live.terrain,
                    rival_slot,
                    contact,
                    true,
                    false,
                )
                .unwrap();
            }
        }
        session
            .set_level2_entity_position(player_slot, contact)
            .unwrap();
        let old_tile = session.economy.map_chains.tile_of(player_slot);
        let cursor = session.effect_journal().cursor(0).next_sequence;
        let listener = session.source_arena().handle(player_slot).unwrap();
        let visited_players = session
            .core
            .players
            .iter()
            .take(usize::from(session.core.runtime.player_count))
            .filter(|player| {
                player.entity_index == player_slot || Some(player.entity_index) == rival
            })
            .map(|player| session.source_arena().handle(player.entity_index).unwrap())
            .collect::<Vec<_>>();
        session.start_transport_audio_trace();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let boundaries = session
            .take_transport_audio_trace()
            .into_iter()
            .filter(|call| call.source == source_handle)
            .collect::<Vec<_>>();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "scheduled transport must not roll back: {:?}",
            session.dispatch_events()
        );
        assert_eq!(
            session.source_arena().handle(source.index),
            Some(source_handle)
        );
        let transport_audio = session
            .effect_journal()
            .records()
            .iter()
            .filter_map(|record| {
                if record.sequence < cursor {
                    return None;
                }
                match record.effect {
                    CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                        entity_index,
                        player_index: -1,
                        sound_id: 20..=22,
                        ..
                    }) if entity_index == source.index => Some(record),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        let sound_ids = transport_audio
            .iter()
            .map(|record| match record.effect {
                CampaignEffect::Flight(crate::FlightEvent::PrepareSound { sound_id, .. }) => {
                    sound_id
                }
                _ => unreachable!("only transport preparation records were retained"),
            })
            .collect::<Vec<_>>();
        let mut expected_sounds = if start_pending { vec![21] } else { Vec::new() };
        if expires {
            expected_sounds.push(20);
        } else if expected_transport {
            expected_sounds.extend(std::iter::repeat_n(22, 1 + usize::from(rival.is_some())));
        }
        assert_eq!(
            sound_ids, expected_sounds,
            "actual source-order transport audio"
        );
        assert_eq!(
            boundaries.len(),
            transport_audio.len(),
            "one journal record per actual source call"
        );
        let mut local_has_teleported = false;
        let mut arrival_targets = Vec::new();
        for (record, boundary) in transport_audio.iter().zip(&boundaries) {
            let entry = boundaries
                .first()
                .expect("this actual source call has an entry");
            assert_eq!(boundary.journal_sequence, record.sequence);
            assert_eq!(boundary.source, source_handle);
            assert_eq!(boundary.listener, listener);
            assert_eq!(
                boundary.source_position, source.position,
                "all preparation precedes finish lowering the emitter"
            );
            assert_eq!(boundary.source_id, source.source_id);
            assert_eq!(boundary.source_action, 36);
            let mut expected_source = source.clone();
            expected_source.set_control_flags(
                expected_source.control_flags | 2 | if muted { 0x80 } else { 0 },
            );
            if boundary.sound_id == 20 {
                expected_source.disable_drawing();
            }
            assert_eq!(boundary.source_flags, expected_source.control_flags);
            let expected_site = match boundary.sound_id {
                21 => crate::MissionAudioSite::TransportStart,
                22 => crate::MissionAudioSite::TransportArrive,
                20 => crate::MissionAudioSite::TransportExpire,
                _ => unreachable!("actual transport sounds only"),
            };
            assert!(
                matches!(record.effect, CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site, entity_index, player_index: -1, sound_id,
            }) if audio_site == expected_site && entity_index == source.index && sound_id == boundary.sound_id)
            );
            if boundary.sound_id == 22 {
                arrival_targets.push(
                    boundary
                        .target
                        .expect("arrival identifies the actual visited player"),
                );
                assert_eq!(
                    boundary.listener_position,
                    if local_has_teleported {
                        destination
                    } else {
                        entry.listener_position
                    },
                    "sound22 precedes each player's teleport"
                );
                if !local_has_teleported {
                    assert_ne!(
                        boundary.listener_position, destination,
                        "the actual listener has not yet been copied to the destination"
                    );
                }
                assert_eq!(boundary.listener_yaw, entry.listener_yaw);
                if boundary.target == Some(listener) {
                    local_has_teleported = true;
                }
            } else {
                assert_eq!(boundary.target, None);
            }
            let expected_audio = (!muted)
                .then(|| {
                    crate::prepare_event_sound_command(
                        boundary.listener_position,
                        boundary.listener_yaw,
                        boundary.source_position,
                        boundary.sound_id,
                        0,
                        boundary.source_id,
                        boundary.source_action,
                    )
                })
                .flatten();
            assert_eq!(
                record.audio, expected_audio,
                "prepared projection uses independently observed pre-copy source and listener"
            );
            assert_eq!(
                record.audio.is_none(),
                muted,
                "mute suppresses prepared audio without suppressing the source call"
            );
            let Some(prepared) = record.audio else {
                continue;
            };
            assert_eq!(prepared.play_type, 1);
            assert_eq!(prepared.command.rate_delta_percent, 0);
            assert_eq!(prepared.command.sound_flags, source.source_id as i16);
        }
        assert_eq!(
            arrival_targets,
            if expected_transport {
                visited_players
            } else {
                Vec::new()
            },
            "actual source player-array order is retained"
        );
        assert_eq!(local_has_teleported, expected_transport);
        let live = &session.session.state.live;
        assert_eq!(
            live.economy.entities[usize::from(source.index)].life,
            0,
            "ordinary life0 persists; configured oldlife1 decrements once"
        );
        assert_eq!(
            live.economy.entities[usize::from(source.index)].drawing_disabled(),
            expires
        );
        if facing == heading && !expires {
            let rng = (
                live.economy.economy.global_rng,
                live.economy.entities[usize::from(source.index)].entity_rng,
            );
            if muted {
                assert_eq!(Some(rng), audible_rng, "sound preparation spends no RNG");
            } else {
                audible_rng = Some(rng);
            }
        }
        if let Some((slot, before_phase, old_tile)) = ballistic {
            let row = &live.economy.entities[usize::from(slot)];
            assert!(row.active, "action20 survives its ordinary source pass");
            assert_eq!((row.class, row.model, row.action), (2, 7, 20));
            assert_eq!(
                row.phase,
                before_phase.wrapping_add(1),
                "enabled action20 advances its source tail"
            );
            for tile in [old_tile, live.economy.map_chains.tile_of(slot)] {
                assert_eq!(
                    live.terrain.planes().entity_indices[usize::from(tile)] as u16,
                    live.economy.map_chains.tile_head(tile)
                );
            }
        }
        let actual = live.core.state.position;
        let near = |a: u16, b: u16| i32::from(a.wrapping_sub(b) as i16).abs() < 256;
        assert_eq!(
            near(actual.x, destination.x) && near(actual.y, destination.y),
            expected_transport,
            "forward-facing contact transports unless the configured source expires first"
        );
        if expected_transport {
            assert_eq!(
                transport::destination(&live.economy.entities[usize::from(source.index)]).z,
                destination.z
            );
        } else {
            assert!(near(actual.x, contact.x) && near(actual.y, contact.y));
        }
        if let Some(rival_slot) = rival {
            let rival_position = live.economy.entities[usize::from(rival_slot)].position;
            assert_eq!(
                near(rival_position.x, destination.x) && near(rival_position.y, destination.y),
                expected_transport,
                "the same scheduled source visits the rival wizard"
            );
            if !expected_transport {
                assert!(near(rival_position.x, contact.x) && near(rival_position.y, contact.y));
            }
        }
        let player_slot = live.core.state.entity_index;
        let new_tile = live.economy.map_chains.tile_of(player_slot);
        assert!(live.economy.map_chains.is_linked(player_slot));
        for tile in [old_tile, new_tile] {
            assert_eq!(
                live.terrain.planes().entity_indices[usize::from(tile)] as u16,
                live.economy.map_chains.tile_head(tile)
            );
        }
        if expires {
            // The configured source expires naturally in its scheduled visit;
            // its original generation is reclaimed only on the following pass.
            let cursor = session.effect_journal().cursor(0).next_sequence;
            session.step_outer_turn(PlayerCommand::default(), 0);
            assert!(
                session.dispatch_events().iter().all(|event| !matches!(
                    event,
                    crate::native_world::Mc2DispatchEvent::Error { .. }
                )),
                "{:?}",
                session.dispatch_events()
            );
            assert!(session.source_arena().resolve(source_handle).is_none());
            assert!(
                !session
                    .effect_journal()
                    .records()
                    .iter()
                    .any(|record| record.sequence >= cursor
                        && matches!(record.effect,
                    CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                        entity_index, sound_id: 20..=22, ..
                    }) if entity_index == source.index)),
                "reclaim has no repeated callback sound"
            );
        }
    }

    // The retained Level15 packet already has flag2. Exercise source21 with
    // a separately constructed, genuinely fresh transport, without rearming
    // or changing the authored source in the ordinary route above.
    let mut fresh = base.clone();
    let particle =
        fresh.session.definition.presentation.particles[usize::from(transport::PARTICLE)];
    let fresh_slot = {
        let live = &mut fresh.session.state.live;
        transport::construct(
            &mut live.economy,
            &mut live.terrain,
            source.position,
            definition(FieldKind::Transport),
            particle,
        )
        .unwrap()
        .expect("configured fresh transport uses the actual source constructor")
    };
    let fresh_handle = fresh.source_arena().handle(fresh_slot).unwrap();
    let initial = fresh.entities()[usize::from(fresh_slot)].clone();
    assert_eq!(initial.control_flags & 2, 0);
    let cursor = fresh.effect_journal().cursor(0).next_sequence;
    fresh.start_transport_audio_trace();
    fresh.step_outer_turn(PlayerCommand::default(), 0);
    let fresh_boundaries = fresh.take_transport_audio_trace();
    assert!(
        fresh
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
    );
    let start = fresh_boundaries
        .iter()
        .find(|call| call.source == fresh_handle && call.sound_id == 21)
        .expect("new source emits its actual first-update sound21");
    assert_eq!(start.target, None);
    assert_eq!(start.source_position, initial.position);
    assert_eq!(start.source_flags, initial.control_flags | 2);
    assert_eq!(start.source_id, initial.source_id);
    let start_record = fresh
        .effect_journal()
        .records()
        .iter()
        .find(|record| record.sequence >= cursor && record.sequence == start.journal_sequence)
        .expect("the actual source21 boundary reaches the Flight journal");
    assert!(matches!(start_record.effect,
        CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
            audio_site: crate::MissionAudioSite::TransportStart,
            entity_index, player_index: -1, sound_id: 21,
        }) if entity_index == fresh_slot));
    assert_eq!(
        start_record.audio,
        crate::prepare_event_sound_command(
            start.listener_position,
            start.listener_yaw,
            initial.position,
            21,
            0,
            initial.source_id,
            initial.action,
        )
    );
    let cursor = fresh.effect_journal().cursor(0).next_sequence;
    fresh.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        fresh
            .dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
    );
    assert!(
        !fresh.effect_journal().records().iter().any(|record| {
            record.sequence >= cursor
                && matches!(record.effect,
            CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                entity_index, sound_id: 21, ..
            }) if entity_index == fresh_slot)
        }),
        "a later visit does not replay the first-update sound"
    );
}

#[test]
fn follower_parent_sentinel_exact_life_and_hidden_parent_semantics() {
    let mut child = new_event(
        20,
        7,
        Position {
            x: 10,
            y: 20,
            z: 30,
        },
    );
    child.set_yaw(2040);
    child.set_actual_speed(0);
    let mut parent = new_event(
        10,
        9,
        Position {
            x: 500,
            y: 700,
            z: 900,
        },
    );
    let before = child.clone();
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert_eq!(child, before);
    child.record[44..46].copy_from_slice(&10_u16.to_le_bytes());
    parent.active = false;
    parent.disable_drawing();
    parent.set_life(-1);
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert_eq!(
        (child.position, child.yaw),
        (
            Position {
                x: 500,
                y: 700,
                z: 256
            },
            37
        )
    );
    assert!(!child.drawing_disabled());
    parent.set_life(1);
    transport::step_follower(&mut child, Some(&parent), &Ground);
    assert!(child.drawing_disabled());
}

#[test]
fn wind_motion_reads_and_writes_the_exported_heading_word() {
    let mut source = new_event(10, 0, Position { x: 0, y: 0, z: 0 });
    source.record[48..50].copy_from_slice(&1111_i16.to_le_bytes());
    source.record[52..54].copy_from_slice(&2222_i16.to_le_bytes());
    let actor = wind::Actor::from_source(&source, None);
    assert_eq!(actor.move_heading, 2222);
    let motion = wind::Motion {
        actor: wind::Actor {
            move_heading: 333,
            ..actor
        },
        actual_speed: None,
        player_roll_delta: 0,
        contact: false,
    };
    motion.write_source(&mut source);
    assert_eq!(wind::Actor::from_source(&source, None).move_heading, 333);
    assert_eq!(source.record[48..50], 1111_i16.to_le_bytes());
}

#[test]
fn palette_uses_rgb6_source_math_and_recovers_through_frame_fade() {
    let original = vec![240_u8; 768];
    let mut palette = super::palette::Palette::default();
    palette.request(super::PaletteRequest {
        mode: 3,
        count: 256,
    });
    let mut colors = original.clone();
    palette.apply_rgb8(&mut colors);
    assert_eq!(
        colors, original,
        "pending source request waits for the frame consumer"
    );
    assert!(palette.advance_frame());
    palette.apply_rgb8(&mut colors);
    assert_eq!(&colors[..6], &[240, 240, 240, 16, 240, 16]);
    for _ in 0..5 {
        palette.advance_frame();
    }
    let mut recovered = original.clone();
    palette.apply_rgb8(&mut recovered);
    assert_eq!(recovered, original);
    palette.request(super::PaletteRequest { mode: 6, count: 0 });
    palette.advance_frame();
    palette.apply_rgb8(&mut recovered);
    assert_eq!(&recovered[3..6], &[252, 252, 252]);
}
