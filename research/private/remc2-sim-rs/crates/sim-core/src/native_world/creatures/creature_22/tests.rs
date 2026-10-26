use super::{mc2, *};
use crate::native_world::Mc2Simulation;

fn source_session() -> Mc2Simulation {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    );
    crate::campaign::CampaignSession::open(
        root.join("level2"),
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("actual current Level2 package retains the29 source constructor counters")
    .simulation()
    .clone()
}

/// Tests borrow the ordinary package-backed arena, allocator, terrain and
/// retained particles; there is no alternate family Runtime implementation.
fn with_runtime<R>(
    session: &mut Mc2Simulation,
    operation: impl FnOnce(&mut mc2::Context<'_>) -> R,
) -> R {
    let particles = session.session.definition.presentation.particles.clone();
    let live = &mut session.session.state.live;
    let families = std::array::from_fn(|_| Vec::new());
    let class3 = live
        .economy
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3)
        .map(|row| row.index)
        .collect::<Vec<_>>();
    let mut runtime = mc2::Context {
        arena: &mut live.economy,
        runtime: &mut live.core.runtime,
        terrain: &mut live.terrain,
        particles: &particles,
        players: &live.core.players,
        families: &families,
        class3: &class3,
        outcome: mc2::Outcome::default(),
        error: None,
    };
    let result = operation(&mut runtime);
    runtime
        .finish()
        .expect("source family operations retain valid assets/map links");
    result
}

fn construct(session: &mut Mc2Simulation, length: Option<u8>) -> u16 {
    with_runtime(session, |runtime| {
        let position = Position {
            x: 0x4000,
            y: 0x4000,
            z: 0,
        };
        let result = if let Some(length) = length {
            mc2::allocate_authored(runtime, position, PROPERTIES.max_life, length, None, 218)
        } else {
            let mut phase = 0;
            mc2::construct(runtime, &mut phase, position, None)
        };
        result
            .expect("real source constructor")
            .expect("source ordinary free-stack threshold")
    })
}

#[test]
fn ordinary_body_is_paired_and_bare_constructor_has_no_children() {
    let mut session = source_session();
    let bare = construct(&mut session, None);
    let root = construct(&mut session, Some(5));
    with_runtime(&mut session, |runtime| {
        let bare = runtime.actor_at(bare).expect("bare root");
        assert_eq!(bare.length_or_ordinal, 15);
        assert!(
            bare.next.is_none(),
            "4CA00 does not build the authored body"
        );
        let root = runtime.actor_at(root).expect("authored root");
        let handles = chain(root.handle, runtime);
        assert_eq!(handles.len(), 5);
        let rows = handles
            .iter()
            .map(|handle| runtime.actor(*handle).expect("actual linked source row"))
            .collect::<Vec<_>>();
        assert_eq!(
            rows.iter()
                .skip(1)
                .map(|row| row.length_or_ordinal)
                .collect::<Vec<_>>(),
            [1, -1, 2, -2]
        );
        assert!(
            rows.iter()
                .skip(1)
                .all(|row| row.action == 180 && row.root == Some(root.handle) && row.mana == 0)
        );
        assert!(
            rows.iter()
                .all(|row| row.source_id == root.source_id && row.entity_rng == root.entity_rng)
        );
        assert!(
            rows.iter()
                .all(|row| row.half_width > 0 && row.height == row.half_width)
        );
        let census = runtime
            .arena
            .creature_runtime_census(&Default::default(), &Default::default());
        assert_eq!(
            census
                .roots_of_kind(crate::level_definition::CreatureKind::Creature22)
                .count(),
            1,
            "paired action180 pieces are not living creature roots"
        );
        for (row, handle) in rows.iter().zip(handles) {
            let raw = &runtime.arena.entities[usize::from(handle.source_slot)];
            assert_eq!((raw.class, raw.model, raw.action), (5, 22, row.action));
            assert_eq!(
                i16::from_le_bytes([raw.record_bytes()[88], raw.record_bytes()[89]]),
                row.half_width
            );
            assert_eq!(
                i16::from_le_bytes([raw.record_bytes()[90], raw.record_bytes()[91]]),
                row.half_width
            );
            assert_eq!(raw.particle_parameter_index, Some(row.visual_index));
        }
    });
}

#[test]
fn possession_changes_chain_colour_and_shrink_hides_exact_pair() {
    let mut session = source_session();
    let root_slot = construct(&mut session, Some(5));
    with_runtime(&mut session, |runtime| {
        let player = runtime
            .class3
            .iter()
            .copied()
            .find(|slot| runtime.player(*slot).is_some())
            .expect("real package player");
        let mut root = runtime.actor_at(root_slot).expect("root");
        let old = root.visual_index;
        root.possessor = player;
        root.cadence_counter = 1;
        runtime.write_actor(root);
        assert!(mc2::step(runtime, root_slot).expect("possession callback"));
        assert_eq!(runtime.actor_at(root_slot).expect("root").action, 177);
        for _ in 0..4 {
            mc2::step(runtime, root_slot).expect("ordered colour ring callback");
        }
        let root = runtime.actor_at(root_slot).expect("claimed root");
        assert_eq!(root.owner, player);
        assert_eq!(root.action, 178);
        assert_ne!(root.visual_index, old);
        let handles = chain(root.handle, runtime);
        resize(root.handle, 3, runtime);
        assert_eq!(chain(root.handle, runtime).len(), 3);
        for handle in &handles[3..] {
            assert!(
                runtime
                    .actor(*handle)
                    .expect("hidden retained piece")
                    .hidden()
            );
        }
        assert_eq!(
            runtime
                .actor(root.handle)
                .expect("resized root")
                .length_or_ordinal,
            3
        );
    });
}

#[test]
fn failed_second_growth_allocation_keeps_length_and_hides_first_allocation() {
    let mut session = source_session();
    let root_slot = construct(&mut session, Some(3));
    let free = session
        .economy
        .free_slots
        .last()
        .copied()
        .expect("one actual free source address");
    session
        .economy
        .economy
        .exhaust_new_event_capacity_for_test();
    session.economy.economy.free_slots.push(free);
    for row in &mut session.economy.entities {
        row.set_control_flags(row.control_flags & !crate::native_world::SOURCE_REPLACEABLE_FLAG);
    }
    session.economy.rebuild_free_arena(&[]);
    with_runtime(&mut session, |runtime| {
        let root = runtime.actor_at(root_slot).expect("root");
        let handles = chain(root.handle, runtime);
        resize(root.handle, 5, runtime);
        assert_eq!(chain(root.handle, runtime), handles);
        assert_eq!(
            runtime
                .actor_at(root_slot)
                .expect("retained root")
                .length_or_ordinal,
            3
        );
        let partial = &runtime.arena.entities[usize::from(free)];
        assert_eq!(
            partial.class, 0,
            "failed pair is not an invented live segment"
        );
        assert!(partial.drawing_disabled());
        assert!(
            !runtime.arena.free_slots.contains(&free),
            "the source first allocation was consumed"
        );
    });
}

#[test]
fn death_hides_whole_chain_without_reclaiming_in_callback() {
    let mut session = source_session();
    let root_slot = construct(&mut session, Some(3));
    let handles = with_runtime(&mut session, |runtime| {
        let mut root = runtime.actor_at(root_slot).expect("root");
        let handles = chain(root.handle, runtime);
        root.action = 181;
        runtime.write_actor(root);
        mc2::step(runtime, root_slot).expect("real mana/death path");
        for handle in &handles {
            assert!(
                runtime
                    .actor(*handle)
                    .expect("hidden row is retained until following pass")
                    .hidden()
            );
            assert!(!runtime.arena.free_slots.contains(&handle.source_slot));
        }
        handles
    });
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        handles.iter().all(|handle| {
            let row = &session.economy.entities[usize::from(handle.source_slot)];
            !row.active || session.economy.slot_generation(handle.source_slot) != handle.generation
        }),
        "ordinary following pass reclaims the hide-marked source chain"
    );
}

#[test]
fn old_callback_handle_cannot_write_a_reused_slot_and_orphan_piece_hides() {
    let mut session = source_session();
    let root_slot = construct(&mut session, Some(3));
    with_runtime(&mut session, |runtime| {
        let root = runtime.actor_at(root_slot).expect("root");
        let child = runtime
            .actor(root.next.expect("paired body"))
            .expect("first piece");
        runtime.arena.entities[usize::from(root_slot)].reclaim_source_row();
        runtime
            .arena
            .economy
            .release_slot(root_slot, Some(root.entity_rng));
        let allocation = runtime
            .allocate()
            .expect("same ordinary reclaimed source slot");
        assert_eq!(allocation.handle.source_slot, root_slot);
        assert_ne!(allocation.handle.generation, root.handle.generation);
        runtime.write_actor(root);
        assert_eq!(
            runtime.arena.entities[usize::from(root_slot)].class,
            0,
            "an old callback cannot overwrite the newborn generation"
        );
        position_segment(child.handle, runtime);
        assert!(
            runtime
                .actor(child.handle)
                .expect("retained orphan")
                .hidden()
        );
    });
}

#[test]
fn growth_uses_source_tail_byte_even_when_tail_is_root() {
    let mut session = source_session();
    let slot = construct(&mut session, Some(1));
    with_runtime(&mut session, |runtime| {
        let root = runtime.actor_at(slot).expect("one-row authored body");
        resize(root.handle, 3, runtime);
        let ordinals = chain(root.handle, runtime)
            .into_iter()
            .skip(1)
            .map(|handle| {
                runtime
                    .actor(handle)
                    .expect("source growth product")
                    .length_or_ordinal
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ordinals,
            [2, -2],
            "27720 uses abs(tail.byte46)+1 without a root special case"
        );
    });
}

#[test]
fn castle_return_uses_horizontal_distance_while_flying_above_castle() {
    let mut session = source_session();
    let player = session.session.state.live.core.players[0].entity_index;
    session
        .economy_mut()
        .build_castle(
            Position {
                x: 0x6000,
                y: 0x6000,
                z: 0,
            },
            player,
        )
        .expect("ordinary player-owned Castle constructor");
    let root_slot = construct(&mut session, Some(3));
    with_runtime(&mut session, |runtime| {
        let player_view = runtime
            .player(player)
            .expect("real player and constructed Castle");
        assert_ne!(player_view.castle_slot, 0);
        let mut root = runtime.actor_at(root_slot).expect("root");
        root.position = Position {
            z: player_view.castle_position.z.wrapping_add(1024),
            ..player_view.castle_position
        };
        root.owner = player;
        root.phase = 0;
        root.speed = root.maximum_speed;
        root.action = 178;
        runtime.write_actor(root);
        return_to_castle(root.handle, runtime);
        let root = runtime.actor(root.handle).expect("returning root");
        assert_eq!(
            root.action, 179,
            "source58490 measures XY only, allowing aerial deposit"
        );
        assert_eq!(root.dword10, PROPERTIES.shrink_delay);
    });
}

#[test]
fn proximity_prepass_propagates_prior_countdown_to_pieces_then_activates_nearby() {
    let mut session = source_session();
    let slot = construct(&mut session, Some(3));
    with_runtime(&mut session, |runtime| {
        let mut root = runtime.actor_at(slot).expect("root");
        let handles = chain(root.handle, runtime);
        let rng = root.entity_rng;
        root.cadence_counter = 2;
        runtime.write_actor(root);
        runtime
            .arena
            .step_raw_creature_proximity(&[slot], root.position);
        assert_eq!(runtime.actor_at(slot).expect("root").cadence_counter, 1);
        for handle in &handles[1..] {
            assert_eq!(
                runtime.actor(*handle).expect("piece").cadence_counter,
                2,
                "68C70 propagates the root's prior value before decrement"
            );
        }
        root = runtime.actor_at(slot).expect("root");
        root.cadence_counter = 0;
        runtime.write_actor(root);
        runtime
            .arena
            .step_raw_creature_proximity(&[slot], root.position);
        assert_eq!(runtime.actor_at(slot).expect("root").cadence_counter, 16);
        for handle in &handles[1..] {
            assert_eq!(runtime.actor(*handle).expect("piece").cadence_counter, 18);
        }
        assert_eq!(runtime.actor_at(slot).expect("root").entity_rng, rng);
    });
}

#[test]
fn owner7_authored_body_runs_contact_motion_and_semantic_consumers_in_one_arena() {
    use crate::native_world::{
        Level2EconomyState, MapType, OwnerPackageProfile, OwnerPackageSchema, parse_entities,
        parse_players, parse_presentation, parse_runtime, parse_terrain,
    };
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    );
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        root.join("level7"),
        crate::campaign::spec::CampaignSelection::ordinary(7).expect("ordinary mission7"),
    )
    .expect("unmodified actual owner7 descriptor");
    let entities = parse_entities(package.member("entities.bin").unwrap())
        .unwrap()
        .into_boxed_slice();
    let players = parse_players(
        package.member("players.bin").unwrap(),
        OwnerPackageProfile::Level2,
        OwnerPackageSchema::V5,
    )
    .unwrap();
    let mut source_runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation = parse_presentation(package.member("presentation.bin").unwrap()).unwrap();
    let mut terrain = crate::native_world::SessionTerrain::from_planes(
        MapType::Night,
        parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        0,
    );
    let player = entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .expect("actual owner7 player");
    let player_slot = player.index;
    let economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
        terrain.clone(),
        &entities,
        player.mana,
        player.model,
        0,
        0,
        source_runtime.random_state,
        Default::default(),
        source_runtime.constructor_counters(),
    )
    .unwrap();
    let mut arena = crate::campaign::source_arena::SourceArena::new(entities, economy);
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|feature| feature.index == 218 && feature.kind == 5 && feature.subtype == 22)
        .expect("actual source row218");
    assert_eq!((feature.activation_group, feature.parameter_1), (5, 15));
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let counter = source_runtime.constructor_counters().unwrap()[22];
    let families = std::array::from_fn(|_| Vec::new());
    let mut runtime = mc2::Context {
        arena: &mut arena,
        runtime: &mut source_runtime,
        terrain: &mut terrain,
        particles: &presentation.particles,
        players: &players,
        families: &families,
        class3: &[player_slot],
        outcome: Default::default(),
        error: None,
    };
    let slot = mc2::allocate_authored(
        &mut runtime,
        position,
        PROPERTIES.max_life,
        feature.parameter_1 as u8,
        None,
        feature.index,
    )
    .unwrap()
    .expect("ordinary actual owner constructor");
    let root = runtime.actor_at(slot).expect("actual root");
    let handles = chain(root.handle, &runtime);
    assert_eq!(
        handles.len(),
        15,
        "actual authored length creates seven pairs"
    );
    assert_eq!(root.phase, counter);
    assert_eq!(
        runtime.runtime.constructor_counters().unwrap()[22],
        counter.wrapping_add(1)
    );
    assert_eq!(root.position.x, position.x);
    assert_eq!(root.position.y, position.y);
    crate::native_world::area_damage::contact(runtime.arena, None, slot, 200, player_slot).unwrap();
    let contacted = runtime.actor_at(slot).unwrap();
    assert_eq!(
        (contacted.hit_source, contacted.damage),
        (player_slot, 200),
        "real Contact writes this exact raw mailbox"
    );
    let life = root.life;
    mc2::step(&mut runtime, slot).unwrap();
    let moved = runtime.actor_at(slot).unwrap();
    assert_eq!(moved.hit_source, 0);
    assert_eq!(
        moved.life, life,
        "ordinary model22 Contact steers and accelerates without generic damage subtraction"
    );
    assert!(moved.speed > moved.maximum_speed);
    for handle in &handles[1..] {
        mc2::step(&mut runtime, handle.source_slot).unwrap();
    }
    let bindings = std::collections::BTreeMap::from([(
        feature.index,
        crate::level_definition::CreatureKind::Creature22,
    )]);
    let census = runtime
        .arena
        .creature_runtime_census(&Default::default(), &bindings);
    assert_eq!(
        census
            .roots_of_kind(crate::level_definition::CreatureKind::Creature22)
            .count(),
        1
    );
    for handle in &handles {
        let actor = runtime.actor(*handle).unwrap();
        let contact_view =
            crate::native_world::area_damage::source_actor(runtime.arena, None, handle.source_slot)
                .unwrap();
        assert_eq!(contact_view.position, actor.position);
        assert_eq!(
            runtime.arena.entities[usize::from(handle.source_slot)].particle_parameter_index,
            Some(actor.visual_index)
        );
    }
    eprintln!(
        "owner7 authored={} group={} root={} parts={} phase={} contact={} speed={}",
        feature.index,
        feature.activation_group,
        slot,
        handles.len() - 1,
        counter,
        player_slot,
        moved.speed
    );
    runtime
        .finish()
        .expect("all actual owner source/particle/map operations succeeded");
}

#[test]
fn source_colour_row_five_and_signed_pair_angles_are_preserved() {
    assert_eq!(colour_index(52, 11, 3), Some(55));
    assert_eq!(colour_index(52, 11, 4), Some(53));
    assert_eq!(segment_offset(0, 0, 1, 15), 0);
    assert_eq!(segment_offset(0, 0, -1, 15), 1024);
    assert_eq!(segment_offset(100, 2, 3, 15), 300);
    assert_eq!(segment_offset(100, 2, -3, 15), 724);
    assert!(
        !enabled(182),
        "enabled table address has no source dispatch case"
    );
}
