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
fn constructor_and_motion_publish_source_proposals_but_hold_preserves_them() {
    let mut session = source_session();
    let slot = construct(&mut session, None);
    with_runtime(&mut session, |runtime| {
        let mut actor = runtime.actor_at(slot).expect("actual constructed root");
        assert_eq!(
            runtime.terrain().source_predicted_position(),
            actor.position,
            "4CA00 publishes its ground-adjusted constructor position"
        );
        let original = actor.position;
        let marker = Position {
            x: 123,
            y: 456,
            z: 78,
        };
        runtime.terrain().set_source_predicted_position(marker);
        let blocked = generic_move(&mut actor, runtime.terrain());
        let proposal = runtime.terrain().source_predicted_position();
        assert_ne!(proposal, marker, "1B8C0 publishes a real motion proposal");
        if blocked {
            let mut last = original;
            last.z = last.z.max(runtime.terrain().ground_height(last));
            crate::move_entity(&mut last, actor.yaw, 0, actor.speed);
            assert_eq!(proposal, last, "a blocked move retains its final proposal");
            assert_eq!(actor.position, original);
        } else {
            assert_eq!(proposal, actor.position);
        }
        actor.control_flags |= 0x800;
        let held = actor.position;
        runtime.terrain().set_source_predicted_position(marker);
        assert!(!generic_move(&mut actor, runtime.terrain()));
        assert_eq!(actor.position, held);
        assert_eq!(runtime.terrain().source_predicted_position(), marker);
        assert_eq!(actor.control_flags & 0x800, 0);
    });
    let paired = construct(&mut session, Some(5));
    with_runtime(&mut session, |runtime| {
        let root = runtime.actor_at(paired).expect("paired root");
        let last = *chain(root.handle, runtime)
            .last()
            .expect("last paired segment");
        assert_eq!(
            runtime.terrain().source_predicted_position(),
            runtime.actor(last).unwrap().position,
            "4CB60 leaves its final 271D0 segment proposal"
        );
        let marker = Position {
            x: 123,
            y: 456,
            z: 78,
        };
        runtime.terrain().set_source_predicted_position(marker);
        position_segment(last, runtime);
        assert_eq!(
            runtime.terrain().source_predicted_position(),
            runtime.actor(last).unwrap().position
        );
    });
}

#[test]
fn possession_changes_chain_colour_and_shrink_hides_exact_pair() {
    let mut session = source_session();
    let root_slot = construct(&mut session, Some(5));
    with_runtime(&mut session, |runtime| {
        let player = runtime
            .class3
            .to_vec()
            .into_iter()
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
    let castle = session
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
    // The Castle constructor does not itself publish the player's A4 link;
    // the next ordinary source pre-dispatch does so.
    session.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "{:?}",
        session.dispatch_events()
    );
    let player_state = session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .find(|entry| entry.entity_index == player)
        .unwrap();
    assert_eq!(
        u16::from_le_bytes([player_state.record[42], player_state.record[43]]),
        castle
    );
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
fn configured_owned_root_deposits_to_its_current_nonlocal_typed_castle() {
    let mut session = source_session();
    let local = session.state.entity_index;
    let rival = session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .find(|player| player.entity_index != 0 && player.entity_index != local)
        .expect("package retains a rival player")
        .entity_index;
    let local_position = Position {
        x: 0x3000,
        y: 0x3000,
        z: 0,
    };
    let rival_position = Position {
        x: 0x5000,
        y: 0x5000,
        z: 0,
    };
    let local_castle = session
        .economy_mut()
        .build_castle(local_position, local)
        .expect("configured local Castle allocation");
    let rival_castle = session
        .economy_mut()
        .build_castle(rival_position, rival)
        .expect("configured rival Castle allocation");
    assert_ne!(local_castle, rival_castle);
    let rival_current_position = session
        .economy()
        .castle_entry(rival_castle)
        .unwrap()
        .position;
    {
        let live = &mut session.session.state.live;
        for player in &mut live.core.players {
            let castle = if player.entity_index == local {
                local_castle
            } else if player.entity_index == rival {
                rival_castle
            } else {
                continue;
            };
            player.record[42..44].copy_from_slice(&castle.to_le_bytes());
        }
        let local = live.economy.economy.castle_entry_mut(local_castle).unwrap();
        local.mana = 400;
        local.mana_capacity = 5000;
        let rival = live.economy.economy.castle_entry_mut(rival_castle).unwrap();
        rival.mana = 100;
        rival.mana_capacity = 5000;
    }
    let root = construct(&mut session, Some(1));
    with_runtime(&mut session, |runtime| {
        let owner = runtime.player(rival).expect("current rival A4 address");
        assert_eq!(
            (owner.castle_slot, owner.castle_position, owner.castle_mana),
            (rival_castle, rival_current_position, 100)
        );
        let mut actor = runtime.actor_at(root).unwrap();
        actor.owner = rival;
        actor.action = 179;
        actor.phase = 0;
        actor.dword10 = 0;
        actor.mana = 750;
        runtime.write_actor(actor);
        shrink_into_castle(actor.handle, runtime);
        assert!(runtime.actor(actor.handle).unwrap().hidden());
        assert_eq!(
            runtime
                .arena
                .economy
                .castle_entry(rival_castle)
                .unwrap()
                .mana,
            850
        );
        assert_eq!(
            runtime
                .arena
                .economy
                .castle_entry(local_castle)
                .unwrap()
                .mana,
            400
        );
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
fn level_seven_real_group_five_body_accepts_public_carried_possession() {
    use crate::PlayerCommand;
    use crate::campaign::source_arena::{CreatureRootOrigin, CreatureRuntimeRole};
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::native_world::level2_dynamic_economy::{
        PossessionSourceReceipt, start_possession_trace, take_possession_trace,
    };
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    let package = std::env::var_os("MC_LEVEL7_HOUSE_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/settlement-20261105-integration/owner-collision-closure-v4/level7"),
        PathBuf::from,
    );
    let mut campaign = CampaignSession::open(package, CampaignSelection::ordinary(7).unwrap())
        .expect("whole Level7 owner package opens without filters");
    let session = campaign.simulation_mut();
    let player = session.state.entity_index;
    assert_eq!(player, 343);
    let player_handle = session
        .source_arena()
        .handle(player)
        .expect("actual player generation");
    let switch = session.entities()[194].clone();
    assert_eq!(
        (switch.class, switch.model, switch.action, switch.source_id),
        (11, 0, 0, 5)
    );
    assert_eq!((switch.position.x, switch.position.y), (38016, 6016));
    assert!(switch.active && !switch.drawing_disabled());
    assert!(
        session
            .source_arena()
            .creature_runtime_census(&BTreeMap::new(), &BTreeMap::new())
            .entries()
            .iter()
            .all(|entry| entry.origin
                != CreatureRootOrigin::AuthoredRecord {
                    authored_record: 218
                })
    );

    // Explicit prior-campaign carry and player-only resources/position/aim.
    // The real holder, source switch, paired actor, public cast and all
    // subsequent actor callbacks remain under their ordinary owners.
    session.apply_carried_spell_families(&[1], Some(1)).unwrap();
    let holder = session.shared_spells().local_player.enabled_spell_holders[1];
    assert_ne!(holder, 0, "the declared carry materializes its real holder");
    let holder_handle = session
        .source_arena()
        .handle(holder)
        .expect("held spell generation");
    assert_eq!(
        session
            .shared_spells()
            .pickup(holder)
            .unwrap()
            .source_spell_parent_entity_index(),
        player
    );
    {
        let live = &mut session.session.state.live;
        live.shared_spells.set_local_player_mana_for_test(10_000);
        let raw = &mut live.economy.entities[usize::from(player)];
        raw.set_mana(10_000);
        raw.set_mana_regen(0);
        let raw = raw.clone();
        live.shared_spells.sync_player_entity_from_authority(&raw);
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
    }

    let mut root = None;
    let mut group_seen = false;
    for _ in 0..16 {
        session
            .set_level2_entity_position(player, switch.position)
            .unwrap();
        session.step_outer_turn(PlayerCommand::default(), 0);
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "{:?}",
            session.dispatch_events()
        );
        group_seen |= session.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 194, group: 5, authored_indices, .. }
                if authored_indices.contains(&218)));
        root = session
            .source_arena()
            .creature_runtime_census(&BTreeMap::new(), &BTreeMap::new())
            .entries()
            .iter()
            .find(|entry| {
                entry.role == CreatureRuntimeRole::Root
                    && entry.origin
                        == CreatureRootOrigin::AuthoredRecord {
                            authored_record: 218,
                        }
            })
            .map(|entry| entry.handle);
        if group_seen && root.is_some() {
            break;
        }
    }
    let root = root.expect("ordinary switch194 group5 constructs authored row218");
    assert!(group_seen, "real switch194 must publish group5");
    let arena = session.source_arena();
    let first = mc2::read_actor(arena, root).expect("actual source root");
    assert_eq!(
        (first.length_or_ordinal, first.life, first.mana, first.owner),
        (15, 2000, 1000, 0)
    );
    let mut body = Vec::new();
    let mut next = Some(root);
    while let Some(handle) = next {
        assert!(
            !body.contains(&handle) && body.len() < 15,
            "source chain must not cycle"
        );
        let actor = mc2::read_actor(arena, handle).expect("current-generation chain actor");
        if handle != root {
            assert_eq!((actor.action, actor.root), (180, Some(root)));
        }
        body.push(handle);
        next = actor.next;
    }
    assert_eq!(body.len(), 15, "authored length produces seven real pairs");
    let head_position = first.position;
    let head_height = first.visual_radius;
    let origin = Position {
        y: head_position.y.wrapping_add(1024),
        z: head_position.z.wrapping_add(256),
        ..head_position
    };
    session.set_level2_entity_position(player, origin).unwrap();
    let launch = Position {
        x: origin.x.wrapping_sub(256),
        z: origin
            .z
            .wrapping_add(session.entities()[usize::from(player)].array_fov()),
        ..origin
    };
    let target = Position {
        z: head_position.z.wrapping_add(head_height),
        ..head_position
    };
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, target);
    session.set_level2_player_aim_for_test(yaw, pitch);
    let before_flyers = session
        .economy()
        .possession_projectiles()
        .iter()
        .map(|row| {
            (
                row.source_entity_index,
                session.economy().slot_generation(row.source_entity_index),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut impacts = Vec::new();
    let mut accepted = false;
    let mut owned = false;
    let mut saw_sound = false;
    let audio_start = session.effect_journal().records().len();
    let player_source_id = session.entities()[usize::from(player)].source_id;
    let (mut ownership_audio, mut oscillation_audio) = (false, false);
    let mut moved = false;
    let mut recoloured = false;
    let mut checked_direct_selector = false;
    let color_index = session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .find(|entry| entry.entity_index == player)
        .unwrap()
        .player_color_index();
    let colour_base = 105 + 8 * [0_u16, 1, 4, 3, 2, 5, 7, 6][usize::from(color_index)];
    for turn in 0..64 {
        start_possession_trace();
        session.step_outer_turn(
            if turn == 0 {
                PlayerCommand {
                    opcode: 6,
                    argument_1: 1,
                    flags: 0x40,
                    ..PlayerCommand::default()
                }
            } else {
                PlayerCommand::default()
            },
            0,
        );
        let receipts = take_possession_trace();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, Mc2DispatchEvent::Error { .. })),
            "turn {turn}: {:?}",
            session.dispatch_events()
        );
        for receipt in receipts {
            match receipt {
                PossessionSourceReceipt::ImpactBirth {
                    effect,
                    projectile,
                    holder: birth_holder,
                    owner,
                    packet,
                } if birth_holder == holder_handle
                    && owner == player_handle
                    && !before_flyers
                        .contains(&(projectile.source_slot, projectile.generation))
                    && (packet.source_class, packet.source_model, packet.action)
                        == (10, 12, 12)
                    && packet.source_id_entity_index == player
                    && packet.projectile_entity_index == projectile.source_slot =>
                {
                    impacts.push(effect);
                }
                PossessionSourceReceipt::MailboxWrite {
                    effect,
                    target,
                    source_id,
                    amount,
                    possessor,
                    ..
                } if impacts.contains(&effect)
                    && body.contains(&target)
                    && source_id == player
                    && amount == 0
                    && possessor == player =>
                {
                    accepted = true;
                }
                _ => {}
            }
        }
        saw_sound |= session.economy().events().iter().any(|event| matches!(event,
            Level2EconomyEvent::PreparedSound { entity_index, sound_id: 4, .. } if *entity_index == player));
        for record in &session.effect_journal().records()[audio_start..] {
            if let crate::campaign::journal::CampaignEffect::Flight(
                crate::FlightEvent::PrepareSound {
                    audio_site,
                    entity_index,
                    player_index,
                    sound_id,
                },
            ) = &record.effect
            {
                if let Some(audio) = record.audio {
                    if *audio_site == crate::MissionAudioSite::Creature22Ownership
                        && *entity_index == player
                        && *player_index == -1
                        && *sound_id == 4
                        && audio.play_type == 1
                        && audio.command.audible
                        && audio.command.distance_xy == 0
                        && (
                            audio.command.relative_x,
                            audio.command.relative_y,
                            audio.command.relative_z,
                        ) == (0, 0, 0)
                        && audio.command.sound_flags == player_source_id as i16
                    {
                        ownership_audio = true;
                    }
                    if *audio_site == crate::MissionAudioSite::Creature22Oscillation
                        && *entity_index == root.source_slot
                        && *player_index == -1
                        && *sound_id == 48
                        && audio.play_type == 1
                        && audio.command.audible
                        && audio.command.sound_flags == first.source_id as i16
                        && session.source_arena().resolve(root).is_some()
                    {
                        oscillation_audio = true;
                    }
                }
            }
        }
        let current = mc2::read_actor(session.source_arena(), root)
            .expect("original root generation remains current during possession");
        if current.animation_frame != 0 {
            let view = session
                .campaign_entity_presentations(0)
                .unwrap()
                .into_iter()
                .find(|view| view.entity_index == root.source_slot)
                .expect("live authored creature root reaches the source presentation");
            assert_eq!(
                view.render_mode, 21,
                "owner particle is a direct-selector mode"
            );
            assert_eq!(
                crate::native_world::effects::animation_frame(
                    session.source_arena(),
                    root.source_slot,
                    view.render_parameter_index
                ),
                current.animation_frame,
                "the actual source carrier exposes its nonzero oscillation"
            );
            assert_eq!(
                view.selected_tmap, view.base_tmap,
                "source mode21 selects the base TMAP even while the actor oscillates"
            );
            checked_direct_selector = true;
        }
        owned |= current.owner == player && matches!(current.action, 177 | 178);
        moved |= current.position != head_position;
        if current.owner == player && current.action == 178 {
            recoloured |= body.iter().all(|&handle| {
                mc2::read_actor(session.source_arena(), handle).is_some_and(|actor| {
                    let ordinal = if handle == root {
                        0
                    } else {
                        actor.length_or_ordinal.unsigned_abs()
                    };
                    ordinal <= 7 && actor.visual_index == colour_base + 7 - u16::from(ordinal)
                })
            });
        }
        if accepted
            && owned
            && saw_sound
            && moved
            && recoloured
            && ownership_audio
            && oscillation_audio
        {
            break;
        }
    }
    assert!(
        accepted
            && owned
            && saw_sound
            && moved
            && recoloured
            && ownership_audio
            && oscillation_audio,
        "real carried public Possession reaches original body/owner/sound/motion/colour: accepted={accepted} owned={owned} sound={saw_sound} moved={moved} recoloured={recoloured} ownership_audio={ownership_audio} oscillation_audio={oscillation_audio} impacts={impacts:?}"
    );
    assert!(
        checked_direct_selector,
        "the real scheduled root must reach a nonzero oscillation"
    );
    assert!(
        session
            .campaign_entity_presentations(0)
            .unwrap()
            .iter()
            .any(|view| view.entity_index == root.source_slot)
    );
}

#[test]
fn configured_short_body_public_possession_grows_and_returns_to_castle() {
    use crate::PlayerCommand;
    use crate::campaign::{
        CampaignSession,
        spec::{CampaignSelection, DecodedCampaignPackage},
    };
    use crate::native_world::Mc2DispatchEvent;
    use crate::native_world::level2_dynamic_economy::{
        PossessionSourceReceipt, start_possession_trace, take_possession_trace,
    };
    use crate::native_world::source_target::{begin_current_mana_writes, take_current_mana_writes};
    use std::{collections::BTreeSet, path::PathBuf};

    let path = std::env::var_os("MC_LEVEL7_HOUSE_PACKAGE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/settlement-20261105-integration/owner-collision-closure-v4/level7"),
        PathBuf::from,
    );
    let package =
        DecodedCampaignPackage::decode(&path, CampaignSelection::ordinary(7).unwrap()).unwrap();
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|f| f.index == 218 && f.kind == 5 && f.subtype == 22)
        .expect("actual Level7 row218");
    assert_eq!((feature.activation_group, feature.parameter_1), (5, 15));
    let mut campaign =
        CampaignSession::open(path, CampaignSelection::ordinary(7).unwrap()).unwrap();
    let session = campaign.simulation_mut();
    let player = session.state.entity_index;
    let player_handle = session.source_arena().handle(player).unwrap();
    let base = feature.world_position(0);
    let position = feature.world_position(session.session.state.live.terrain.ground_height(base));

    // Declared prior spell carry, player resources and a real Castle prerequisite.
    // Short length is an unshipped constructor INPUT, not a claim about row218.
    session.apply_carried_spell_families(&[1], Some(1)).unwrap();
    let holder = session.shared_spells().local_player.enabled_spell_holders[1];
    let holder_handle = session
        .source_arena()
        .handle(holder)
        .expect("real carried holder");
    assert_eq!(
        session
            .shared_spells()
            .pickup(holder)
            .unwrap()
            .source_spell_parent_entity_index(),
        player
    );
    {
        let live = &mut session.session.state.live;
        live.player_life = 200_000;
        live.shared_spells.set_player_entity_life(200_000);
        live.shared_spells.set_local_player_mana_for_test(10_000);
        let raw = &mut live.economy.entities[usize::from(player)];
        raw.set_life(200_000);
        raw.set_mana(10_000);
        raw.set_mana_regen(0);
        let raw = raw.clone();
        live.shared_spells.sync_player_entity_from_authority(&raw);
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
    }
    // Longer than 1024 maximum-speed visits, leaving the real growth countdown time to run.
    let castle_position = Position {
        x: position.x.wrapping_add(18_432),
        ..position
    };
    let castle = session
        .economy_mut()
        .build_castle(castle_position, player)
        .expect("real player Castle");
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|e| matches!(e, Mc2DispatchEvent::Error { .. })),
        "{:?}",
        session.dispatch_events()
    );
    let record = &session
        .session
        .state
        .live
        .core
        .players
        .iter()
        .find(|p| p.entity_index == player)
        .unwrap()
        .record;
    assert_eq!(u16::from_le_bytes([record[42], record[43]]), castle);
    let castle_handle = session
        .source_arena()
        .handle(castle)
        .expect("Castle generation");
    let slot = with_runtime(session, |runtime| {
        mc2::allocate_authored(
            runtime,
            position,
            PROPERTIES.max_life,
            3,
            None,
            feature.index,
        )
        .unwrap()
        .expect("actual authored constructor")
    });
    let root = session.source_arena().handle(slot).unwrap();
    let first = mc2::read_actor(session.source_arena(), root).unwrap();
    assert_eq!(
        (
            first.length_or_ordinal,
            first.life,
            first.mana,
            first.owner,
            first.target_or_growth
        ),
        (3, 2000, 1000, 0, 1024)
    );
    let body = with_runtime(session, |runtime| chain(root, runtime));
    assert_eq!(body.len(), 3);
    let origin = Position {
        y: first.position.y.wrapping_add(1024),
        z: first.position.z.wrapping_add(256),
        ..first.position
    };
    session.set_level2_entity_position(player, origin).unwrap();
    let launch = Position {
        x: origin.x.wrapping_sub(256),
        z: origin
            .z
            .wrapping_add(session.entities()[usize::from(player)].array_fov()),
        ..origin
    };
    let target = Position {
        z: first.position.z.wrapping_add(first.visual_radius),
        ..first.position
    };
    let (yaw, pitch) = crate::level1_mission::projectile_guidance_angles(launch, target);
    session.set_level2_player_aim_for_test(yaw, pitch);
    let before_flyers = session
        .economy()
        .possession_projectiles()
        .iter()
        .map(|p| {
            (
                p.source_entity_index,
                session.economy().slot_generation(p.source_entity_index),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut impacts = Vec::new();
    let (mut accepted, mut owned) = (false, false);
    for turn in 0..64 {
        start_possession_trace();
        session.step_outer_turn(
            if turn == 0 {
                PlayerCommand {
                    opcode: 6,
                    argument_1: 1,
                    flags: 0x40,
                    ..PlayerCommand::default()
                }
            } else {
                PlayerCommand::default()
            },
            0,
        );
        for receipt in take_possession_trace() {
            match receipt {
                PossessionSourceReceipt::ImpactBirth {
                    effect,
                    projectile,
                    holder: h,
                    owner,
                    packet,
                } if h == holder_handle
                    && owner == player_handle
                    && !before_flyers
                        .contains(&(projectile.source_slot, projectile.generation))
                    && (packet.source_class, packet.source_model, packet.action)
                        == (10, 12, 12)
                    && packet.source_id_entity_index == player
                    && packet.projectile_entity_index == projectile.source_slot =>
                {
                    impacts.push(effect)
                }
                PossessionSourceReceipt::MailboxWrite {
                    effect,
                    target,
                    source_id,
                    amount,
                    possessor,
                    ..
                } if impacts.contains(&effect)
                    && body.contains(&target)
                    && source_id == player
                    && amount == 0
                    && possessor == player =>
                {
                    accepted = true
                }
                _ => {}
            }
        }
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|e| matches!(e, Mc2DispatchEvent::Error { .. })),
            "turn {turn}: {:?}",
            session.dispatch_events()
        );
        let actor = mc2::read_actor(session.source_arena(), root).expect("original root retained");
        owned |= actor.owner == player && actor.action == 178;
        if accepted && owned {
            break;
        }
    }
    assert!(
        accepted && owned,
        "public Possession must reach the original body: accepted={accepted}, owned={owned}, impacts={impacts:?}"
    );
    let (mut grown, mut returned, mut hidden, mut deposit) = (None, false, false, None);
    let mut lengths = Vec::new();
    let mut other_castle_writes = Vec::new();
    let target_handle = session.source_arena().handle(372);
    eprintln!(
        "configured growth initial Castle={castle_handle:?} root={root:?} body={body:?} target372={target_handle:?} carrier={:?} view={:?}",
        target_handle.and_then(|handle| session.source_arena().resolve(handle)),
        crate::native_world::source_target::read(session.source_arena(), Default::default(), 372),
    );
    // Natural movement, growth, Castle approach and the source 128-visit shrink delay.
    for turn in 0..3072 {
        let before =
            mc2::read_actor(session.source_arena(), root).expect("root retained before hide");
        let castle_before = session.source_arena().handle(castle);
        begin_current_mana_writes();
        session.step_outer_turn(PlayerCommand::default(), 0);
        let writes = take_current_mana_writes();
        assert!(
            !session
                .dispatch_events()
                .iter()
                .any(|e| matches!(e, Mc2DispatchEvent::Error { .. })),
            "turn {turn}: {:?}, target372={:?}, carrier={:?}, view={:?}",
            session.dispatch_events(),
            session.source_arena().handle(372),
            session
                .source_arena()
                .handle(372)
                .and_then(|handle| session.source_arena().resolve(handle)),
            crate::native_world::source_target::read(
                session.source_arena(),
                Default::default(),
                372
            ),
        );
        let actor = mc2::read_actor(session.source_arena(), root)
            .expect("hidden root retained in source pass");
        if before.length_or_ordinal == 3 && actor.length_or_ordinal == 5 {
            assert!(
                session
                    .cf10_production_schedule_receipts()
                    .iter()
                    .any(|receipt| matches!(
                        receipt,
                        crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                            if *slot == root.source_slot
                    )),
                "growth is produced by the actual root callback"
            );
            assert_eq!((before.target_or_growth, actor.target_or_growth), (0, 1024));
            assert_eq!(actor.mana, before.mana + PROPERTIES.mana_growth);
            let current = with_runtime(session, |runtime| chain(root, runtime));
            assert_eq!(current.len(), 5);
            assert!(body.iter().all(|handle| current.contains(handle)));
            let added = current
                .iter()
                .filter(|handle| !body.contains(handle))
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(added.len(), 2);
            let mut ordinals = added
                .iter()
                .map(|handle| {
                    let part = mc2::read_actor(session.source_arena(), *handle).unwrap();
                    assert_eq!((part.action, part.root), (180, Some(root)));
                    part.length_or_ordinal
                })
                .collect::<Vec<_>>();
            ordinals.sort_unstable();
            assert_eq!(ordinals, [-2, 2]);
            assert!(
                session
                    .campaign_entity_presentations(0)
                    .unwrap()
                    .iter()
                    .any(|view| view.entity_index == root.source_slot)
            );
            grown = Some(current);
        }
        if before.length_or_ordinal != actor.length_or_ordinal {
            lengths.push((before.length_or_ordinal, actor.length_or_ordinal));
        }
        returned |= actor.action == 179;
        for write in writes {
            if write.slot != castle {
                continue;
            }
            if write.source != Some(root) {
                if other_castle_writes.is_empty() {
                    eprintln!("other real Castle mana writer: {write:?}");
                }
                other_castle_writes.push(write);
                continue;
            }
            assert_eq!(castle_before, Some(castle_handle));
            assert_eq!(session.source_arena().handle(castle), Some(castle_handle));
            assert!(session.source_arena().resolve(root).is_some());
            assert_eq!(
                (before.action, before.length_or_ordinal, before.dword10),
                (179, 1, 0)
            );
            assert_eq!(write.delta, before.mana);
            assert_eq!(
                write.after,
                write.before.wrapping_add(write.delta).min(write.capacity)
            );
            deposit = Some(write);
        }
        if actor.hidden() {
            hidden = true;
            assert!(session.source_arena().resolve(root).is_some());
            assert!(
                !session
                    .campaign_entity_presentations(0)
                    .unwrap()
                    .iter()
                    .any(|view| view.entity_index == root.source_slot)
            );
            break;
        }
    }
    let grown =
        grown.expect("ordinary callbacks consume the 1024 growth countdown and allocate a pair");
    assert!(
        returned && hidden && deposit.is_some(),
        "real Castle return/deposit/hide: returned={returned}, hidden={hidden}, deposit={deposit:?}, other_castle_writes={other_castle_writes:?}, lengths={lengths:?}"
    );
    assert!(
        lengths.contains(&(5, 3)) && lengths.contains(&(3, 1)),
        "natural pairwise shrink: {lengths:?}"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|e| matches!(e, Mc2DispatchEvent::Error { .. })),
        "{:?}",
        session.dispatch_events()
    );
    for handle in grown {
        assert!(
            session.source_arena().resolve(handle).is_none(),
            "original body generation reclaimed: {handle:?}"
        );
    }
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
