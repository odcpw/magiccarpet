use super::*;

#[test]
fn alliance_follows_retained_parent_and_expires_when_dead() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_four();
    let mut a4 = mission_four_a4();
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
    .unwrap();
    let terminal =
        crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
    let effect = &mut arena.entities[usize::from(terminal)];
    effect.source_id = player;
    effect.record[74] = 1;
    effect.subspell_index = 40;
    effect.record[154..156].copy_from_slice(&slot.to_le_bytes());
    let impact =
        crate::native_world::alliance_spell::step_impact(&mut arena, terminal, &[slot]).unwrap();
    assert_eq!(impact.affected_entities, [slot]);
    let hostile = arena
        .allocate_source_slot_with_terrain(&mut terrain)
        .unwrap();
    let mut enemy = crate::world::terrain::cave_features::new_event(
        hostile,
        17,
        Position {
            x: position.x.wrapping_add(512),
            ..position
        },
    );
    enemy.class = 3;
    enemy.model = 1;
    enemy.record[67] = 3;
    enemy.record[68] = 1;
    enemy.set_life(1000);
    arena.entities[usize::from(hostile)] = enemy;
    crate::native_world::area_damage::contact(&mut arena, None, slot, 1, hostile).unwrap();
    let parent = &mut arena.entities[usize::from(player)];
    parent.set_life(100);
    parent.record[154..156].copy_from_slice(&hostile.to_le_bytes());
    parent.record[42..44].fill(0);
    parent.reclaim_source_row();
    assert_eq!(parent.class, 0);
    assert_eq!(parent.life, 100);
    let families = std::array::from_fn(|model| {
        if model == usize::from(SOURCE_MODEL) {
            vec![slot]
        } else {
            Vec::new()
        }
    });
    for life in [100, 0] {
        let mut trial = arena.clone();
        let mut trial_runtime = runtime.clone();
        let mut trial_terrain = terrain.clone();
        trial.entities[usize::from(player)].set_life(life);
        let effects = step_fixture(
            &mut trial,
            &mut trial_runtime,
            &mut trial_terrain,
            &particles,
            slot,
            &[],
            &families,
            &mut a4,
        );
        let actor = &trial.entities[usize::from(slot)];
        assert_eq!(actor.record[77], if life > 0 { 14 } else { 10 });
        assert_eq!(
            u16::from_le_bytes([actor.record[44], actor.record[45]]),
            if life > 0 { player } else { 0 }
        );
        if life > 0 {
            assert_eq!(effects.earlier_feedback, Some((player, 19)));
            assert_eq!(effects.feedback, Some((player, 24)));
            assert_eq!(i16::from_le_bytes([actor.record[50], actor.record[51]]), 39);
        } else {
            assert_eq!(actor.source_word_96(), 0);
            assert_eq!(actor.control_flags & 0x0600_0000, 0);
        }
    }
}

use crate::campaign::source_arena::SourceArena;
use crate::native_world::{
    Level2EconomyState, ParticleParameter, RuntimeSnapshot, parse_entities, parse_presentation,
    parse_runtime, parse_terrain,
};

fn mission_four_root() -> std::path::PathBuf {
    std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES")
        .map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
            },
            std::path::PathBuf::from,
        )
        .join("level4")
}

fn mission_four_a4() -> (
    crate::native_world::Level2SharedSpellResetSession,
    Vec<crate::native_world::PlayerSnapshot>,
    crate::FlightState,
) {
    let campaign = crate::campaign::CampaignSession::open(
        mission_four_root(),
        crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
    )
    .unwrap();
    let sim = campaign.simulation();
    (
        sim.shared_spells().clone(),
        sim.players().to_vec(),
        sim.state.clone(),
    )
}

fn step_fixture(
    arena: &mut SourceArena,
    runtime: &mut RuntimeSnapshot,
    terrain: &mut SessionTerrain,
    particles: &[ParticleParameter],
    slot: u16,
    players: &[u16],
    models: &[Vec<u16>; 29],
    a4: &mut (
        crate::native_world::Level2SharedSpellResetSession,
        Vec<crate::native_world::PlayerSnapshot>,
        crate::FlightState,
    ),
) -> SpiderEffects {
    let base = crate::native_world::source_target::Context {
        map_type: Some(terrain.map_type()),
        particles: Some(particles),
        ..Default::default()
    };
    let mut feedback = crate::native_world::source_target::Creature23PlayerFeedback {
        shared: &mut a4.0,
        core_players: &mut a4.1,
        fire: None,
        local_state: &mut a4.2,
        domains: None,
    };
    mc2::step_row(
        arena,
        runtime,
        terrain,
        particles,
        slot,
        players,
        models,
        base,
        &mut feedback,
    )
    .unwrap()
}

pub(super) fn mission_four() -> (
    SourceArena,
    RuntimeSnapshot,
    SessionTerrain,
    Vec<ParticleParameter>,
    u16,
    crate::level1_mission::AuthoredFeature,
) {
    let root = std::env::var_os("MC_ORDINARY_CAMPAIGN_PACKAGES").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq6-ordinary-20261025-zclu5fjj")
        },
        std::path::PathBuf::from,
    );
    let package = crate::campaign::spec::DecodedCampaignPackage::decode(
        root.join("level4"),
        crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
    )
    .unwrap();
    let entities = parse_entities(package.member("entities.bin").unwrap())
        .unwrap()
        .into_boxed_slice();
    let runtime = parse_runtime(package.member("runtime.bin").unwrap()).unwrap();
    let presentation = parse_presentation(package.member("presentation.bin").unwrap()).unwrap();
    let terrain = SessionTerrain::from_planes(
        MapType::Cave,
        parse_terrain(package.member("terrain.bin").unwrap()).unwrap(),
        0,
    );
    let player = entities
        .iter()
        .find(|e| e.active && e.class == 3 && e.model == 0)
        .unwrap();
    let player_slot = player.index;
    let economy = Level2EconomyState::from_parts_with_castle_stage_footprints(
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
    let feature = crate::level1_mission::parse_authored_features(package.authored_bytes())
        .unwrap()
        .into_iter()
        .find(|f| f.index == 137)
        .unwrap();
    assert_eq!(
        (feature.kind, feature.subtype, feature.activation_group),
        (5, 20, 6)
    );
    (
        SourceArena::new(entities, economy),
        runtime,
        terrain,
        presentation.particles.to_vec(),
        player_slot,
        feature,
    )
}

#[test]
fn mission_four_record137_constructs_source_spider_and_live_consumers() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_four();
    let mut a4 = mission_four_a4();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let before = runtime.constructor_counters().unwrap()[20];
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
    .unwrap();
    let actor = mc2::load(
        &arena.entities[usize::from(slot)],
        arena.slot_generation(slot),
    );
    assert_eq!(
        (actor.life, actor.mana, actor.speed, actor.action),
        (5500, 2750, 32, 161)
    );
    assert_eq!(
        (actor.phase, runtime.constructor_counters().unwrap()[20]),
        (before, before.wrapping_add(1))
    );
    assert_eq!(
        (actor.visual, actor.half_width, actor.half_height),
        (288, 384, 512)
    );
    let bindings =
        std::collections::BTreeMap::from([(137, crate::level_definition::CreatureKind::Spider)]);
    assert_eq!(
        arena
            .creature_runtime_census(&Default::default(), &bindings)
            .roots_of_kind(crate::level_definition::CreatureKind::Spider)
            .count(),
        1
    );
    crate::native_world::area_damage::contact(&mut arena, None, slot, 200, player).unwrap();
    let families = std::array::from_fn(|model| if model == 20 { vec![slot] } else { Vec::new() });
    step_fixture(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player],
        &families,
        &mut a4,
    );
    let hit = mc2::load(
        &arena.entities[usize::from(slot)],
        arena.slot_generation(slot),
    );
    assert_eq!(
        (hit.life, hit.pending_attacker, hit.target, hit.action),
        (5300, 0, player, 162)
    );
    let contact_view =
        crate::native_world::area_damage::source_actor(&mut arena, None, slot).unwrap();
    assert_eq!(contact_view.position, hit.position);
    eprintln!(
        "mission4 authored137 group6 spider={slot} player={player} life={} action={} particle={}",
        hit.life, hit.action, hit.visual
    );
}

#[test]
fn web_charge_uses_live_status_and_source_contact_countdown() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_four();
    let mut a4 = mission_four_a4();
    a4.2.immobilize_counter = 1; // Explicit current-A4 configured status.
    let position = arena.entities[usize::from(player)].position;
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
    .unwrap();
    let row = &mut arena.entities[usize::from(slot)];
    row.set_source_action(162);
    row.set_phase(1); // No ranged callback: the actual A4 read still advances phase0.
    row.set_control_flags(row.control_flags | 0x800);
    row.record[154..156].copy_from_slice(&player.to_le_bytes());
    let models = std::array::from_fn(|model| if model == 20 { vec![slot] } else { Vec::new() });
    let effects = step_fixture(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player],
        &models,
        &mut a4,
    );
    assert!(effects.web_target.is_none() && effects.web_launched_target.is_none());
    let mut actor = mc2::load(
        &arena.entities[usize::from(slot)],
        arena.slot_generation(slot),
    );
    assert_eq!(actor.attack_phase, 1);
    let target = Target {
        slot: player,
        source_id: player,
        class: 3,
        model: 0,
        life: 10000,
        position: actor.position,
        ..Target::default()
    };
    let mut context = SpiderContext {
        terrain: &terrain,
        targets: &[target],
        class3: &[player],
        model20: &[],
        class5: &[],
        stage_runtime: None,
        sentinel: None,
        damage_trace: None,
    };
    actor.control_flags |= 0x800;
    actor.phase = 35;
    let effects = step(&mut actor, &mut context).unwrap();
    assert_eq!(effects.melee, Some((player, 100, actor.source_id)));
    assert_eq!(
        (actor.attack_phase, actor.attack_timer, actor.speed),
        (0, 31, 32)
    );
    actor.action = 166;
    assert!(
        step(&mut actor, &mut context).is_none(),
        "source166 has no dispatch case"
    );
}

#[test]
fn source_constructor_is_not_cave_gated_and_counter_wraps_once() {
    let mut counter = 255;
    let actor = construct(701, 3, 19, Position::default(), MapType::Day, &mut counter).unwrap();
    assert_eq!((counter, actor.phase), (0, 255));
    assert_eq!(actor.rng, level2_next_rng16(19));
    assert_eq!((actor.attack_timer, actor.visual), (1, 288));
    assert_eq!(actor.yaw, ((actor.rng & 2047) as i16).wrapping_sub(1));
}

#[test]
fn roaming_links_only_while_source_proximity_is_active() {
    let (_, _, terrain, _, _, _) = mission_four();
    let mut actor = construct(
        701,
        3,
        19,
        Position {
            x: 30000,
            y: 30000,
            z: 0,
        },
        MapType::Cave,
        &mut 0,
    )
    .unwrap();
    let peer = Target {
        slot: 702,
        source_id: 702,
        class: 5,
        model: 20,
        position: Position {
            x: 30256,
            ..actor.position
        },
        life: 5500,
        ..Target::default()
    };
    actor.yaw = heading_for_positions(actor.position, peer.position) as i16;
    actor.control_flags |= 0x800;
    actor.proximity = 0;
    let mut context = SpiderContext {
        terrain: &terrain,
        targets: &[peer],
        class3: &[],
        model20: &[702],
        class5: &[702],
        stage_runtime: None,
        sentinel: None,
        damage_trace: None,
    };
    step(&mut actor, &mut context).unwrap();
    assert_eq!((actor.action, actor.previous), (161, 0));
    actor.control_flags |= 0x800;
    actor.proximity = 1;
    step(&mut actor, &mut context).unwrap();
    assert_eq!((actor.action, actor.previous), (163, 702));
}

#[test]
fn mission_four_spider_death_mana_reclaim_and_reuse_preserve_lifetime() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_four();
    let mut a4 = mission_four_a4();
    // Group6 production occurs after the ordinary initial reclaim prologue.
    // Otherwise that later prologue also frees lower package slots (e.g.53),
    // and the source allocator correctly chooses those before this root.
    arena
        .rebuild_source_arena_with_terrain(&mut terrain, &[])
        .unwrap();
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
    .unwrap();
    let old = arena.handle(slot).unwrap();
    crate::native_world::area_damage::contact(&mut arena, None, slot, 5501, player).unwrap();
    let mut credit = false;
    let mut dropped = false;
    for _ in 0..24 {
        let families = arena.rebuilt_class5_model_slots();
        let result = step_fixture(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            slot,
            &[player],
            &families,
            &mut a4,
        );
        credit |= result.kill_credit == Some(player);
        if result.kill {
            dropped = true;
            break;
        }
        let row = &mut arena.entities[usize::from(slot)];
        row.set_phase(row.phase.wrapping_add(1));
    }
    assert!(dropped && credit);
    assert!(arena.entities[usize::from(slot)].drawing_disabled());
    let sphere = arena.spheres().iter().find(|s| s.mana == 2750).unwrap();
    assert_eq!(sphere.position, arena.entities[usize::from(slot)].position);
    assert!(
        arena.resolve(old).is_some(),
        "same-pass hide retains the source address"
    );
    arena
        .rebuild_source_arena_with_terrain(&mut terrain, &[])
        .unwrap();
    assert!(
        arena.resolve(old).is_none(),
        "following prologue reclaims the hidden root"
    );
    let reused = mc2::allocate_authored(
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
    assert_eq!(reused, slot);
    assert_ne!(arena.handle(reused).unwrap().generation, old.generation);
    assert!(arena.resolve(old).is_none());
}

#[test]
fn web_replacement_keeps_current_occupant_and_source_phase() {
    // Explicit allocator exhaustion is a failure-path boundary, not an
    // ordinary Mission4 play capture. Spider defaults do not set eligibility.
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_four();
    let mut a4 = mission_four_a4();
    let position = arena.entities[usize::from(player)].position;
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
    .unwrap();
    let old = arena.handle(slot).unwrap();
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    let row = &mut arena.entities[usize::from(slot)];
    row.set_control_flags(
        row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG | 0x800,
    );
    row.set_source_action(162);
    row.set_phase(35);
    row.record[154..156].copy_from_slice(&player.to_le_bytes());
    arena.rebuild_source_free_arena();
    let families = arena.rebuilt_class5_model_slots();
    let result = step_fixture(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player],
        &families,
        &mut a4,
    );
    assert_eq!(result.web_launched_target, Some(player));
    let row = &arena.entities[usize::from(slot)];
    assert_eq!((row.class, row.model, row.action), (9, 21, 22));
    assert_eq!(
        row.phase, slot as u8,
        "NewEvent's phase belongs to the replacement; tail has not run"
    );
    assert_eq!(
        row.actual_speed,
        u16::from_le_bytes([row.record[136], row.record[137]]) as i16
    );
    assert!(arena.resolve(old).is_none());
    assert_ne!(arena.slot_generation(slot), old.generation);
}

#[test]
fn mission_four_scheduler_advances_self_replacing_web_current_occupant() {
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
    let particles = session.session.definition.presentation.particles.clone();
    let (_, _, _, _, _, feature) = mission_four();
    let (slot, old) = {
        let live = &mut session.session.state.live;
        // Drain initial hidden imported rows before imposing the explicit
        // full-arena boundary; the real pass must not regain their slots.
        live.economy
            .rebuild_source_arena_with_terrain(&mut live.terrain, &[])
            .unwrap();
        let player = live.core.state.entity_index;
        let position = live.economy.entities[usize::from(player)].position;
        let slot = mc2::allocate_authored(
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
        let old = live.economy.handle(slot).unwrap();
        for row in &mut live.economy.entities {
            row.set_control_flags(
                row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG,
            );
        }
        live.economy.economy.exhaust_new_event_capacity_for_test();
        let row = &mut live.economy.entities[usize::from(slot)];
        row.set_control_flags(
            row.control_flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG | 0x800,
        );
        row.set_source_action(162);
        row.set_phase(35);
        row.record[154..156].copy_from_slice(&player.to_le_bytes());
        live.economy.rebuild_source_free_arena();
        (slot, old)
    };
    // One real source pass owns the replacement's common tail. Exhaustion
    // and attack cadence are diagnostic boundaries, not earned gameplay.
    session.step_level2_source_pass(0);
    let arena = &session.session.state.live.economy;
    let row = &arena.entities[usize::from(slot)];
    assert_eq!(
        (row.class, row.model, row.action, row.phase),
        (9, 21, 22, (slot as u8).wrapping_add(1))
    );
    assert_ne!(arena.slot_generation(slot), old.generation);
    assert!(arena.resolve(old).is_none());
    assert!(
        !session
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
    );
}

type SpiderGame = crate::game_session::GameSession<crate::campaign::backend::Mc2CampaignBackend>;

fn simulation(game: &SpiderGame) -> &crate::native_world::Mc2Simulation {
    game.backend().compatibility_campaign().simulation()
}

fn simulation_mut(game: &mut SpiderGame) -> &mut crate::native_world::Mc2Simulation {
    game.backend_mut()
        .compatibility_campaign_mut_for_test()
        .simulation_mut()
}

fn no_dispatch_errors(sim: &crate::native_world::Mc2Simulation, label: &str, turn: usize) {
    assert!(
        sim.dispatch_events()
            .iter()
            .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "{label} turn{turn}: {:?}",
        sim.dispatch_events()
    );
}

fn public_step(
    game: &mut SpiderGame,
    command: crate::game_session::GameCommand,
    label: &str,
    turn: usize,
) {
    game.advance(game.identity().guard(), command).unwrap();
    no_dispatch_errors(simulation(game), label, turn);
}

fn mission_four_mana_percent(sim: &crate::native_world::Mc2Simulation) -> i32 {
    let player = sim.state.entity_index;
    let economy = sim.economy();
    let denominator = economy.owner_mana_global();
    assert!(denominator > 0);
    economy.castle_for_owner(player).map_or(0, |castle| {
        100 * economy
            .owner_mana_aggregate(player)
            .saturating_add(castle.mana)
            / denominator
    })
}

fn declare_mana_prefix_firing_pose(
    sim: &mut crate::native_world::Mc2Simulation,
    target: Position,
    horizontal_distance: i16,
) -> (i16, i16) {
    use crate::FlightWorld;
    let fov = sim.shared_spells().player_entity().array_fov();
    let mut hand = Position {
        x: target.x.wrapping_add_signed(horizontal_distance),
        y: target.y,
        z: target.z.checked_add(512).unwrap(),
    };
    let yaw = crate::level1_mission::projectile_guidance_angles(hand, target).0;
    let hand_yaw = yaw
        .wrapping_sub(sim.shared_spells().beyond_sight_auxiliary().next_entity)
        .wrapping_sub(512);
    let mut origin = Position {
        z: hand.z.checked_sub(fov).unwrap(),
        ..hand
    };
    crate::move_entity(&mut origin, hand_yaw.wrapping_add(1024), 0, 256);
    // The raw parent FOV and source hand offset own the launch, not the
    // independently displayed FlightState FOV. Raise only the declared player
    // fixture to the actual floor; reject an impossible Cave clearance.
    origin.z = origin
        .z
        .max(
            sim.canonical_terrain()
                .ground_height(origin)
                .checked_add(sim.profile().ground_clearance)
                .unwrap(),
        )
        .max(
            sim.canonical_terrain()
                .ground_height(hand)
                .checked_add(crate::level1_mission::FIREBALL_COLLISION_HALF_HEIGHT)
                .unwrap()
                .checked_sub(fov)
                .unwrap(),
        );
    hand.z = origin.z.checked_add(fov).unwrap();
    for (position, clearance) in [
        (origin, sim.profile().ceiling_clearance),
        (hand, crate::level1_mission::FIREBALL_COLLISION_HALF_HEIGHT),
    ] {
        let ceiling = sim.canonical_terrain().ceiling_height(position).unwrap();
        assert!(
            position.z <= ceiling.checked_sub(clearance).unwrap(),
            "declared mana-prefix pose must fit the actual Cave: origin={origin:?} hand={hand:?} target={target:?}"
        );
    }
    assert_eq!(
        crate::resolve_legacy_movement(
            &mut crate::FlightState::default(),
            origin,
            origin,
            sim.profile(),
            sim.canonical_terrain()
        ),
        crate::MovementResolution::Accept(origin),
    );
    assert!(crate::distance_3d_wrapped(origin, target) >= 1536);
    let aim = crate::level1_mission::projectile_guidance_angles(hand, target);
    sim.set_level2_entity_position(sim.state.entity_index, origin)
        .unwrap();
    sim.state.actual_speed = 0;
    sim.state.target_speed = 0;
    sim.set_level2_player_aim_for_test(aim.0, aim.1);
    aim
}

fn assert_mana_prefix_waiting(
    sim: &crate::native_world::Mc2Simulation,
    gate: crate::campaign::source_arena::SourceEntityHandle,
) {
    let objective = &sim.runtime().objectives[usize::from(sim.state.player_color_index)];
    assert_eq!(
        (
            objective.stage_selectors[0],
            objective.stage_selectors[1],
            objective.text_selector,
            objective.is_level_end
        ),
        (1, 1, 0, 0)
    );
    assert_eq!(sim.source_arena().handle(6), Some(gate));
    assert!(!sim.entities()[6].drawing_disabled());
    assert!(
        !sim.creature_runtime_census()
            .roots_of_kind(crate::level_definition::CreatureKind::Spider)
            .any(|root| root.living)
    );
    assert!(!sim.dispatch_events().iter().any(|event| matches!(
        event,
        crate::native_world::Mc2DispatchEvent::AuthoredGroupActivated {
            source_slot: 6,
            group: 6,
            ..
        }
    )));
}

fn observe_mana_prefix_payment(
    sim: &crate::native_world::Mc2Simulation,
    holder: crate::campaign::source_arena::SourceEntityHandle,
    player: crate::campaign::source_arena::SourceEntityHandle,
    cost: i32,
    launches: &[(u16, u32)],
) -> (Vec<(u16, u32)>, i64) {
    use crate::campaign::schedule::{CurrentOccupant, ScheduleReceipt};
    let mut charged = Vec::new();
    let mut debited = 0;
    for receipt in sim.cf10_production_schedule_receipts() {
        let ScheduleReceipt::EpilogueCompleted {
            slot,
            original,
            current: CurrentOccupant::Known { selector, .. },
            ..
        } = receipt
        else {
            continue;
        };
        let (Some(before), Some(after)) = (
            original.observed_player_resources(),
            selector.observed_player_resources(),
        ) else {
            continue;
        };
        if slot == holder.source_slot
            && sim.source_arena().handle(slot) == Some(holder)
            && before.0 == player
            && after
                == (
                    player,
                    before.1,
                    if before.2 >= 0 {
                        -cost
                    } else {
                        before.2.wrapping_sub(cost)
                    },
                    before.3,
                )
        {
            charged.extend_from_slice(launches);
        }
        if slot == player.source_slot
            && before.0 == player
            && after.0 == player
            && before.2 <= -cost
            && after.1 == before.1.wrapping_add(before.2).clamp(0, before.3)
            && before.1 - after.1 >= cost
        {
            debited += i64::from(before.1 - after.1);
        }
    }
    (charged, debited)
}

/// A declared prior bank isolates the last earned increment, not the whole
/// mission's economy. No objective, creature, RNG, phase or lifetime is staged.
/// Source60F00 freezes global/owned10/45 counters; 58F00 adds the CURRENT
/// Castle store. Only a real 60AB0 class3 deposit may cross this boundary.
fn earn_mission_four_mana_and_spawn_spiders(game: &mut SpiderGame) {
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::game_session::{GameCommand, SpellAssignment, SpellAssignmentHand};
    use crate::level_definition::{CreatureKind, FIREBALL_PROPERTIES, POSSESSION_PROPERTIES};
    use crate::native_world::level2_dynamic_economy::{
        PossessionSourceReceipt, start_possession_trace, take_possession_trace,
    };
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage};
    use crate::{FlightWorld, PlayerCommand};
    use std::collections::BTreeSet;

    const GROUP6: &[u16] = &[
        8, 9, 10, 11, 12, 137, 138, 139, 141, 250, 251, 255, 601, 602, 724, 725, 726,
    ];
    let player = simulation(game).state.entity_index;
    let player_handle = simulation(game).source_arena().handle(player).unwrap();
    let gate = simulation(game).source_arena().handle(6).unwrap();
    assert_eq!(
        (
            simulation(game).entities()[6].class,
            simulation(game).entities()[6].model,
            simulation(game).entities()[6].source_id,
            simulation(game).entities()[6].level_gate
        ),
        (11, 32, 6, 0)
    );
    assert_eq!(
        (
            simulation(game).runtime().stages[0].kind,
            simulation(game).runtime().stages[1].kind
        ),
        (0, 7)
    );
    let objectives = game
        .backend()
        .compatibility_campaign()
        .definition()
        .level_definition()
        .objectives();
    assert_eq!(
        objectives[0].condition(),
        crate::level_definition::ObjectiveCondition::CastleManaPercent(40)
    );
    assert_eq!(
        objectives[1].condition(),
        crate::level_definition::ObjectiveCondition::NoLivingCreatures(CreatureKind::Spider)
    );
    assert_mana_prefix_waiting(simulation(game), gate);
    assert!(
        simulation(game)
            .economy()
            .castle_for_owner(player)
            .is_none()
    );
    let firefly = simulation(game).source_arena().handle(165).unwrap();
    let original = *simulation(game)
        .economy()
        .authored_class5()
        .iter()
        .find(|row| row.active && row.source_entity_index == firefly.source_slot)
        .unwrap();
    assert_eq!(
        (
            original.model,
            original.life,
            original.max_life,
            original.mana
        ),
        (19, 600, 600, 300)
    );
    let baseline_global = simulation(game).economy().owner_mana_global();
    assert_eq!(simulation(game).economy().owner_mana_aggregate(player), 0);
    // Predeclare the bank from the original source projection and this real
    // actor's body mana, before casting or observing a death/collection.
    // 100*B/(G+B)<40; one 300 credit must cross, with room in the declared
    // prior Castle capacity. The paid spell mana is a separate player budget.
    let crossing_bank = i32::try_from((40_i64 * i64::from(baseline_global) + 59) / 60).unwrap();
    let bank = crossing_bank.checked_sub(original.mana / 2).unwrap();
    assert!(bank > 0);
    simulation_mut(game)
        .apply_carried_spell_families(&[0, 1], Some(0))
        .unwrap();
    simulation_mut(game)
        .stage_player_mana_for_test(20_000)
        .unwrap();
    public_step(
        game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.fireball").unwrap(),
                tier: 0,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
        "mana-prefix carried FireballI assignment",
        0,
    );
    let target = *simulation(game)
        .economy()
        .authored_class5()
        .iter()
        .find(|row| row.active && row.source_entity_index == firefly.source_slot)
        .unwrap();
    assert_eq!(target.life, 600);
    let collision = crate::native_world::source_target::read(
        simulation(game).source_arena(),
        crate::native_world::source_target::Context::default(),
        firefly.source_slot,
    )
    .unwrap()
    .unwrap()
    .collision
    .expect("ordinary Firefly source collision axes");
    let centre = Position {
        z: target
            .position
            .z
            .checked_add(collision.half_height)
            .unwrap()
            .checked_sub(crate::level1_mission::FIREBALL_COLLISION_VERTICAL_OFFSET)
            .unwrap(),
        ..target.position
    };
    // C2's east approach put real164 in front of165: source68490 could
    // acquire164 and10780 could collide with it first. Approach from west,
    // retaining the same distance/burst/health, and check the actual source
    // candidate order before committing the one fixed aim.
    let aim = declare_mana_prefix_firing_pose(simulation_mut(game), centre, -2048);
    let parent = simulation(game).shared_spells().player_entity();
    let mut muzzle = parent.position;
    crate::move_entity(&mut muzzle, parent.yaw.wrapping_sub(512), 0, 256);
    assert!(
        simulation(game).canonical_terrain().ground_height(muzzle) <= parent.position.z,
        "fixed left-hand fixture must not use the source terrain fallback"
    );
    muzzle.z = muzzle.z.checked_add(parent.array_fov()).unwrap();
    let source_scores = simulation(game)
        .economy()
        .fireball_guidance_targets(simulation(game).entities())
        .unwrap()
        .into_iter()
        .filter(|target| target.can_be_acquired_by(player))
        .filter_map(|target| {
            crate::level1_mission::projectile_target_score(
                muzzle,
                aim.0,
                aim.1,
                target.top_position,
            )
            .map(|score| (target.source_slot, score, target.top_position))
        })
        .collect::<Vec<_>>();
    let score = source_scores
        .iter()
        .find(|row| row.0 == firefly.source_slot)
        .expect("original165 lies inside the actual source acquisition cone/range")
        .1;
    assert!(
        source_scores
            .iter()
            .all(|row| row.0 == firefly.source_slot || score < row.1),
        "original165 must win source68490 scoring before this fixed burst: muzzle={muzzle:?} aim={aim:?} scores={source_scores:?}"
    );
    let holder_slot = simulation(game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[0];
    let holder = simulation(game).source_arena().handle(holder_slot).unwrap();
    let cost = FIREBALL_PROPERTIES.tiers[0].mana_cost;
    assert_eq!(
        simulation(game)
            .shared_spells()
            .pickup(holder_slot)
            .unwrap()
            .max_mana,
        cost
    );
    assert!(!simulation(game).shared_spells().free_spell_usage);
    let mut no_fireball = game.backend().compatibility_campaign().clone();
    let (mut launches, mut births, mut accepted, mut damage) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut charged = BTreeSet::new();
    let mut debited = 0_i64;
    let mut sphere: Option<SourceEntityHandle> = None;
    let mut terminals = Vec::new();
    let mut burst_history = Vec::new();
    #[derive(Debug)]
    struct FireflyBoundary {
        writes_end: usize,
        events_end: usize,
        source: Option<(SourceEntityHandle, i32, (i32, u16))>,
        callbacks: usize,
    }
    let mut boundaries = Vec::new();
    assert_eq!(
        area_damage::source_bounty(simulation(game).source_arena(), None, firefly.source_slot)
            .unwrap(),
        (0, 0)
    );
    area_damage::start_actor_damage_trace(&[firefly]);
    for turn in 0..96 {
        // One four-frame public held burst, then neutral input. Source5F660
        // permits tierI reactivation while active; word30=5 is not a minimum
        // interval. All four flyers can acquire before C4's first turn6 hit
        // starts the normal phase7/228-speed charge. The one initial pose/aim
        // above is never updated from a later target or player observation.
        let cast = turn < 4;
        let damage_before = damage.len();
        let had_sphere = sphere.is_some();
        let events = simulation(game).economy().events().len();
        let before_holder = simulation(game)
            .shared_spells()
            .pickup(holder_slot)
            .unwrap();
        let before_holder = (
            before_holder.active_value,
            before_holder.activation_value,
            u16::from_le_bytes([before_holder.record[58], before_holder.record[59]]),
        );
        area_damage::start_fireball_launch_trace();
        crate::native_world::start_fireball_terminal_trace();
        area_damage::start_effect_damage_trace(firefly.source_slot);
        public_step(
            game,
            GameCommand {
                cast_left: cast,
                ..Default::default()
            },
            "fixed mana-source Fireball burst",
            turn,
        );
        let own = area_damage::take_fireball_launch_trace()
            .into_iter()
            .filter(|(_, _, owner)| *owner == player)
            .collect::<Vec<_>>();
        let own_handles = own
            .iter()
            .map(|&(slot, generation, _)| (slot, generation))
            .collect::<Vec<_>>();
        let payment = observe_mana_prefix_payment(
            simulation(game),
            holder,
            player_handle,
            cost,
            &own_handles,
        );
        charged.extend(payment.0);
        debited += payment.1;
        for &(slot, generation, _) in &own {
            let packet = *simulation(game)
                .economy()
                .fireball_projectiles()
                .iter()
                .find(|row| row.active && row.source_entity_index == slot)
                .unwrap();
            assert_eq!(
                (
                    packet.owner_entity_index,
                    packet.holder_entity_index,
                    packet.source_subspell_index
                ),
                (player, holder.source_slot, 250)
            );
            assert_eq!(
                simulation(game).source_arena().slot_generation(slot),
                generation
            );
            launches.push((slot, generation, packet));
        }
        births.extend(area_damage::take_fireball_impact_birth_trace());
        terminals.extend(crate::native_world::take_fireball_terminal_trace());
        accepted.extend(area_damage::take_effect_damage_trace());
        let sim = simulation(game);
        for (offset, event) in sim.economy().events()[events..].iter().enumerate() {
            match *event {
                Level2EconomyEvent::DamageApplied {
                    target_entity_index,
                    attacker_entity_index,
                    damage: amount,
                    lethal,
                } if target_entity_index == firefly.source_slot => {
                    assert_eq!(attacker_entity_index, player);
                    assert_eq!(
                        sim.source_arena().handle(firefly.source_slot),
                        Some(firefly),
                        "DamageApplied must belong to the original Firefly generation"
                    );
                    damage.push((events + offset, amount, lethal));
                }
                Level2EconomyEvent::ManaSphereSpawned {
                    sphere_id,
                    source_entity_index,
                    mana,
                } if source_entity_index == firefly.source_slot => {
                    assert!(sphere.is_none(), "one original Firefly death birth");
                    assert_eq!(mana, original.mana);
                    let handle = sim
                        .source_arena()
                        .handle(u16::try_from(sphere_id).unwrap())
                        .unwrap();
                    assert!(
                        sim.campaign_entity_presentations(0)
                            .unwrap()
                            .iter()
                            .any(|view| view.entity_index == handle.source_slot)
                    );
                    sphere = Some(handle);
                }
                _ => {}
            }
        }
        let source = sim
            .source_arena()
            .handle(firefly.source_slot)
            .filter(|handle| *handle == firefly)
            .map(|handle| {
                let row = sim
                    .economy()
                    .authored_class5()
                    .iter()
                    .find(|row| row.active && row.source_entity_index == handle.source_slot)
                    .unwrap();
                assert_eq!((row.model, row.max_life), (19, 600));
                (
                    handle,
                    row.life,
                    area_damage::source_bounty(sim.source_arena(), None, handle.source_slot)
                        .unwrap(),
                )
            });
        boundaries.push(FireflyBoundary {
            writes_end: accepted.len(),
            events_end: sim.economy().events().len(),
            source,
            callbacks: sim
                .cf10_production_schedule_receipts()
                .iter()
                .filter(|receipt| {
                    matches!(receipt,
                crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. }
                    if *slot == firefly.source_slot)
                })
                .count(),
        });
        if turn < 12
            || turn % 8 == 0
            || damage.len() != damage_before
            || (!had_sphere && sphere.is_some())
        {
            let current = sim
                .economy()
                .authored_class5()
                .iter()
                .find(|row| row.active && row.source_entity_index == firefly.source_slot)
                .map(|row| {
                    (
                        sim.source_arena().handle(firefly.source_slot),
                        row.life,
                        row.action,
                        row.position,
                        row.yaw,
                        row.byte_0x46_70,
                        row.pending_word_0x62,
                    )
                });
            let holder_now = sim.shared_spells().pickup(holder_slot).unwrap();
            let holder_now = (
                holder_now.active_value,
                holder_now.activation_value,
                u16::from_le_bytes([holder_now.record[58], holder_now.record[59]]),
            );
            let flyers = sim
                .economy()
                .fireball_projectiles()
                .iter()
                .filter(|row| row.active && row.owner_entity_index == player)
                .map(|row| {
                    (
                        sim.source_arena().handle(row.source_entity_index),
                        row.position,
                        row.yaw,
                        row.pitch,
                        row.target_source_entity_index,
                        row.life,
                    )
                })
                .collect::<Vec<_>>();
            burst_history.push((
                turn,
                cast,
                sim.state.position,
                before_holder,
                holder_now,
                current,
                flyers,
            ));
        }
        assert_mana_prefix_waiting(sim, gate);
        no_fireball.step(PlayerCommand::default(), 0);
        no_dispatch_errors(
            no_fireball.simulation(),
            "equal elapsed no-Fireball mana source",
            turn,
        );
        let negative = no_fireball
            .simulation()
            .economy()
            .authored_class5()
            .iter()
            .find(|row| row.active && row.source_entity_index == firefly.source_slot)
            .unwrap();
        assert_eq!(
            no_fireball
                .simulation()
                .source_arena()
                .handle(firefly.source_slot),
            Some(firefly)
        );
        assert_eq!(negative.life, 600);
        if turn >= 32 && sphere.is_some() && sim.source_arena().resolve(firefly).is_none() {
            break;
        }
    }
    let launch_summary = launches
        .iter()
        .map(|&(slot, generation, packet)| {
            (
                slot,
                generation,
                packet.position,
                packet.yaw,
                packet.pitch,
                packet.target_source_entity_index,
                packet.life,
                packet.holder_entity_index,
            )
        })
        .collect::<Vec<_>>();
    let accepted_summary = accepted
        .iter()
        .map(|write| {
            (
                write.source_slot,
                write
                    .source_raw_impact
                    .as_ref()
                    .map(|(generation, row)| (*generation, row.class, row.model)),
                write.pulse.position,
                write.pulse.payload,
                write.target_slot,
                write.target_generation,
                write.mailbox_before,
                write.mailbox_after,
                write.hit_count,
                write.event_cursor,
                write.writer_sequence,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        launches.len(),
        4,
        "four held input frames must produce four source launches: launch={launch_summary:?} history={burst_history:?}"
    );
    assert!(
        launches
            .iter()
            .all(|&(slot, generation, _)| charged.contains(&(slot, generation)))
    );
    assert!(debited >= i64::try_from(launches.len()).unwrap() * i64::from(cost));
    #[derive(Debug)]
    struct AdmittedWrite {
        observation: usize,
        event_cursor: usize,
        sequence: u64,
        before: (i32, u16),
        after: (i32, u16),
    }
    let mut writes = Vec::new();
    let mut represented_writers = BTreeSet::new();
    let mut previous_sequence: Option<u64> = None;
    let mut previous_cursor = 0;
    for (observation, write) in accepted.iter().enumerate().filter(|(_, write)| {
        write.target_slot == firefly.source_slot && write.target_generation == firefly.generation
    }) {
        let Some(sequence) = write.writer_sequence else {
            assert_eq!(
                write.mailbox_before, write.mailbox_after,
                "an unrepresented write cannot change the original mailbox"
            );
            continue;
        };
        assert!(
            represented_writers.insert(sequence),
            "one observation per actual writer"
        );
        assert!(previous_sequence.is_none_or(|previous| previous < sequence));
        assert!(previous_cursor <= write.event_cursor);
        previous_sequence = Some(sequence);
        previous_cursor = write.event_cursor;
        assert!(write.source_scoped);
        assert_eq!((write.pulse.owner, write.pulse.payload), (player, 250));
        let (generation, packet) = write
            .source_raw_impact
            .as_ref()
            .expect("actual ordinary Fireball child packet at its write");
        assert_eq!(
            (
                packet.class,
                packet.model,
                packet.subspell_index,
                packet.source_id
            ),
            (10, 0, 250, player)
        );
        assert!(
            births
                .iter()
                .any(
                    |&(impact, actual, parent, parent_generation, owner)| write.source_slot
                        == Some(impact)
                        && actual == *generation
                        && owner == player
                        && launches
                            .iter()
                            .any(|&(slot, launch_generation, launch)| slot == parent
                                && launch_generation == parent_generation
                                && launch.owner_entity_index == player
                                && launch.holder_entity_index == holder.source_slot
                                && charged.contains(&(slot, launch_generation)))
                ),
            "each admitted250 writer needs its own paid launch/child generations: write={accepted_summary:?} births={births:?} launch={launch_summary:?}"
        );
        let before = write
            .mailbox_before
            .expect("actual pre-write Firefly mailbox");
        let after = write
            .mailbox_after
            .expect("actual post-write Firefly mailbox");
        // Ordinary10C80 replaces an ownerless payload; otherwise it adds.
        // This checks the actual writer arithmetic, not a multiple-of250 rule.
        assert_eq!(
            after,
            (
                if before.1 == 0 {
                    250
                } else {
                    before.0.wrapping_add(250)
                },
                player
            )
        );
        writes.push(AdmittedWrite {
            observation,
            event_cursor: write.event_cursor,
            sequence,
            before,
            after,
        });
    }

    // Effect event_cursor is the append boundary AFTER the write. A write
    // with cursor<=DamageApplied's index precedes that consumer; after it,
    // the next cursor is at least index+1. Preserve the frame's receipt bound
    // too: several ordinary turns can share an unchanged event cursor.
    let mut next_write = 0;
    let mut next_damage = 0;
    let mut mailbox = (0, 0);
    let mut pending_writers = Vec::new();
    let mut consumed_writers = BTreeSet::new();
    let mut consumption_chain = Vec::new();
    let mut life = original.life;
    let fold_write = |write: &AdmittedWrite, mailbox: &mut (i32, u16), pending: &mut Vec<u64>| {
        assert_eq!(
            *mailbox, write.before,
            "every observed write must start at the exact preceding write/clear: {write:?}"
        );
        *mailbox = write.after;
        pending.push(write.sequence);
    };
    let mut previous_events_end = 0;
    for boundary in &boundaries {
        assert!(previous_events_end <= boundary.events_end);
        previous_events_end = boundary.events_end;
        let mut frame_consumptions = 0;
        while next_damage < damage.len() && damage[next_damage].0 < boundary.events_end {
            let (event, amount, lethal) = damage[next_damage];
            while next_write < writes.len()
                && writes[next_write].observation < boundary.writes_end
                && writes[next_write].event_cursor <= event
            {
                fold_write(&writes[next_write], &mut mailbox, &mut pending_writers);
                next_write += 1;
            }
            assert!(
                !pending_writers.is_empty(),
                "each consumption needs new admitted writers"
            );
            assert_eq!(
                mailbox,
                (amount, player),
                "the complete preceding writer chain must equal the ACTUAL consumed amount: event={event} pending={pending_writers:?} writes={writes:?}"
            );
            let contributors = std::mem::take(&mut pending_writers);
            assert!(
                contributors
                    .iter()
                    .all(|sequence| consumed_writers.insert(*sequence)),
                "an actual writer cannot fund two consumer events"
            );
            let before = life;
            life = life.wrapping_sub(amount);
            assert_eq!(lethal, life < 0);
            consumption_chain.push((event, contributors, before, amount, life, lethal));
            // Source25610/1BF90 clear only owner62 and retain payload5E.
            // The frame-end mailbox or next ordinary write must witness it;
            // Ordinary10C80 then replaces the ownerless retained payload.
            mailbox = (amount, 0);
            next_damage += 1;
            frame_consumptions += 1;
        }
        while next_write < writes.len() && writes[next_write].observation < boundary.writes_end {
            assert!(writes[next_write].event_cursor <= boundary.events_end);
            fold_write(&writes[next_write], &mut mailbox, &mut pending_writers);
            next_write += 1;
        }
        if frame_consumptions != 0 {
            assert!(
                boundary.callbacks >= frame_consumptions && boundary.source.is_some(),
                "generation-bound DamageApplied must have its actual scheduled consumer: {boundary:?}"
            );
        }
        if let Some((handle, actual_life, actual_mailbox)) = boundary.source {
            assert_eq!(handle, firefly);
            assert_eq!(
                actual_life, life,
                "exact continuous600HP arithmetic at the actual source boundary: chain={consumption_chain:?}"
            );
            assert_eq!(
                actual_mailbox, mailbox,
                "observed source mailbox must agree after consumers and any later writes: {boundary:?}"
            );
        } else {
            assert!(
                life < 0 && mailbox.1 == 0 && pending_writers.is_empty(),
                "original generation may disappear only after its reconstructed paid fatal subtraction"
            );
        }
    }
    assert_eq!(next_write, writes.len());
    assert_eq!(next_damage, damage.len());
    assert!(pending_writers.is_empty());
    assert_eq!(
        consumed_writers, represented_writers,
        "every admitted writer is consumed exactly once"
    );
    assert!(
        life < 0 && damage.last().is_some_and(|row| row.2),
        "unchanged600HP original165 must die to this fixed held burst: life={life} damage={damage:?} current={:?} player={:?} muzzle={muzzle:?} aim={aim:?} scores={source_scores:?} launches={launch_summary:?} impacts={births:?} terminals={terminals:?} accepted={accepted_summary:?} history={burst_history:?}",
        simulation(game)
            .economy()
            .authored_class5()
            .iter()
            .find(|row| row.source_entity_index == firefly.source_slot)
            .map(|row| (
                row.life,
                row.action,
                row.position,
                row.source_byte_3e,
                row.byte_0x46_70,
                row.pending_word_0x62,
                row.word_0x26
            )),
        simulation(game).state.position
    );
    assert!(simulation(game).source_arena().resolve(firefly).is_none());
    let sphere: SourceEntityHandle = sphere.expect("real paid death produced its mana sphere");
    eprintln!(
        "real Level4 paid Firefly death: source={firefly:?} sphere={sphere:?} paid_launches={} debit={debited} consumed={consumption_chain:?} writes={writes:?}",
        launches.len()
    );

    // Declare a prior real Castle at a source-valid nearby site. Its bank and
    // capacity are fixed above; no sphere/collector transform or relation is
    // edited. This is not proof of earning that prior Castle or bank.
    let sphere_position = simulation(game)
        .economy()
        .spheres()
        .iter()
        .find(|row| row.active && row.source_entity_index == sphere.source_slot)
        .unwrap()
        .position;
    let castle = {
        let sim = simulation_mut(game);
        for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
            sim.economy_mut()
                .set_castle_stage_footprint(stage, footprint);
        }
        let site = (1_u16..32)
            .flat_map(|y| {
                (1_u16..32).map(move |x| Position {
                    x: x * 2048,
                    y: y * 2048,
                    z: 0,
                })
            })
            .filter(|position| crate::distance_3d_wrapped(*position, sphere_position) >= 2048)
            .filter(|position| {
                !sim.economy().castle_projectile_site_blocked(
                    *position,
                    sim.entities(),
                    sim.canonical_terrain(),
                )
            })
            .min_by_key(|position| crate::distance_3d_wrapped(*position, sphere_position))
            .expect("declared prior Castle has an actual admitted site");
        let castle = sim.economy_mut().build_castle(site, player).unwrap();
        assert!(
            sim.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, bank + 2 * original.mana)
        );
        assert!(
            sim.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, bank)
        );
        castle
    };
    public_step(
        game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.possession").unwrap(),
                tier: 0,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
        "mana-prefix carried PossessionI assignment",
        0,
    );
    assert_eq!(
        simulation(game).economy().castle_at(castle).unwrap().mana,
        bank
    );
    assert!(mission_four_mana_percent(simulation(game)) < 40);
    assert_mana_prefix_waiting(simulation(game), gate);
    let castle_handle = simulation(game).source_arena().handle(castle).unwrap();
    let target = simulation(game)
        .possession_guidance_target_top_for_test(sphere.source_slot)
        .unwrap();
    let possession_aim = declare_mana_prefix_firing_pose(simulation_mut(game), target, 2048);
    let possession_holder = simulation(game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[1];
    let possession_holder = simulation(game)
        .source_arena()
        .handle(possession_holder)
        .unwrap();
    let mut no_possession = game.backend().compatibility_campaign().clone();
    let mut claim = false;
    let mut possession_launches = Vec::new();
    let mut possession_charged = BTreeSet::new();
    let mut possession_debited = 0_i64;
    let mut collected: Option<(SourceEntityHandle, i32)> = None;
    let mut credited = false;
    let mut hidden = None;
    let mut reclaimed = false;
    let mut claim_receipts = Vec::new();
    let castle_position = simulation(game)
        .economy()
        .castle_at(castle)
        .unwrap()
        .position;
    let bound = 32
        + 2 * (crate::distance_3d_wrapped(sphere_position, castle_position) as usize).div_ceil(48)
        + 192;
    start_possession_trace();
    for turn in 0..bound {
        let before_store = simulation(game).economy().castle_at(castle).unwrap().mana;
        let children = simulation(game).economy().castle_children().to_vec();
        let events = simulation(game).economy().events().len();
        public_step(
            game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
            "one paid mana claim and ordinary collection",
            turn,
        );
        let sim = simulation(game);
        let mut own_this_turn = Vec::new();
        for row in sim.economy().possession_projectiles().iter().filter(|row| {
            row.source_id_entity_index == player
                && row.holder_entity_index == possession_holder.source_slot
        }) {
            if let Some(handle) = sim.source_arena().handle(row.source_entity_index)
                && !possession_launches.contains(&handle)
            {
                possession_launches.push(handle);
                own_this_turn.push((handle.source_slot, handle.generation));
            }
        }
        let payment = observe_mana_prefix_payment(
            sim,
            possession_holder,
            player_handle,
            POSSESSION_PROPERTIES.tiers[0].mana_cost,
            &own_this_turn,
        );
        possession_charged.extend(payment.0);
        possession_debited += payment.1;
        assert_eq!(sim.source_arena().handle(castle), Some(castle_handle));
        for event in &sim.economy().events()[events..] {
            match *event {
                Level2EconomyEvent::ManaSphereClaimed {
                    sphere_id,
                    owner_entity_index,
                } if sphere_id == u32::from(sphere.source_slot) && owner_entity_index == player => {
                    assert_eq!(sim.source_arena().handle(sphere.source_slot), Some(sphere));
                    claim = true;
                }
                Level2EconomyEvent::CastleDefenderCollectedSphere {
                    defender_slot,
                    sphere_id,
                    mana,
                } if sphere_id == u32::from(sphere.source_slot) => {
                    assert!(claim && collected.is_none());
                    assert_eq!(mana, original.mana);
                    assert_eq!(sim.source_arena().handle(sphere.source_slot), Some(sphere));
                    let before = children
                        .iter()
                        .find(|row| row.source_entity_index == defender_slot)
                        .unwrap();
                    let after = sim
                        .economy()
                        .castle_children()
                        .iter()
                        .find(|row| row.source_entity_index == defender_slot)
                        .unwrap();
                    assert_eq!(
                        (before.mana, after.mana, after.id_entity_index),
                        (0, mana, player)
                    );
                    collected = Some((sim.source_arena().handle(defender_slot).unwrap(), mana));
                    hidden = Some(turn);
                    assert!(
                        !sim.campaign_entity_presentations(0)
                            .unwrap()
                            .iter()
                            .any(|view| view.entity_index == sphere.source_slot)
                    );
                }
                Level2EconomyEvent::CastleDefenderArrived {
                    defender_slot,
                    castle_id,
                    mana,
                } if castle_id == castle
                    && collected.is_some_and(|(holder, payload)| {
                        holder.source_slot == defender_slot && payload == mana
                    }) =>
                {
                    let (holder, payload) = collected.unwrap();
                    assert!(!credited);
                    assert_eq!(sim.source_arena().handle(defender_slot), Some(holder));
                    assert_eq!(before_store, bank);
                    assert_eq!(
                        sim.economy().castle_at(castle).unwrap().mana,
                        bank + payload
                    );
                    assert_eq!(
                        sim.economy().owner_mana_aggregate(player),
                        0,
                        "a ground sphere/3-3 carrier never supplies owned10/45 objective mana"
                    );
                    assert!(100 * bank / sim.economy().owner_mana_global() < 40);
                    assert!(mission_four_mana_percent(sim) >= 40);
                    let objective =
                        &sim.runtime().objectives[usize::from(sim.state.player_color_index)];
                    assert_eq!(
                        (
                            objective.stage_selectors[0],
                            objective.stage_selectors[1],
                            objective.text_selector,
                            objective.is_level_end
                        ),
                        (2, 1, 1, 0)
                    );
                    assert_eq!(sim.source_arena().handle(6), Some(gate));
                    assert!(!sim.entities()[6].drawing_disabled());
                    assert!(!sim.dispatch_events().iter().any(|event| matches!(
                        event,
                        Mc2DispatchEvent::AuthoredGroupActivated {
                            source_slot: 6,
                            group: 6,
                            ..
                        }
                    )));
                    credited = true;
                }
                _ => {}
            }
        }
        if hidden.is_some_and(|hidden| turn == hidden + 1) {
            assert!(sim.source_arena().resolve(sphere).is_none());
            reclaimed = true;
        }
        if !credited {
            assert_eq!(sim.economy().castle_at(castle).unwrap().mana, bank);
            assert!(mission_four_mana_percent(sim) < 40);
            assert_mana_prefix_waiting(sim, gate);
        }
        no_possession.step(PlayerCommand::default(), 0);
        no_dispatch_errors(
            no_possession.simulation(),
            "equal elapsed unclaimed mana/no crossing",
            turn,
        );
        assert_eq!(
            no_possession
                .simulation()
                .economy()
                .castle_at(castle)
                .unwrap()
                .mana,
            bank
        );
        assert!(mission_four_mana_percent(no_possession.simulation()) < 40);
        assert_mana_prefix_waiting(no_possession.simulation(), gate);
        if turn < 8 || turn % 32 == 0 || credited {
            let castle_now = sim.economy().castle_at(castle).unwrap();
            let sphere_now = sim
                .economy()
                .spheres()
                .iter()
                .find(|row| row.source_entity_index == sphere.source_slot)
                .map(|row| {
                    (
                        sim.source_arena().handle(sphere.source_slot),
                        row.position,
                        row.mana,
                        row.player_entity_index,
                        row.pending_delete,
                        row.control_flags,
                        row.assigned_defender_slot,
                    )
                });
            let collectors = sim
                .economy()
                .castle_children()
                .iter()
                .filter(|row| row.active && row.source_class == 3 && row.model == 3)
                .map(|row| {
                    (
                        sim.source_arena().handle(row.source_entity_index),
                        row.position,
                        row.target_sphere_slot,
                        row.mana,
                        row.action,
                        row.speed,
                    )
                })
                .collect::<Vec<_>>();
            claim_receipts.push((
                turn,
                sim.economy().owner_mana_global(),
                (
                    castle_now.mana,
                    castle_now.mana_capacity,
                    castle_now.level,
                    castle_now.action,
                ),
                claim,
                collected,
                credited,
                sphere_now,
                collectors,
            ));
        }
        if credited {
            assert!(reclaimed);
            break;
        }
    }
    let possession_trace = take_possession_trace();
    assert!(
        claim && credited && reclaimed,
        "real same-generation sphere must credit Castle before mana40: sphere={sphere:?} bank={bank} aim={possession_aim:?} history={claim_receipts:?} trace={possession_trace:?}"
    );
    assert_eq!(possession_launches.len(), 1);
    assert!(possession_charged.contains(&(
        possession_launches[0].source_slot,
        possession_launches[0].generation
    )));
    assert!(possession_debited >= i64::from(POSSESSION_PROPERTIES.tiers[0].mana_cost));
    assert!(possession_trace.iter().any(|receipt| matches!(receipt,
        PossessionSourceReceipt::MailboxWrite { effect, target, source_id, amount, possessor, .. }
            if *target == sphere && *source_id == player && *amount == 0 && *possessor == player
                && possession_trace.iter().any(|birth| matches!(birth,
                    PossessionSourceReceipt::ImpactBirth { effect: actual, projectile, holder, owner, .. }
                        if actual == effect && *projectile == possession_launches[0]
                            && *holder == possession_holder && *owner == player_handle)))));

    let mut published = false;
    for turn in 0..8 {
        assert_eq!(simulation(game).source_arena().handle(6), Some(gate));
        public_step(
            game,
            GameCommand::default(),
            "following real mana-complete switch",
            turn,
        );
        no_possession.step(PlayerCommand::default(), 0);
        no_dispatch_errors(
            no_possession.simulation(),
            "equal elapsed no mana-complete switch",
            turn,
        );
        assert_mana_prefix_waiting(no_possession.simulation(), gate);
        let sim = simulation(game);
        if sim.dispatch_events().iter().any(|event| matches!(event,
            Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 6, group: 6, authored_indices, .. }
                if authored_indices.as_ref() == GROUP6)) {
            assert!(sim.entities()[6].drawing_disabled());
            published = true;
            break;
        }
    }
    assert!(
        published,
        "real earned mana40 must reach original reset6's group6 callback"
    );
    let roots = simulation(game)
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Spider)
        .map(|row| (row.origin, row.handle))
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 6);
    for authored_record in [137, 138, 139, 724, 725, 726] {
        let handle = roots
            .iter()
            .find_map(|&(origin, handle)| {
                (origin == CreatureRootOrigin::AuthoredRecord { authored_record }).then_some(handle)
            })
            .unwrap();
        let sim = simulation(game);
        let row = &sim.entities()[usize::from(handle.source_slot)];
        assert_eq!(
            (row.class, row.model, row.life, row.max_life),
            (5, 20, 5500, 5500)
        );
        assert!(
            sim.campaign_entity_presentations(0)
                .unwrap()
                .iter()
                .any(|view| view.entity_index == handle.source_slot
                    && view.render_parameter_index == 288)
        );
    }
    let phases_before = roots
        .iter()
        .map(|&(_, handle)| simulation(game).entities()[usize::from(handle.source_slot)].phase)
        .collect::<Vec<_>>();
    public_step(
        game,
        GameCommand::default(),
        "six real living Spiders block ordered absence",
        0,
    );
    // The scheduler observer is thread-local, not session-owned. Capture the
    // actual positive pass BEFORE the mirrored control steps and replaces it.
    // Source58F00 consumes ObjectiveDone after UpdateEntities; the ordinary
    // pass itself must still reach each original Spider and its phase tail.
    let living_pass = simulation(game).cf10_production_schedule_receipts();
    no_possession.step(PlayerCommand::default(), 0);
    no_dispatch_errors(
        no_possession.simulation(),
        "equal elapsed final no-crossing",
        0,
    );
    assert_mana_prefix_waiting(no_possession.simulation(), gate);
    let sim = simulation(game);
    let objective = &sim.runtime().objectives[usize::from(sim.state.player_color_index)];
    assert_eq!(
        (
            objective.stage_selectors[0],
            objective.stage_selectors[1],
            objective.text_selector,
            objective.is_level_end,
            objective.done
        ),
        (2, 1, 1, 0, 0)
    );
    for (&(_, handle), phase) in roots.iter().zip(phases_before) {
        assert_eq!(sim.source_arena().handle(handle.source_slot), Some(handle));
        assert!(
            sim.creature_identity_for_source_slot(handle.source_slot)
                .unwrap()
                .living
        );
        let row = &sim.entities()[usize::from(handle.source_slot)];
        assert_eq!(row.phase, phase.wrapping_add(1));
        assert_eq!((row.life, row.max_life), (5500, 5500));
        assert!(living_pass.iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::CallbackCompleted { slot, .. } if *slot == handle.source_slot)));
        assert!(living_pass.iter().any(|receipt| matches!(receipt,
            crate::campaign::schedule::ScheduleReceipt::EpilogueCompleted { slot, .. } if *slot == handle.source_slot)));
    }
    eprintln!(
        "real Level4 mana prefix: Firefly={firefly:?} sphere={sphere:?} Castle={castle_handle:?} prior_bank={bank} earned={} source_global={} percent={} gate={gate:?} roots={roots:?}",
        original.mana,
        sim.economy().owner_mana_global(),
        mission_four_mana_percent(sim)
    );
}

#[test]
fn mission_four_earned_mana_forty_drives_original_switch_and_six_living_spiders() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::game_session::GameSession;
    let package = mission_four_root();
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2004,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    earn_mission_four_mana_and_spawn_spiders(&mut game);
}

#[test]
fn mission_four_actual_spider_web_contact_recovery_and_paid_meteor_cleanup() {
    use std::collections::BTreeSet;
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::game_session::{GameCommand, GameSession, SpellAssignment, SpellAssignmentHand};
    use crate::level_definition::{CreatureKind, METEOR_PROPERTIES};
    use crate::native_world::{Level2EconomyEvent, area_damage, lightning};
    use crate::native_world::level2_dynamic_economy::{
        PossessionSourceReceipt, start_possession_trace, take_possession_trace,
    };
    use crate::{FlightWorld, PlayerCommand};

    let package = mission_four_root();
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2004,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    let player = simulation(&game).state.entity_index;
    let player_handle = simulation(&game).source_arena().handle(player).unwrap();
    // Continue the SAME real session through earned mana40, reset6 and all
    // six living Spider callbacks before any encounter-only resource setup.
    earn_mission_four_mana_and_spawn_spiders(&mut game);
    let earned_castle = simulation(&game)
        .economy()
        .castle_for_owner(player)
        .unwrap()
        .source_entity_index;
    let earned_castle = simulation(&game)
        .source_arena()
        .handle(earned_castle)
        .unwrap();
    // Age actual startup protection before the encounter; its counter, NPCs,
    // source RNG and timers are never edited. All owner-package rows remain.
    for turn in 0..256 {
        if simulation(&game).session.state.live.player_protection_ticks == 0 {
            break;
        }
        public_step(
            &mut game,
            GameCommand::default(),
            "natural startup protection",
            turn,
        );
    }
    assert_eq!(
        simulation(&game).session.state.live.player_protection_ticks,
        0
    );

    // Declared prior carry/progression and player resources are prerequisites
    // for the paid public tierII activation below. The earned prefix's real
    // Castle and objective state remain the current session's carriers.
    {
        let sim = simulation_mut(&mut game);
        sim.apply_carried_spell_families(&[9], Some(9)).unwrap();
        // Install source family9's coherent prior tierII progression through
        // the continuation API. This is declared carry, not experience earned
        // by this encounter; public hand assignment still validates the tier.
        sim.apply_carried_spell_progression(&[crate::native_world::CarriedSpellProgress {
            family: 9,
            persistent_experience: crate::native_world::LEVEL2_SPELL_TIER_THRESHOLDS[9][1],
            runtime_experience: 0,
            unlocked_tier: 1,
            selected_tier: 1,
        }])
        .unwrap();
        assert_eq!(
            sim.shared_spells().local_player().spell_unlocked_levels[9],
            1
        );
        // Only AFTER the real earned crossing/group6 proof, restore the old
        // declared encounter bank: source68D50 also gates Lightning/Meteor on
        // Castle mana. This200k is a resource prerequisite, not earned mana.
        assert_eq!(
            sim.source_arena().handle(earned_castle.source_slot),
            Some(earned_castle)
        );
        assert!(
            sim.economy_mut()
                .set_castle_mana_prerequisite_for_test(earned_castle.source_slot, 200_000)
        );
        assert_eq!(
            sim.economy().castle_at(earned_castle.source_slot).unwrap().mana,
            200_000
        );
        assert_eq!(
            sim.source_arena().handle(earned_castle.source_slot),
            Some(earned_castle)
        );
        let live = &mut sim.session.state.live;
        let row = &mut live.economy.entities[usize::from(player)];
        row.set_max_life(200_000);
        row.set_life(200_000);
        row.max_mana = 200_000;
        row.record[144..148].copy_from_slice(&200_000_i32.to_le_bytes());
        row.set_mana(200_000);
        row.set_mana_regen(0);
        let row = row.clone();
        live.player_life = row.life;
        live.shared_spells.sync_player_entity_from_authority(&row);
        live.shared_spells.player_mana = row.mana;
        live.shared_spells.caster.mana = row.mana;
        live.shared_spells
            .sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() {
            fire.caster.mana = row.mana;
        }
        live.core.state.actual_speed = 0;
        live.core.state.target_speed = 0;
    }
    public_step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.meteor").unwrap(),
                tier: 1,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
        "declared MeteorII public assignment after earned group6",
        0,
    );
    let roots = simulation(&game)
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Spider)
        .filter(|row| row.living)
        .map(|row| (row.origin, row.handle))
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 6);
    let root = roots
        .iter()
        .find_map(|&(origin, handle)| {
            (origin
                == CreatureRootOrigin::AuthoredRecord {
                    authored_record: 137,
                })
            .then_some(handle)
        })
        .unwrap();
    let original = simulation(&game).entities()[usize::from(root.source_slot)].clone();
    let entry_roots = roots
        .iter()
        .map(|&(origin, handle)| {
            let row = &simulation(&game).entities()[usize::from(handle.source_slot)];
            (
                origin,
                handle,
                row.source_id,
                row.position,
                row.yaw,
                row.phase,
                row.record[61],
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        (
            original.class,
            original.model,
            original.life,
            original.max_life
        ),
        (5, 20, 5500, 5500)
    );
    let views = simulation(&game).campaign_entity_presentations(0).unwrap();
    assert!(views.iter().any(|view| view.entity_index == root.source_slot && view.render_parameter_index == 288));

    // One fixed closer lure. C3's4096 line favored authored726 (about1230
    // away) and its other web legitimately advanced137's current-A4 charge.
    // The source-table1280 line stays near137's height and makes137 the
    // nearest original Spider; every other group member remains admitted.
    let mut lure = original.position;
    crate::move_entity(&mut lure, original.yaw, 0, 1280);
    lure.z = simulation(&game)
        .canonical_terrain()
        .ground_height(lure)
        .wrapping_add(256);
    if let Some(ceiling) = simulation(&game).canonical_terrain().ceiling_height(lure) {
        assert!(
            lure.z <= ceiling.wrapping_sub(256),
            "fixed lure must fit actual Cave geometry"
        );
    }
    let own_distance =
        crate::native_world::level2_wrapping_source_distance_3d(lure, original.position);
    assert!(
        roots
            .iter()
            .filter(|(_, handle)| *handle != root)
            .all(
                |&(_, handle)| crate::native_world::level2_wrapping_source_distance_3d(
                    lure,
                    simulation(&game).entities()[usize::from(handle.source_slot)].position
                ) > own_distance
            ),
        "the fixed entry must favor original137: root={root:?} sourceID={} lure={lure:?} roots={entry_roots:?}",
        original.source_id
    );
    // Declared prior LightningI carry at the player encounter boundary.
    // Its real holder is materialised after the earned wave. Carry adoption
    // must preserve the current original Spider identities and source state.
    {
        let sim = simulation_mut(&mut game);
        sim.apply_carried_spell_families(&[7, 9], Some(9)).unwrap();
        sim.apply_carried_spell_progression(&[crate::native_world::CarriedSpellProgress {
            family: 7,
            persistent_experience: 0,
            runtime_experience: 0,
            unlocked_tier: 0,
            selected_tier: 0,
        }])
        .unwrap();
        for &(origin, handle, source_id, position, yaw, phase, proximity) in &entry_roots {
            let row = &sim.entities()[usize::from(handle.source_slot)];
            assert_eq!(
                sim.source_arena().handle(handle.source_slot),
                Some(handle),
                "{origin:?}"
            );
            assert_eq!(
                (
                    row.source_id,
                    row.position,
                    row.yaw,
                    row.phase,
                    row.record[61]
                ),
                (source_id, position, yaw, phase, proximity)
            );
        }
    }
    simulation_mut(&mut game)
        .set_level2_entity_position(player, lure)
        .unwrap();
    simulation_mut(&mut game).state.actual_speed = 0;
    simulation_mut(&mut game).state.target_speed = 0;
    let mut lightning_hand = lure;
    lightning_hand.z = lightning_hand
        .z
        .wrapping_add(simulation(&game).entities()[usize::from(player)].array_fov());
    let provocation_aim = crate::level1_mission::projectile_guidance_angles(
        lightning_hand,
        Position {
            z: original.position.z.wrapping_add(original.array_fov()),
            ..original.position
        },
    );
    public_step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.lightning").unwrap(),
                tier: 0,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
        "declared LightningI public assignment",
        0,
    );
    // Establish this ONE predeclared aim after the public hand assignment.
    // C4 spent the helper's single held-flight visit on that assignment:
    // the next neutral cast input added -1947/4=-486, giving pitch1461.
    // No target pose is read again and no later command updates this aim.
    simulation_mut(&mut game).set_level2_player_aim_for_test(provocation_aim.0, provocation_aim.1);
    let lightning_holder = simulation(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[7];
    let lightning_holder_handle = simulation(&game)
        .source_arena()
        .handle(lightning_holder)
        .unwrap();
    let lightning_price = simulation(&game)
        .shared_spells()
        .pickup(lightning_holder)
        .unwrap()
        .max_mana;
    assert!(lightning_price > 0 && !simulation(&game).shared_spells().free_spell_usage);
    let initial_rays = simulation(&game)
        .entities()
        .iter()
        .filter(|row| row.active && (row.class, row.model, row.source_id) == (9, 9, player))
        .filter_map(|row| simulation(&game).source_arena().handle(row.index))
        .collect::<Vec<_>>();
    let mut provocation_rays: Vec<(
        crate::campaign::source_arena::SourceEntityHandle,
        crate::native_world::EntitySnapshot,
    )> = Vec::new();
    let mut provocation_visits: Vec<(
        area_damage::PlayerDamageConsumptionReceipt,
        crate::native_world::EntitySnapshot,
    )> = Vec::new();
    let mut lightning_paid = 0_i32;
    lightning::start_endpoint_birth_trace();
    web::start_trace(root);
    area_damage::start_actor_damage_trace(&[player_handle, root]);
    let mut provocation_effects = Vec::new();
    let initial_root_mailbox =
        area_damage::source_bounty(simulation(&game).source_arena(), None, root.source_slot)
            .unwrap();
    assert_eq!(initial_root_mailbox, (0, 0));
    let mut root_health_frames = Vec::new();
    let mut foreign300_reported = false;
    let mut player_consumptions = Vec::new();
    let mut immobilized = false;
    let mut recovered = false;
    let mut audible = false;
    let mut web_first_hit_audio = false;
    let mut web_sound_receipts = Vec::new();
    let mut moved = false;
    let mut movement_probe = None;
    let mut history = Vec::new();
    let mut cadence_history = Vec::new();
    let mut close_boundary = None;
    let mut close_contacts = Vec::new();
    let mut close_hit = false;
    for turn in 0..192 {
        let before = &simulation(&game).entities()[usize::from(root.source_slot)];
        let entry_life = before.life;
        let entry = (
            before.action,
            before.phase,
            before.record[74],
            before.source_dword_10(),
            before.position,
            before.yaw,
            before.source_word_96(),
        );
        let entry_player = (
            simulation(&game).state.position,
            simulation(&game).state.immobilize_counter,
            simulation(&game).state.immobilize_subcounter,
        );
        let mana_before = simulation(&game).shared_spells().player_entity().mana;
        simulation_mut(&mut game).start_creature17_trace();
        area_damage::start_effect_damage_trace(root.source_slot);
        public_step(
            &mut game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
            "ordinary web/contact",
            turn,
        );
        let sound_trace = simulation_mut(&mut game).take_creature17_trace();
        provocation_effects.extend(area_damage::take_effect_damage_trace());
        close_contacts.extend(sound_trace.contacts);
        let sim = simulation(&game);
        lightning_paid = lightning_paid.saturating_add(
            mana_before
                .saturating_sub(sim.shared_spells().player_entity().mana)
                .max(0),
        );
        for ray in sim.entities().iter().filter(|row| {
            row.active
                && (row.class, row.model, row.source_id, row.subspell_index) == (9, 9, player, 200)
                && u16::from_le_bytes([row.record[42], row.record[43]]) == lightning_holder
        }) {
            if let Some(handle) = sim.source_arena().handle(ray.index)
                && !initial_rays.contains(&handle)
                && !provocation_rays.iter().any(|(old, _)| *old == handle)
            {
                provocation_rays.push((handle, ray.clone()));
            }
        }
        assert_eq!(sim.source_arena().handle(root.source_slot), Some(root));
        let row = &sim.entities()[usize::from(root.source_slot)];
        assert!(
            row.life > 0,
            "ordinary lure must retain the original living Spider"
        );
        moved |= row.position != original.position;
        let ticks = game.snapshot().value.player_immobilized_ticks;
        assert_eq!(
            ticks,
            u16::from(sim.state.immobilize_counter.cast_unsigned()),
            "actual overlay DTO uses Flight authority"
        );
        audible |= sim.effect_journal().records().iter().any(|record|
            matches!(record.effect, crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                audio_site: crate::MissionAudioSite::SpiderAttack, entity_index, sound_id: 32, player_index: -1,
            }) if entity_index == root.source_slot)
                && record.audio.is_some_and(|audio| audio.command.audible && audio.play_type == 3));
        for (impact, target, sequence) in sound_trace.web_first_hit_sounds {
            if target == player_handle && sim.effect_journal().records().iter().any(|record|
                record.sequence == sequence && matches!(record.effect,
                    crate::campaign::journal::CampaignEffect::Flight(crate::FlightEvent::PrepareSound {
                        audio_site: crate::MissionAudioSite::WebFirstHit, entity_index,
                        sound_id: 54..=57, player_index: -1,
                    }) if entity_index == player)
                    && record.audio.is_some_and(|audio| audio.command.audible && audio.play_type == 3)) {
                web_sound_receipts.push((impact, target, sequence));
            }
        }
        let current_consumptions = area_damage::take_player_damage_consumptions();
        for visit in current_consumptions.iter().filter(|visit| {
            visit.target == root
                && visit.life_before == 5500
                && visit.life_after == 5300
                && visit.mailbox == (200, player)
        }) {
            if entry_life == visit.life_before
                && row.life == visit.life_after
                && row.action == 162
                && row.source_word_96() == player
                && row.phase == entry.1.wrapping_add(1)
                && u16::from_le_bytes([row.record[102], row.record[103]]) == 0
            {
                provocation_visits.push((*visit, row.clone()));
            }
        }
        let (traced_launches, traced_impacts) = web::trace_snapshot();
        let is_own_web = |visit: &area_damage::PlayerDamageConsumptionReceipt| {
            visit.target == player_handle
                && visit.mailbox.1 == original.source_id
                && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0)
                && visit.writer.is_some_and(|writer| {
                    writer.target == player_handle
                        && writer.mailbox == visit.mailbox
                        && traced_impacts.iter().any(|impact| {
                            writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                                && close_contacts.iter().any(|contact| {
                                    contact.producer == impact.impact
                                        && contact.current_producer == Some(impact.impact)
                                        && contact.target == player_handle
                                        && (contact.payload, contact.attacker) == (780, original.source_id)
                                        && contact.writer_sequence == Some(writer.sequence)
                                        && contact.mailbox_after == writer.mailbox
                                        && contact.mailbox_after == (
                                            if contact.mailbox_before.1 != 0 {
                                                780
                                            } else {
                                                contact.mailbox_before.0.wrapping_add(780)
                                            },
                                            original.source_id,
                                        )
                                        && contact.player_life_after == contact.player_life_before
                                        && visit.event_cursor >= contact.event_cursor
                                })
                                && (
                                    impact.packet.class,
                                    impact.packet.model,
                                    impact.packet.subspell_index,
                                ) == (10, 66, 780)
                                && web_sound_receipts.iter().any(|&(source, target, _)| {
                                    source == impact.impact && target == player_handle
                                })
                                && traced_launches.iter().any(|launch| {
                                    launch.spider == root
                                        && launch.projectile == impact.projectile
                                        && launch.target == player
                                        && (
                                            launch.packet.class,
                                            launch.packet.model,
                                            launch.packet.subspell_index,
                                        ) == (9, 21, 780)
                                })
                        })
                })
        };
        let fresh_own_web = current_consumptions.iter().any(&is_own_web);
        if fresh_own_web && ticks != 0 {
            // Competitors cannot start this sequence or the movement probe.
            // The current780 writer, original projectile/impact generations
            // and prepared first-hit audio are all proven before placement.
            immobilized = true;
            web_first_hit_audio = true;
            if movement_probe.is_none() {
                movement_probe = Some(game.backend().compatibility_campaign().clone());
            }
        }
        if immobilized && ticks == 0 {
            recovered = true;
        }
        // Source11900 contributes100, but a consumed ownerless mailbox can
        // retain an earlier payload. Require the actual write arithmetic and
        // the SAME accepted writer to reach real subtraction/DamageApplied;
        // neither close geometry nor a guessed final mailbox100 suffices.
        for consumed in &current_consumptions {
            if consumed.target != player_handle {
                continue;
            }
            let Some(writer) = consumed.writer else {
                continue;
            };
            let Some(contact) = close_contacts.iter().find(|contact| {
                contact.producer == root
                    && contact.current_producer == Some(root)
                    && contact.target == player_handle
                    && contact.payload == 100
                    && contact.attacker == original.source_id
                    && contact.writer_sequence == Some(writer.sequence)
            }) else {
                continue;
            };
            let expected = if contact.mailbox_before.1 != 0 {
                contact.payload
            } else {
                contact.mailbox_before.0.wrapping_add(contact.payload)
            };
            if contact.mailbox_after == (expected, original.source_id)
                && contact.player_life_after == contact.player_life_before
                && writer.source == area_damage::PlayerMailboxSource::Contact(root)
                && writer.target == player_handle
                && writer.mailbox == contact.mailbox_after
                && consumed.mailbox == contact.mailbox_after
                && consumed.event_cursor >= contact.event_cursor
                && consumed.life_after == consumed.life_before.wrapping_sub(consumed.mailbox.0)
                && matches!(sim.economy().events().get(consumed.event_cursor),
                    Some(Level2EconomyEvent::DamageApplied { target_entity_index, attacker_entity_index,
                        damage, lethal: false }) if *target_entity_index == player
                            && *attacker_entity_index == original.source_id && *damage == consumed.mailbox.0)
            {
                if !close_hit {
                    eprintln!(
                        "actual original137 contact100: write={contact:?} consumed={consumed:?}"
                    );
                }
                close_hit = true;
            }
        }
        player_consumptions.extend(current_consumptions);
        root_health_frames.push((
            provocation_effects.len(),
            player_consumptions.len(),
            row.life,
            row.max_life,
            area_damage::source_bounty(sim.source_arena(), None, root.source_slot).unwrap(),
        ));
        // Emit the first real foreign300 writer/consumption before any
        // player-placement guard, using only the existing collected receipts.
        if !foreign300_reported {
            if let Some((effect, consumed)) = provocation_effects.iter().filter(|effect| {
                effect.target_slot == root.source_slot
                    && effect.target_generation == root.generation
                    && effect.pulse.payload == 300
                    && effect.pulse.owner != player
                    && effect.writer_sequence.is_some()
            }).find_map(|effect| {
                player_consumptions.iter().find(|visit| {
                    visit.target == root && visit.writer.is_some_and(|writer|
                        Some(writer.sequence) == effect.writer_sequence)
                }).map(|consumed| (effect, consumed))
            }) {
                eprintln!("actual foreign300 Spider damage: effect={effect:?} consumed={consumed:?}");
                foreign300_reported = true;
            }
        }
        let own_web_consumed = player_consumptions.iter().any(&is_own_web);
        let close = close_hit;
        if turn < 8 || turn % 8 == 0 {
            history.push((
                turn,
                row.action,
                row.phase,
                row.record[74],
                row.position,
                row.source_word_96(),
                sim.state.position,
                sim.state.immobilize_counter,
                sim.state.immobilize_subcounter,
            ));
        }
        if matches!(entry.1, 33..=37 | 68..=73 | 103..=107 | 138..=143) {
            cadence_history.push((
                turn,
                entry,
                entry_player,
                (
                    row.action,
                    row.phase,
                    row.record[74],
                    row.source_dword_10(),
                    row.position,
                    row.yaw,
                    row.source_word_96(),
                ),
                (
                    sim.state.position,
                    sim.state.immobilize_counter,
                    sim.state.immobilize_subcounter,
                ),
            ));
        }
        // At most ONE declared player-only contact-boundary placement,
        // after an actual own780 subtraction and before a source cadence35
        // visit while charge2 is still live. This does not change NPC phase,
        // remaining32-visit timer, target, pose or RNG. Later reads never
        // track the player to the actor again. The strict source100 contact
        // and final full-generation web writer remain independently required.
        let contact_pose = if close_boundary.is_none()
            && !close
            && own_web_consumed
            && immobilized
            && row.action == 162
            && row.record[74] == 2
            && row.phase % RULES.scan_period == 0
        {
            let mut position = row.position;
            crate::move_entity(&mut position, row.yaw, 0, 512);
            position.z = row.position.z.checked_add(256).unwrap().max(
                sim.canonical_terrain().ground_height(position).checked_add(256).unwrap()
            );
            assert!(
                position.z
                    >= sim
                        .canonical_terrain()
                        .ground_height(position)
                        .wrapping_add(256),
                "single contact placement must clear actual floor: root={root:?} row={row:?} pose={position:?}"
            );
            if let Some(ceiling) = sim.canonical_terrain().ceiling_height(position) {
                assert!(
                    position.z <= ceiling.wrapping_sub(256),
                    "single contact placement must fit actual Cave ceiling"
                );
            }
            Some((row.phase, position))
        } else {
            None
        };
        if let Some((phase, position)) = contact_pose {
            simulation_mut(&mut game)
                .set_level2_entity_position(player, position)
                .unwrap();
            close_boundary = Some((turn, phase, position));
        }
        if moved && audible && web_first_hit_audio && immobilized && recovered && close {
            break;
        }
    }
    let endpoint_births = lightning::take_endpoint_birth_trace();
    let provoked = provocation_visits.iter().any(|(visit, after)| {
        visit.target == root
            && visit.life_after == visit.life_before.wrapping_sub(200)
            && after.life == 5300
            && after.action == 162
            && after.source_word_96() == player
            && visit.writer.is_some_and(|writer| {
                writer.target == root
                    && writer.mailbox == visit.mailbox
                    && endpoint_births.iter().any(|birth| {
                        birth.owner == player
                            && writer.source
                                == area_damage::PlayerMailboxSource::Effect(birth.endpoint)
                            && provocation_rays.iter().any(|(ray, packet)| {
                                *ray == birth.ray
                                    && (
                                        packet.class,
                                        packet.model,
                                        packet.source_id,
                                        packet.subspell_index,
                                    ) == (9, 9, player, 200)
                                    && u16::from_le_bytes([packet.record[42], packet.record[43]])
                                        == lightning_holder
                            })
                            && provocation_effects.iter().any(|effect| {
                                effect.source_scoped
                                    && effect.source_slot == Some(birth.endpoint.source_slot)
                                    && effect.target_slot == root.source_slot
                                    && effect.target_generation == root.generation
                                    && effect.pulse.owner == player
                                    && effect.pulse.payload == 200
                                    && effect.writer_sequence == Some(writer.sequence)
                            })
                    })
            })
    });
    assert!(
        provoked
            && provocation_rays.len() == 1
            && lightning_paid >= lightning_price
            && simulation(&game).source_arena().handle(lightning_holder)
                == Some(lightning_holder_handle),
        "one real paid LightningI must nonfatally provoke original137: root={root:?} sourceID={} pose={lure:?} aim={provocation_aim:?} paid={lightning_paid} price={lightning_price} holder={lightning_holder_handle:?} rays={provocation_rays:?} endpoints={endpoint_births:?} visits={provocation_visits:?} effects={provocation_effects:?} cadence={cadence_history:?}",
        original.source_id
    );
    // Source10C80 ordinary writes add to owned payload and replace ownerless
    // residue. Merge by actual writer sequence, not an ambiguous event cursor;
    // a queued pulse may be consumed on the following ordinary source pass.
    let fold_root_write = |effect: &area_damage::EffectDamageReceipt,
                           mailbox: &mut (i32, u16),
                           pending: &mut Vec<u64>,
                           represented: &mut BTreeSet<u64>,
                           last_source: &mut Option<SourceEntityHandle>| {
        assert_eq!(
            (effect.target_slot, effect.target_generation),
            (root.source_slot, root.generation)
        );
        let Some(sequence) = effect.writer_sequence else {
            assert_eq!(effect.mailbox_before, effect.mailbox_after);
            return;
        };
        assert!(effect.source_scoped && !effect.pulse.continuous);
        assert!(effect.hit_count.is_some_and(|hits| hits != 0));
        // Typed carriers can own an address with an inactive raw shadow.
        // Use the same canonical incarnation as the actual accepted writer;
        // raw header snapshots remain diagnostic, not producer admission.
        let source = effect.source_handle
            .expect("the accepted callback needs its canonical producer handle");
        assert_eq!(effect.source_slot, Some(source.source_slot));
        assert_eq!(effect.mailbox_before, Some(*mailbox));
        let after = (
            if mailbox.1 != 0 {
                mailbox.0.wrapping_add(i32::from(effect.pulse.payload))
            } else {
                i32::from(effect.pulse.payload)
            },
            effect.pulse.owner,
        );
        assert_eq!(effect.mailbox_after, Some(after));
        assert!(
            represented
                .last()
                .is_none_or(|previous| *previous < sequence)
        );
        assert!(represented.insert(sequence));
        pending.push(sequence);
        *last_source = Some(source);
        *mailbox = after;
    };
    let mut checked_root_life = 5500_i32;
    let mut checked_root_mailbox = initial_root_mailbox;
    let mut next_health_effect = 0;
    let mut next_health_consumption = 0;
    let mut pending_health_writers = Vec::new();
    let mut represented_health_writers = BTreeSet::new();
    let mut consumed_health_writers = BTreeSet::new();
    let mut latest_health_source = None;
    for &(effects_end, consumptions_end, actual_life, actual_max, actual_mailbox) in
        &root_health_frames
    {
        for visit in player_consumptions[next_health_consumption..consumptions_end]
            .iter()
            .filter(|visit| visit.target == root)
        {
            let writer = visit
                .writer
                .expect("every original-root subtraction needs its actual writer");
            while next_health_effect < effects_end
                && provocation_effects[next_health_effect]
                    .writer_sequence
                    .is_none_or(|sequence| sequence <= writer.sequence)
            {
                fold_root_write(
                    &provocation_effects[next_health_effect],
                    &mut checked_root_mailbox,
                    &mut pending_health_writers,
                    &mut represented_health_writers,
                    &mut latest_health_source,
                );
                next_health_effect += 1;
            }
            assert_eq!(writer.target, root);
            assert_eq!(pending_health_writers.last(), Some(&writer.sequence));
            assert_eq!(
                writer.source,
                area_damage::PlayerMailboxSource::Effect(latest_health_source.unwrap())
            );
            assert_eq!(
                (writer.mailbox, visit.mailbox),
                (checked_root_mailbox, checked_root_mailbox)
            );
            assert_eq!(visit.life_before, checked_root_life);
            checked_root_life = checked_root_life.wrapping_sub(checked_root_mailbox.0);
            assert_eq!(visit.life_after, checked_root_life);
            for sequence in pending_health_writers.drain(..) {
                assert!(
                    consumed_health_writers.insert(sequence),
                    "one consumption per writer"
                );
            }
            checked_root_mailbox.1 = 0; // Source1C310 clears owner, retaining payload.
        }
        next_health_consumption = consumptions_end;
        while next_health_effect < effects_end {
            fold_root_write(
                &provocation_effects[next_health_effect],
                &mut checked_root_mailbox,
                &mut pending_health_writers,
                &mut represented_health_writers,
                &mut latest_health_source,
            );
            next_health_effect += 1;
        }
        assert_eq!(
            (actual_life, actual_max, actual_mailbox),
            (checked_root_life, 5500, checked_root_mailbox),
            "every ordinary frame must preserve the exact original5500HP writer/consumer chain"
        );
    }
    assert_eq!(next_health_effect, provocation_effects.len());
    assert_eq!(next_health_consumption, player_consumptions.len());
    assert!(pending_health_writers.is_empty());
    assert_eq!(checked_root_mailbox.1, 0);
    assert_eq!(represented_health_writers, consumed_health_writers);
    eprintln!(
        "actual original5500HP Spider chain: endpoint={checked_root_life} writers={consumed_health_writers:?} consumed={:?}",
        player_consumptions
            .iter()
            .filter(|visit| visit.target == root)
            .collect::<Vec<_>>()
    );
    let (web_launches, web_impacts) = web::take_trace();
    let web_hit = player_consumptions.iter().any(|visit| {
        visit.target == player_handle
            && visit.mailbox.1 == original.source_id
            && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0)
            && visit.writer.is_some_and(|writer| {
                writer.target == player_handle
                    && writer.mailbox == visit.mailbox
                    && web_impacts.iter().any(|impact| {
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                            && close_contacts.iter().any(|contact| {
                                contact.producer == impact.impact
                                    && contact.current_producer == Some(impact.impact)
                                    && contact.target == player_handle
                                    && (contact.payload, contact.attacker) == (780, original.source_id)
                                    && contact.writer_sequence == Some(writer.sequence)
                                    && contact.mailbox_after == writer.mailbox
                                    && contact.mailbox_after == (
                                        if contact.mailbox_before.1 != 0 {
                                            780
                                        } else {
                                            contact.mailbox_before.0.wrapping_add(780)
                                        },
                                        original.source_id,
                                    )
                                    && contact.player_life_after == contact.player_life_before
                                    && visit.event_cursor >= contact.event_cursor
                            })
                            && web_sound_receipts.iter().any(|&(source, target, _)| {
                                source == impact.impact && target == player_handle
                            })
                            && (
                                impact.packet.class,
                                impact.packet.model,
                                impact.packet.subspell_index,
                            ) == (10, 66, 780)
                            && web_launches.iter().any(|launch| {
                                launch.spider == root
                                    && launch.projectile == impact.projectile
                                    && launch.target == player
                                    && (
                                        launch.packet.class,
                                        launch.packet.model,
                                        launch.packet.subspell_index,
                                    ) == (9, 21, 780)
                            })
                    })
            })
    });
    assert!(
        moved && audible && web_first_hit_audio && web_hit && close_hit && immobilized && recovered,
        "real original137 web/contact/overlay/recovery: root={root:?} sourceID={} initial={original:?} entry_roots={entry_roots:?} lure={lure:?} close_boundary={close_boundary:?} moved={moved} sound32={audible} web_first_hit_audio={web_first_hit_audio} web={web_hit} close={close_hit} immobilized={immobilized} recovered={recovered} launches={web_launches:?} impacts={web_impacts:?} web_sounds={web_sound_receipts:?} contacts={close_contacts:?} consumptions={player_consumptions:?} history={history:?} exact_entry_exit_cadences={cadence_history:?}",
        original.source_id
    );
    // A clone of the genuinely webbed state exercises actual player movement.
    // Identical fixed acceleration/coast intervals are independent of NPC pose;
    // both must retain the same translation while the status is active.
    let mut accelerate = movement_probe.unwrap();
    let mut coast = accelerate.clone();
    let mut suppressed = false;
    for turn in 0..5 {
        if accelerate.simulation().state.immobilize_counter == 0 {
            break;
        }
        accelerate.step(
            PlayerCommand {
                flags: 1,
                ..Default::default()
            },
            0,
        );
        coast.step(PlayerCommand::default(), 0);
        no_dispatch_errors(
            accelerate.simulation(),
            "actual immobilized acceleration",
            turn,
        );
        no_dispatch_errors(coast.simulation(), "actual immobilized coast", turn);
        assert_eq!(
            accelerate.simulation().state.position,
            coast.simulation().state.position
        );
        suppressed |=
            accelerate.simulation().state.actual_speed != coast.simulation().state.actual_speed;
    }
    assert!(
        suppressed,
        "positive player speed input is suppressed by actual web movement status"
    );

    // The movement clones share observer TLS and physical handle values, but
    // not this live session. Restart only the observers after asserting that
    // the real root has no pending owned write at this observation boundary.
    assert_eq!(
        simulation(&game).source_arena().handle(root.source_slot),
        Some(root)
    );
    assert_eq!(
        area_damage::source_bounty(simulation(&game).source_arena(), None, root.source_slot)
            .unwrap(),
        checked_root_mailbox
    );
    assert_eq!(checked_root_mailbox.1, 0);
    assert_eq!(
        simulation(&game).entities()[usize::from(root.source_slot)].life,
        checked_root_life
    );
    area_damage::start_actor_damage_trace(&[root]);
    area_damage::start_effect_damage_trace(root.source_slot);
    public_step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.meteor").unwrap(),
                tier: 1,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
        "restore declared MeteorII public hand",
        0,
    );
    let restore_consumptions = area_damage::take_player_damage_consumptions();
    let restore_effects = area_damage::take_effect_damage_trace();
    assert!(
        restore_consumptions.is_empty(),
        "quiet real hand assignment: {restore_consumptions:?}"
    );
    assert!(
        restore_effects
            .iter()
            .all(|effect| effect.writer_sequence.is_none()
                && effect.mailbox_before == effect.mailbox_after),
        "no unaccounted accepted root write during real hand assignment: {restore_effects:?}"
    );
    assert_eq!(
        area_damage::source_bounty(simulation(&game).source_arena(), None, root.source_slot)
            .unwrap(),
        checked_root_mailbox
    );

    // One declared player-only fixed firing placement.1024 behind the current
    // actor leaves the256 hand offset within the source113 acquisition cone.
    // Aim is computed once; no subsequent pose/life read changes the command.
    // TierII supplies five1600 pulses against the exact verified life chain,
    // with stationary half-widths0/384/768/1152/1536. C7's tierIII also killed
    // original138; the smaller tierII area must still pass the unchanged
    // five-survivor and +1 credit checks below.
    let target = simulation(&game).entities()[usize::from(root.source_slot)].clone();
    assert_eq!(
        (target.life, target.max_life),
        (checked_root_life, 5500),
        "original5500HP must match every actual admitted writer/subtraction, including the real hand-assignment boundary"
    );
    let mut firing_pose = target.position;
    crate::move_entity(&mut firing_pose, target.yaw, 0, -1024);
    firing_pose.z = simulation(&game)
        .canonical_terrain()
        .ground_height(firing_pose)
        .wrapping_add(384);
    if let Some(ceiling) = simulation(&game)
        .canonical_terrain()
        .ceiling_height(firing_pose)
    {
        assert!(
            firing_pose.z <= ceiling.wrapping_sub(256),
            "fixed shot must fit actual Cave geometry"
        );
    }
    let mut hand = firing_pose;
    hand.z = hand
        .z
        .wrapping_add(simulation(&game).entities()[usize::from(player)].array_fov());
    let target_top = Position {
        z: target.position.z.wrapping_add(target.array_fov()),
        ..target.position
    };
    let firing_aim = crate::level1_mission::projectile_guidance_angles(hand, target_top);
    {
        let sim = simulation_mut(&mut game);
        sim.set_level2_entity_position(player, firing_pose).unwrap();
        sim.state.actual_speed = 0;
        sim.state.target_speed = 0;
        sim.set_level2_player_aim_for_test(firing_aim.0, firing_aim.1);
    }
    let mut no_cast = game.backend().compatibility_campaign().clone();
    let sim = simulation(&game);
    let holder = sim.shared_spells().local_player().enabled_spell_holders[9];
    let holder_handle = sim.source_arena().handle(holder).unwrap();
    assert!(!sim.shared_spells().free_spell_usage);
    let credit_before = sim.economy().authored_bee_kill_census(player);
    // Capture identities BEFORE any Meteor allocation, hide or reuse. The
    // legacy-named Sentinel credit observer covers Spider's shared dispatch
    // too; a later occupant cannot identify a reclaimed credited victim.
    let census = sim.creature_runtime_census();
    let live = &sim.session.state.live;
    let context = crate::native_world::source_target::Context {
        spells: Some(&live.shared_spells),
        local_player_state: Some(&live.core.state),
        particles: Some(&sim.session.definition.presentation.particles),
        map_type: Some(live.terrain.map_type()),
        ..crate::native_world::source_target::Context::from_ordinary(&live.goats, &live.ordinary)
    };
    let mut pre_meteor_class5 = Vec::new();
    for slots in &sim.source_arena().rebuilt_class5_model_slots() {
        for &slot in slots {
            let current =
                crate::native_world::source_target::read(sim.source_arena(), context, slot)
                    .unwrap()
                    .unwrap();
            if current.active
                && current.class == 5
                && current.life >= 0
                && current.control_flags & ENTITY_DRAW_DISABLED_BIT == 0
            {
                let handle = sim.source_arena().handle(slot).unwrap();
                let identity = census
                    .entries()
                    .iter()
                    .find(|identity| identity.handle == handle)
                    .copied();
                pre_meteor_class5.push((
                    handle,
                    current.model,
                    current.source_id,
                    current.position,
                    current.life,
                    identity,
                ));
            }
        }
    }
    let other_roots = roots
        .iter()
        .filter_map(|&(_, handle)| (handle != root).then_some(handle))
        .collect::<Vec<_>>();
    area_damage::start_actor_damage_trace(&[root]);
    area_damage::start_effect_damage_trace(root.source_slot);
    crate::native_world::creatures::creature_18::mc2::start_kill_credit_trace();
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut consumptions = Vec::new();
    let mut hidden = false;
    let mut dto_lost = false;
    let mut mana_dto = false;
    let mut death_sphere = None;
    let mut burst_dto = false;
    let mut paid = 0_i32;
    let mut turns = 0;
    let mut shot_history = Vec::new();
    for turn in 0..160 {
        let before_mana = simulation(&game).shared_spells().player_entity().mana;
        let event_start = simulation(&game).economy().events().len();
        simulation_mut(&mut game).start_creature17_trace();
        public_step(
            &mut game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
            "one paid fixed Meteor",
            turn,
        );
        turns += 1;
        let sim = simulation_mut(&mut game);
        paid = paid.saturating_add(
            before_mana
                .saturating_sub(sim.shared_spells().player_entity().mana)
                .max(0),
        );
        let trace = sim.take_creature17_trace();
        launches.extend(
            trace
                .meteor_launches
                .into_iter()
                .filter(|launch| launch.holder == holder_handle && launch.caster == player_handle),
        );
        impacts.extend(trace.meteor_impacts);
        consumptions.extend(area_damage::take_player_damage_consumptions());
        let views = sim.campaign_entity_presentations(0).unwrap();
        if sim.source_arena().handle(root.source_slot) == Some(root) {
            let row = &sim.entities()[usize::from(root.source_slot)];
            if turn < 16 || turn % 8 == 0 {
                shot_history.push((
                    turn,
                    row.life,
                    row.action,
                    row.phase,
                    row.position,
                    u16::from_le_bytes([row.record[102], row.record[103]]),
                ));
            }
            if row.drawing_disabled() {
                hidden = true;
                dto_lost |= !views
                    .iter()
                    .any(|view| view.entity_index == root.source_slot);
            }
        }
        for event in &sim.economy().events()[event_start..] {
            if let Level2EconomyEvent::ManaSphereSpawned {
                sphere_id,
                source_entity_index,
                mana,
            } = event
                && *source_entity_index == root.source_slot
                && *mana == 2750
            {
                let slot = u16::try_from(*sphere_id).unwrap();
                death_sphere = Some((sim.source_arena().handle(slot).unwrap(), *mana));
                mana_dto |= sim.economy().spheres().iter().any(|sphere| {
                    sphere.id == *sphere_id
                        && sphere.active
                        && sphere.mana >= *mana
                        && views
                            .iter()
                            .any(|view| view.entity_index == sphere.source_entity_index)
                });
            }
        }
        for (&slot, &(generation, parent)) in sim.source_arena().effects.death_burst_origins() {
            if parent == root.source_slot && sim.source_arena().slot_generation(slot) == generation
            {
                burst_dto |= views.iter().any(|view| view.entity_index == slot);
            }
        }
        if hidden && dto_lost && mana_dto && burst_dto && sim.source_arena().resolve(root).is_none()
        {
            break;
        }
    }
    let effects = area_damage::take_effect_damage_trace();
    let credits = crate::native_world::creatures::creature_18::mc2::take_kill_credit_trace();
    for turn in 0..turns {
        no_cast.step(PlayerCommand::default(), 0);
        no_dispatch_errors(no_cast.simulation(), "equal elapsed no-cast", turn);
    }
    let causal_fatal = consumptions.iter().any(|visit| {
        visit.target == root
            && visit.life_before >= 0
            && visit.life_after < 0
            && visit.life_after == visit.life_before.wrapping_sub(visit.mailbox.0)
            && visit.mailbox.1 == player
            && visit.writer.is_some_and(|writer| {
                writer.target == root
                    && writer.mailbox == visit.mailbox
                    && impacts.iter().any(|impact| {
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                            && (
                                impact.state.source_class,
                                impact.state.model,
                                impact.state.subspell_index,
                            ) == (10, 17, 8000)
                            && launches.iter().any(|launch| {
                                launch.projectile == impact.projectile
                                    && launch.holder == holder_handle
                                    && launch.caster == player_handle
                                    && (
                                        launch.packet.class,
                                        launch.packet.model,
                                        launch.packet.subspell_index,
                                    ) == (9, 3, 8000)
                            })
                            && effects.iter().any(|receipt| {
                                receipt.source_scoped
                                    && receipt.source_slot == Some(impact.impact.source_slot)
                                    && receipt.target_slot == root.source_slot
                                    && receipt.target_generation == root.generation
                                    && receipt.pulse.owner == player
                                    && receipt.pulse.payload == 1600
                                    && receipt.writer_sequence == Some(writer.sequence)
                            })
                    })
            })
    });
    assert!(
        causal_fatal,
        "one real paid holder/caster→9/3→10/17 writer must cause original137 fatal subtraction: root={root:?} pose={firing_pose:?} aim={firing_aim:?} launches={launches:?} impacts={impacts:?} consumed={consumptions:?} history={shot_history:?}"
    );
    assert_eq!(launches.len(), 1);
    assert!(paid >= METEOR_PROPERTIES.tiers[1].mana_cost);
    let sim = simulation(&game);
    let named_credits = credits
        .iter()
        .map(|credit| {
            (
                credit,
                pre_meteor_class5
                    .iter()
                    .find(|entry| entry.0 == credit.source),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sim.economy().authored_bee_kill_census(player),
        credit_before.wrapping_add(1),
        "unchanged +1 aggregate gate: credit_before={credit_before} pre_meteor_class5={pre_meteor_class5:?} named_credits={named_credits:?}"
    );
    assert!(hidden && dto_lost && mana_dto && burst_dto);
    assert!(sim.source_arena().resolve(root).is_none());
    assert!(
        !sim.creature_runtime_census()
            .roots_of_kind(CreatureKind::Spider)
            .any(|row| row.handle == root)
    );
    assert!(
        other_roots.iter().all(|&handle| sim
            .creature_identity_for_source_slot(handle.source_slot)
            .is_some_and(|row| row.handle == handle && row.living)),
        "other five originals still block family absence"
    );
    assert!(
        no_cast
            .simulation()
            .creature_identity_for_source_slot(root.source_slot)
            .is_some_and(|row| row.handle == root && row.living),
        "same elapsed no-cast keeps original137 alive"
    );
    assert_eq!(
        no_cast
            .simulation()
            .economy()
            .authored_bee_kill_census(player),
        credit_before
    );

    // Shared-combat collection continuation. Every original combat/credit/
    // lifetime/survivor assertion above remains a prerequisite. PossessionI
    // is a declared carried player loadout at this boundary, not an earned
    // pickup. Only one fixed public activation is made; no sphere/collector
    // pose, ownership, timer, RNG or lifetime is edited.
    let (sphere, birth_mana) = death_sphere.expect("original137 produced its real death sphere");
    assert_eq!(
        simulation(&game).source_arena().handle(sphere.source_slot),
        Some(sphere)
    );
    simulation_mut(&mut game)
        .apply_carried_spell_families(&[1, 7, 9], Some(1))
        .unwrap();
    public_step(
        &mut game,
        GameCommand {
            assign_spell: Some(SpellAssignment {
                spell_id: crate::game_definition::SemanticId::parse("spell.possession").unwrap(),
                tier: 0,
                hand: SpellAssignmentHand::Left,
                holder_actor: None,
            }),
            ..Default::default()
        },
        "declared collection PossessionI assignment",
        0,
    );
    assert_eq!(
        simulation(&game).source_arena().handle(sphere.source_slot),
        Some(sphere)
    );
    let target = simulation(&game)
        .possession_guidance_target_top_for_test(sphere.source_slot)
        .unwrap();
    let mut approach = Position {
        x: target.x.wrapping_sub(1024),
        y: target.y,
        z: target.z,
    };
    let terrain = simulation(&game).canonical_terrain();
    let floor = terrain.ground_height(approach).wrapping_add(128);
    let ceiling = terrain
        .ceiling_height(approach)
        .expect("actual Cave4 ceiling")
        .wrapping_sub(
            simulation(&game)
                .shared_spells()
                .player_entity()
                .array_fov(),
        )
        .wrapping_sub(crate::level1_mission::POSSESSION_PROJECTILE_COLLISION_HALF_HEIGHT);
    assert!(
        floor <= ceiling,
        "source-valid collection approach: target={target:?} floor={floor} ceiling={ceiling}"
    );
    approach.z = approach.z.clamp(floor, ceiling);
    simulation_mut(&mut game)
        .set_level2_entity_position(player, approach)
        .unwrap();
    simulation_mut(&mut game).state.actual_speed = 0;
    simulation_mut(&mut game).state.target_speed = 0;
    let mut hand = approach;
    hand.z = hand.z.wrapping_add(
        simulation(&game)
            .shared_spells()
            .player_entity()
            .array_fov(),
    );
    let aim = crate::level1_mission::projectile_guidance_angles(hand, target);
    simulation_mut(&mut game).set_level2_player_aim_for_test(aim.0, aim.1);
    let possession_holder = simulation(&game)
        .shared_spells()
        .local_player()
        .enabled_spell_holders[1];
    let possession_holder = simulation(&game)
        .source_arena()
        .handle(possession_holder)
        .unwrap();
    let mut no_possession = game.backend().compatibility_campaign().clone();
    let mut claim = false;
    let mut possession_paid = 0_i32;
    let mut flyers: Vec<(
        SourceEntityHandle,
        crate::native_world::Level2PossessionProjectile,
    )> = Vec::new();
    let mut ownership_effects: Vec<(
        SourceEntityHandle,
        crate::native_world::Level2PossessionEffect,
    )> = Vec::new();
    let mut claim_history = Vec::new();
    start_possession_trace();
    for turn in 0..32 {
        assert_eq!(
            simulation(&game).source_arena().handle(sphere.source_slot),
            Some(sphere)
        );
        let before = simulation(&game).shared_spells().player_entity().mana;
        let events = simulation(&game).economy().events().len();
        public_step(
            &mut game,
            GameCommand {
                cast_left: turn == 0,
                ..Default::default()
            },
            "one fixed public Possession of original death sphere",
            turn,
        );
        no_possession.step(PlayerCommand::default(), 0);
        no_dispatch_errors(
            no_possession.simulation(),
            "equal elapsed no-Possession",
            turn,
        );
        let sim = simulation(&game);
        possession_paid += before
            .saturating_sub(sim.shared_spells().player_entity().mana)
            .max(0);
        assert_eq!(sim.source_arena().handle(player), Some(player_handle));
        assert_eq!(
            sim.source_arena().handle(possession_holder.source_slot),
            Some(possession_holder)
        );
        for row in sim.economy().possession_projectiles().iter().filter(|row| {
            row.source_id_entity_index == player
                && row.holder_entity_index == possession_holder.source_slot
        }) {
            if let Some(handle) = sim.source_arena().handle(row.source_entity_index)
                && !flyers.iter().any(|(old, _)| *old == handle)
            {
                flyers.push((handle, *row));
            }
        }
        for row in sim
            .economy()
            .possession_effects()
            .iter()
            .filter(|row| row.source_id_entity_index == player)
        {
            if let Some(handle) = sim.source_arena().handle(row.source_entity_index)
                && flyers.iter().any(|(flyer, _)| {
                    flyer.source_slot == row.projectile_entity_index
                        && sim.source_arena().slot_generation(flyer.source_slot) == flyer.generation
                })
                && !ownership_effects.iter().any(|(old, _)| *old == handle)
            {
                ownership_effects.push((handle, *row));
            }
        }
        claim |= sim.economy().events()[events..].iter().any(|event| {
            matches!(event,
            Level2EconomyEvent::ManaSphereClaimed { sphere_id, owner_entity_index }
                if *sphere_id == u32::from(sphere.source_slot) && *owner_entity_index == player)
        });
        let current = sim
            .economy()
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == sphere.source_slot)
            .copied();
        claim_history.push((
            turn,
            sim.source_arena().handle(sphere.source_slot),
            current,
            sim.state.position,
        ));
        if claim {
            break;
        }
    }
    let possession_trace = take_possession_trace();
    assert!(
        claim && flyers.len() == 1 && !ownership_effects.is_empty(),
        "paid holder/caster→9/1→10/12 must claim original sphere: target={target:?} approach={approach:?} aim={aim:?} flyers={flyers:?} effects={ownership_effects:?} history={claim_history:?}"
    );
    assert!(possession_paid >= crate::level_definition::POSSESSION_PROPERTIES.tiers[0].mana_cost);
    assert_eq!(
        (
            flyers[0].1.source_class,
            flyers[0].1.source_model,
            flyers[0].1.action
        ),
        (9, 1, 1)
    );
    assert!(ownership_effects.iter().all(|(_, row)| (
        row.source_class,
        row.source_model,
        row.action
    ) == (10, 12, 12)));
    let sphere_writer = possession_trace.iter().rev().find_map(|receipt| match receipt {
        PossessionSourceReceipt::MailboxWrite { effect, target, source_id, amount, possessor, .. }
            if *target == sphere && *source_id == player && *amount == 0 && *possessor == player => Some(*effect),
        _ => None,
    }).unwrap_or_else(|| panic!("actual typed-sphere ownership mailbox needs its generation-bound effect: sphere={sphere:?} trace={possession_trace:?}"));
    assert!(
        possession_trace.iter().any(|receipt| matches!(receipt,
        PossessionSourceReceipt::ImpactBirth { effect, projectile, holder, owner, packet }
            if *effect == sphere_writer && *projectile == flyers[0].0
                && *holder == possession_holder && *owner == player_handle
                && (packet.source_class, packet.source_model, packet.action) == (10, 12, 12))),
        "the accepted original-sphere writer must come from this public holder/caster launch: {possession_trace:?}"
    );
    assert!(!simulation(&game).shared_spells().free_spell_usage);
    assert_eq!(
        simulation(&game).source_arena().handle(sphere.source_slot),
        Some(sphere)
    );
    assert_eq!(
        simulation(&game)
            .economy()
            .spheres()
            .iter()
            .find(|row| row.source_entity_index == sphere.source_slot)
            .unwrap()
            .player_entity_index,
        player
    );
    assert!(
        no_possession
            .simulation()
            .economy()
            .spheres()
            .iter()
            .any(|row| row.source_entity_index == sphere.source_slot
                && row.player_entity_index != player)
    );

    // Separate declared player-owned Castle resource prerequisite, AFTER the
    // real paid claim: preserve its established capacity but start collection
    // with an empty store. This is not earned mana, an upgrade, recharge, or a
    // relaxation of the source full-store gathering gate.
    let castle = simulation(&game)
        .economy()
        .castle_for_owner(player)
        .unwrap()
        .source_entity_index;
    let castle_handle = simulation(&game).source_arena().handle(castle).unwrap();
    assert_eq!(
        castle_handle, earned_castle,
        "continue the earned prefix's same Castle generation"
    );
    let capacity = simulation(&game)
        .economy()
        .castle_at(castle)
        .unwrap()
        .mana_capacity;
    for sim in [simulation_mut(&mut game), no_possession.simulation_mut()] {
        assert!(
            sim.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, 0)
        );
        assert_eq!(sim.economy().castle_at(castle).unwrap().mana, 0);
        assert_eq!(
            sim.economy().castle_at(castle).unwrap().mana_capacity,
            capacity
        );
    }
    let sphere_position = simulation(&game)
        .economy()
        .spheres()
        .iter()
        .find(|row| row.source_entity_index == sphere.source_slot)
        .unwrap()
        .position;
    let castle_position = simulation(&game)
        .economy()
        .castle_at(castle)
        .unwrap()
        .position;
    // Two48-unit collector legs plus the source1024/16 attraction tail,
    // vertical32-unit convergence and maintenance cadence. Inputs stay neutral.
    let travel = crate::distance_3d_wrapped(sphere_position, castle_position) as usize;
    let collection_bound = 2 * travel.div_ceil(48) + 192;
    let mut collected: Option<(SourceEntityHandle, i32)> = None;
    let mut hidden_pass = None;
    let mut reclaimed = false;
    let mut deposited = false;
    let mut pickup_count = 0;
    let mut credit_count = 0;
    let mut collection_history = Vec::new();
    for pass in 0..collection_bound {
        let sim = simulation(&game);
        let before_children = sim.economy().castle_children().to_vec();
        let before_store = sim.economy().castle_at(castle).unwrap().mana;
        let event_start = sim.economy().events().len();
        simulation_mut(&mut game).step_level2_source_pass((pass % 4) as u8);
        no_possession
            .simulation_mut()
            .step_level2_source_pass((pass % 4) as u8);
        no_dispatch_errors(simulation(&game), "real death-sphere collector", pass);
        no_dispatch_errors(
            no_possession.simulation(),
            "same elapsed unclaimed sphere",
            pass,
        );
        let sim = simulation(&game);
        assert_eq!(sim.source_arena().handle(castle), Some(castle_handle));
        let events = &sim.economy().events()[event_start..];
        for event in events {
            match *event {
                Level2EconomyEvent::CastleDefenderCollectedSphere {
                    defender_slot,
                    sphere_id,
                    mana,
                } if sphere_id == u32::from(sphere.source_slot) => {
                    pickup_count += 1;
                    assert_eq!(sim.source_arena().handle(sphere.source_slot), Some(sphere));
                    let retained = sim
                        .economy()
                        .spheres()
                        .iter()
                        .find(|row| row.source_entity_index == sphere.source_slot)
                        .unwrap();
                    assert_eq!(retained.mana, mana);
                    assert!(
                        mana >= birth_mana
                            && retained.pending_delete
                            && retained.control_flags & ENTITY_DRAW_DISABLED_BIT != 0
                    );
                    let before = before_children
                        .iter()
                        .find(|row| row.source_entity_index == defender_slot)
                        .unwrap();
                    let after = sim
                        .economy()
                        .castle_children()
                        .iter()
                        .find(|row| row.source_entity_index == defender_slot)
                        .unwrap();
                    assert_eq!(
                        before.mana, 0,
                        "this collector has no unrelated prior payload"
                    );
                    assert_eq!(after.mana, before.mana.wrapping_add(mana));
                    assert_eq!(after.id_entity_index, player);
                    collected = Some((sim.source_arena().handle(defender_slot).unwrap(), mana));
                    hidden_pass = Some(pass);
                    assert!(
                        !sim.campaign_entity_presentations(0)
                            .unwrap()
                            .iter()
                            .any(|view| view.entity_index == sphere.source_slot)
                    );
                }
                Level2EconomyEvent::CastleDefenderArrived {
                    defender_slot,
                    castle_id,
                    mana,
                } if collected.is_some_and(|(holder, payload)| {
                    holder.source_slot == defender_slot && payload == mana
                }) && castle_id == castle =>
                {
                    let (holder, payload) = collected.unwrap();
                    assert_eq!(sim.source_arena().handle(defender_slot), Some(holder));
                    assert_eq!(before_store, 0);
                    assert_eq!(
                        sim.economy().castle_at(castle).unwrap().mana,
                        before_store.wrapping_add(payload)
                    );
                    assert_eq!(
                        sim.economy()
                            .castle_children()
                            .iter()
                            .find(|row| row.source_entity_index == defender_slot)
                            .unwrap()
                            .mana,
                        0
                    );
                    credit_count += 1;
                    deposited = true;
                }
                _ => {}
            }
        }
        if hidden_pass.is_some_and(|hidden| pass == hidden + 1) {
            assert!(
                sim.source_arena().resolve(sphere).is_none(),
                "pickup reclaims at following source boundary"
            );
            reclaimed = true;
        }
        if pass < 8 || pass % 64 == 0 {
            collection_history.push((
                pass,
                sim.economy().castle_at(castle).copied(),
                sim.economy()
                    .spheres()
                    .iter()
                    .find(|row| row.source_entity_index == sphere.source_slot)
                    .copied(),
                sim.economy().castle_children().to_vec(),
            ));
        }
        if deposited && reclaimed {
            break;
        }
    }
    assert!(
        deposited && reclaimed && pickup_count == 1 && credit_count == 1,
        "same death sphere must lift/collect/credit once/reclaim naturally: sphere={sphere:?} collected={collected:?} pickup={pickup_count} credit={credit_count} hidden={hidden_pass:?} bound={collection_bound} history={collection_history:?}"
    );
    let credited = collected.unwrap().1;
    assert_eq!(
        simulation(&game).economy().castle_at(castle).unwrap().mana,
        credited
    );
    assert_eq!(
        no_possession
            .simulation()
            .economy()
            .castle_at(castle)
            .unwrap()
            .mana,
        0
    );
    simulation_mut(&mut game).step_level2_source_pass(0);
    no_possession.simulation_mut().step_level2_source_pass(0);
    no_dispatch_errors(simulation(&game), "following once-only Castle credit", 0);
    no_dispatch_errors(
        no_possession.simulation(),
        "following no-Possession boundary",
        0,
    );
    assert_eq!(
        simulation(&game).economy().castle_at(castle).unwrap().mana,
        credited
    );
    assert!(simulation(&game).source_arena().resolve(sphere).is_none());
    assert_eq!(
        simulation(&game).economy().authored_bee_kill_census(player),
        credit_before.wrapping_add(1)
    );
    assert!(simulation(&game).source_arena().resolve(root).is_none());
}

#[test]
fn configured_mission_four_all_six_spider_deaths_drive_scheduled_absence_and_reclaim() {
    use crate::PlayerCommand;
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::game_session::GameSession;
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::area_damage;

    let package = mission_four_root();
    let definition = load_mc2_campaign(
        &package,
        &crate::game_definition::default_catalog().unwrap(),
    )
    .unwrap();
    let mut game = GameSession::open(
        &Mc2CampaignAdapter,
        std::sync::Arc::new(definition),
        2004,
        Mc2CampaignInputs::new(&package),
    )
    .unwrap();
    earn_mission_four_mana_and_spawn_spiders(&mut game);
    // The explicitly configured lethal-contact continuation starts from the
    // same real earned stage transition and its unmodified six live roots.
    let mut session = simulation(&game).clone();
    let stage = 1;
    assert_eq!(
        game.backend()
            .compatibility_campaign()
            .definition()
            .level_definition()
            .objectives()[stage]
            .condition(),
        ObjectiveCondition::NoLivingCreatures(CreatureKind::Spider)
    );
    assert_eq!(session.runtime().stages[stage].kind, 7);
    let owner = usize::from(session.state.player_color_index);
    let player = session.state.entity_index;
    // Require an ordinary living blocked pass before the configured contacts.
    session.step_outer_turn(PlayerCommand::default(), 0);
    no_dispatch_errors(&session, "configured absence initial living pass", 0);
    assert_eq!(
        session.runtime().objectives[owner].stage_selectors[stage],
        1
    );
    assert_eq!(
        usize::from(session.runtime().objectives[owner].text_selector),
        stage
    );
    let roots = session
        .creature_runtime_census()
        .roots_of_kind(CreatureKind::Spider)
        .filter(|row| row.living)
        .map(|row| (row.origin, row.handle))
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 6);
    assert!(
        [137, 138, 139, 724, 725, 726]
            .into_iter()
            .all(|authored_record| roots.iter().any(
                |&(origin, _)| origin == CreatureRootOrigin::AuthoredRecord { authored_record }
            ))
    );
    let mut living_control = session.clone();
    let exit_publications = |events: &[crate::native_world::Mc2DispatchEvent]| {
        events
            .iter()
            .filter(|event| {
                matches!(event,
                    crate::native_world::Mc2DispatchEvent::AuthoredGroupActivated {
                        group: 45, authored_indices, ..
                    } if authored_indices.contains(&728))
            })
            .count()
    };
    let has_exit = |sim: &crate::native_world::Mc2Simulation| {
        sim.economy()
            .authored_switch_producers()
            .iter()
            .any(|row| row.active && row.authored_index == 728)
    };
    assert!(!has_exit(&session) && !has_exit(&living_control));
    let mut exit_group_count = 0;
    // Configured lethal contacts isolate the scheduled absence/lifecycle
    // consumer. The separate ordinary encounter owns real paid combat proof.
    for &(_, handle) in &roots {
        let life = session.entities()[usize::from(handle.source_slot)].life;
        area_damage::contact(
            &mut session.session.state.live.economy,
            None,
            handle.source_slot,
            u16::try_from(life + 1).unwrap(),
            player,
        )
        .unwrap();
        assert_eq!(
            session.entities()[usize::from(handle.source_slot)].life,
            life
        );
    }
    session.step_outer_turn(PlayerCommand::default(), 0);
    living_control.step_outer_turn(PlayerCommand::default(), 0);
    exit_group_count += exit_publications(session.dispatch_events());
    assert_eq!(exit_publications(living_control.dispatch_events()), 0);
    no_dispatch_errors(&session, "configured fatal source pass", 0);
    no_dispatch_errors(&living_control, "living negative fatal elapsed pass", 0);
    assert!(roots.iter().all(|&(_, handle)| {
        let row = &session.entities()[usize::from(handle.source_slot)];
        session.source_arena().handle(handle.source_slot) == Some(handle)
            && row.life == -1
            && row.action == 164
            && !row.drawing_disabled()
    }));
    assert!(session.source_arena().rebuilt_class5_model_slots()[20].is_empty());
    assert_eq!(
        session.runtime().objectives[owner].stage_selectors[stage],
        1,
        "fatal pass keeps the still-nonempty frozen membership for the objective"
    );
    session.step_outer_turn(PlayerCommand::default(), 0);
    living_control.step_outer_turn(PlayerCommand::default(), 0);
    exit_group_count += exit_publications(session.dispatch_events());
    assert_eq!(exit_publications(living_control.dispatch_events()), 0);
    no_dispatch_errors(&session, "following empty source boundary", 0);
    no_dispatch_errors(&living_control, "living negative following boundary", 0);
    assert_eq!(
        session.runtime().objectives[owner].stage_selectors[stage],
        2,
        "following pass freezes no living Spider before natural corpse hiding"
    );
    assert!(roots.iter().all(
        |&(_, handle)| session.source_arena().handle(handle.source_slot) == Some(handle)
            && session.entities()[usize::from(handle.source_slot)].action == 165
    ));
    let mut hidden = vec![false; roots.len()];
    let mut reclaimed = vec![false; roots.len()];
    for turn in 0..16 {
        session.step_outer_turn(PlayerCommand::default(), 0);
        living_control.step_outer_turn(PlayerCommand::default(), 0);
        exit_group_count += exit_publications(session.dispatch_events());
        assert_eq!(exit_publications(living_control.dispatch_events()), 0);
        no_dispatch_errors(&session, "configured natural hide/reclaim", turn);
        no_dispatch_errors(&living_control, "equal elapsed living negative", turn);
        let views = session.campaign_entity_presentations(0).unwrap();
        for (index, &(_, handle)) in roots.iter().enumerate() {
            if reclaimed[index] {
                continue;
            }
            if hidden[index] {
                assert!(
                    session.source_arena().resolve(handle).is_none(),
                    "following prologue loses original generation"
                );
                reclaimed[index] = true;
            } else {
                assert_eq!(
                    session.source_arena().handle(handle.source_slot),
                    Some(handle)
                );
                if session.entities()[usize::from(handle.source_slot)].drawing_disabled() {
                    assert!(
                        !views
                            .iter()
                            .any(|view| view.entity_index == handle.source_slot)
                    );
                    assert!(
                        !session
                            .economy()
                            .free_source_slots()
                            .contains(&handle.source_slot)
                    );
                    hidden[index] = true;
                }
            }
        }
        if reclaimed.iter().all(|seen| *seen) {
            break;
        }
    }
    assert!(hidden.iter().all(|seen| *seen) && reclaimed.iter().all(|seen| *seen));
    assert!(
        !session
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Spider)
            .any(|row| row.living)
    );
    assert_eq!(
        living_control.runtime().objectives[owner].stage_selectors[stage],
        1
    );
    assert!(
        roots.iter().all(|&(_, handle)| living_control
            .creature_identity_for_source_slot(handle.source_slot)
            .is_some_and(|row| row.handle == handle && row.living)),
        "same elapsed six genuine living roots block absence"
    );

    // The same objective result must drive the ordinary end gate and its
    // authored exit producer; the equally advanced living control cannot.
    for turn in 0..32 {
        if has_exit(&session) {
            break;
        }
        session.step_outer_turn(PlayerCommand::default(), 0);
        living_control.step_outer_turn(PlayerCommand::default(), 0);
        no_dispatch_errors(&session, "absence to exit group", turn);
        no_dispatch_errors(&living_control, "living negative exit group", turn);
        exit_group_count += exit_publications(session.dispatch_events());
        assert_eq!(exit_publications(living_control.dispatch_events()), 0);
    }
    assert_eq!(
        session.runtime().objectives[owner].stage_selectors[..2],
        [2, 2]
    );
    assert_eq!(session.runtime().objectives[owner].is_level_end, 1);
    assert_eq!(
        exit_group_count, 1,
        "the ordinary end gate publishes group 45 once"
    );
    let exit = session
        .economy()
        .authored_switch_producers()
        .iter()
        .find(|row| row.active && row.authored_index == 728)
        .expect("group 45 constructs the actual Cave exit trigger");
    assert_eq!((exit.model, exit.action), (12, 12));
    assert_eq!(
        living_control.runtime().objectives[owner].stage_selectors[stage],
        1
    );
    assert_eq!(living_control.runtime().objectives[owner].is_level_end, 0);
    assert!(!has_exit(&living_control));
    assert_cave_four_skeleton_switch(session, player);
}

fn assert_cave_four_skeleton_switch(mut session: crate::native_world::Mc2Simulation, player: u16) {
    use crate::campaign::journal::CampaignEffect;
    use crate::campaign::schedule::ScheduleReceipt;
    use crate::campaign::source_arena::{CreatureRootOrigin, SourceEntityHandle};
    use crate::native_world::{Mc2DispatchEvent, area_damage};
    use crate::{FlightEvent, FlightWorld, PlayerCommand};

    let groups = |sim: &crate::native_world::Mc2Simulation, expected_group: u16| {
        sim.dispatch_events().iter().find_map(|event| match event {
            Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot,
                group,
                authored_indices,
                ..
            } if *group == expected_group => Some((*source_slot, authored_indices.clone())),
            _ => None,
        })
    };
    let visit = |sim: &mut crate::native_world::Mc2Simulation, label, turn| {
        sim.step_outer_turn(PlayerCommand::default(), 0);
        no_dispatch_errors(sim, label, turn);
    };
    let place_at = |sim: &mut crate::native_world::Mc2Simulation, authored: u16| {
        let position = sim
            .economy()
            .authored_switch_producers()
            .iter()
            .find(|row| row.active && row.authored_index == authored)
            .expect("switch was constructed by its real parent group")
            .position;
        let terrain = sim.canonical_terrain();
        let mut pose = position;
        pose.z = terrain.ground_height(pose).saturating_add(512);
        if let Some(ceiling) = terrain.ceiling_height(pose) {
            pose.z = pose.z.min(ceiling.saturating_sub(256));
        }
        sim.set_level2_entity_position(player, pose).unwrap();
    };
    let resume_position = session.state.position;
    place_at(&mut session, 141);
    let mut entered = false;
    let mut wave_group = None;
    for turn in 0..16 {
        visit(
            &mut session,
            "ordinary row141 group7 and row145 objective gate",
            turn,
        );
        if let Some((_, rows)) = groups(&session, 7) {
            assert_eq!(rows.as_ref(), &[142, 143, 145]);
            entered = true;
        }
        wave_group = groups(&session, 9);
        if wave_group.is_some() {
            break;
        }
    }
    assert!(entered);
    let expected_wave: Vec<_> = [119_u16]
        .into_iter()
        .chain(146..=155)
        .chain(168..=171)
        .collect();
    assert_eq!(
        wave_group
            .expect("row145 produces the actual Skeleton wave")
            .1
            .as_ref(),
        expected_wave
    );
    session
        .set_level2_entity_position(player, resume_position)
        .unwrap();

    let switch = |sim: &crate::native_world::Mc2Simulation| {
        *sim.economy()
            .authored_switch_producers()
            .iter()
            .find(|row| row.active && row.authored_index == 119)
            .expect("actual group9 model22 controller")
    };
    let controller = switch(&session);
    assert_eq!(
        (controller.model, controller.action, controller.stage_tag),
        (22, 22, 2)
    );
    let handle = session
        .source_arena()
        .handle(controller.source_entity_index)
        .unwrap();
    let frozen = controller.dword_0x10;
    assert!(
        matches!(frozen, 0 | 16),
        "only its own birth pass can have armed it"
    );
    let wave: Vec<_> = session
        .creature_runtime_census()
        .roots_of_kind(crate::level_definition::CreatureKind::Skeleton)
        .filter_map(|actor| match actor.origin {
            CreatureRootOrigin::AuthoredRecord {
                authored_record: 146..=155,
            } if actor.living => Some(actor.handle),
            _ => None,
        })
        .collect();
    assert_eq!(wave.len(), 10);
    let (&last, others) = wave.split_last().unwrap();
    let hit = |sim: &mut crate::native_world::Mc2Simulation, target: SourceEntityHandle| {
        assert_eq!(sim.source_arena().handle(target.source_slot), Some(target));
        let life = sim.entities()[usize::from(target.source_slot)].life;
        area_damage::contact(
            sim.source_arena_mut(),
            None,
            target.source_slot,
            u16::try_from(life + 1).unwrap(),
            player,
        )
        .unwrap();
    };
    for target in others {
        hit(&mut session, *target);
    }
    for turn in 0..32 {
        if session.source_arena().rebuilt_class5_model_slots()[9] == [last.source_slot] {
            break;
        }
        visit(
            &mut session,
            "live Skeleton membership blocks the countdown",
            turn,
        );
        assert_eq!(switch(&session).dword_0x10, frozen);
        assert!(groups(&session, 2).is_none());
    }
    assert_eq!(
        session.source_arena().rebuilt_class5_model_slots()[9],
        [last.source_slot]
    );
    for turn in 0..17 {
        visit(&mut session, "one surviving authored Skeleton", turn);
        assert_eq!(
            session.source_arena().rebuilt_class5_model_slots()[9],
            [last.source_slot]
        );
        assert_eq!(switch(&session).dword_0x10, frozen);
        assert!(groups(&session, 2).is_none());
    }
    hit(&mut session, last);
    for turn in 0..32 {
        if session.source_arena().rebuilt_class5_model_slots()[9].is_empty() {
            break;
        }
        visit(
            &mut session,
            "final Skeleton fatal pass stays blocked by frozen membership",
            turn,
        );
        assert_eq!(switch(&session).dword_0x10, frozen);
        assert!(groups(&session, 2).is_none());
    }
    assert!(session.source_arena().rebuilt_class5_model_slots()[9].is_empty());

    let mut timer = frozen;
    let clear_visits = if frozen == 0 { 17 } else { 16 };
    for turn in 1..clear_visits {
        visit(&mut session, "ordinary source-empty countdown visit", turn);
        timer = if timer == 0 { 16 } else { timer - 1 };
        assert_eq!(switch(&session).dword_0x10, timer);
        assert!(groups(&session, 2).is_none());
    }
    assert_eq!(timer, 1);
    let cursor = session.effect_journal().cursor(0).next_sequence;
    visit(
        &mut session,
        "consume the actual ordered group2",
        clear_visits,
    );
    let (producer, rows) = groups(&session, 2).expect("real model22 must publish group2 once");
    assert_eq!(producer, controller.source_entity_index);
    let scrolls = [120, 121, 122, 123, 560, 561, 562, 563, 564, 565];
    assert_eq!(
        rows.as_ref(),
        scrolls.into_iter().chain([566, 585]).collect::<Vec<_>>()
    );
    let records = session.effect_journal().records();
    let sound = records
        .iter()
        .find(|record| {
            record.sequence >= cursor
                && matches!(record.effect,
        CampaignEffect::Flight(FlightEvent::PrepareSound { entity_index, sound_id: 41, .. })
            if entity_index == producer)
        })
        .expect("model22 sound41 precedes activation");
    let publication = records.iter().find(|record| record.sequence >= cursor && matches!(record.effect,
        CampaignEffect::Dispatch(Mc2DispatchEvent::AuthoredGroupActivated { group: 2, source_slot, .. })
            if source_slot == producer)).unwrap();
    assert!(sound.sequence < publication.sequence);
    let disabled = switch(&session);
    assert!(disabled.fired && !disabled.drawing_enabled);
    assert_eq!(session.source_arena().handle(producer), Some(handle));
    let actual_scrolls: Vec<_> = session
        .economy()
        .authored_class14()
        .iter()
        .filter(|row| row.active && row.activation_group == 2 && row.model == 5 && row.action == 10)
        .map(|row| row.authored_index)
        .collect();
    assert_eq!(actual_scrolls, scrolls);
    for (index, group) in [(566, 33), (585, 41)] {
        assert!(
            session
                .economy()
                .authored_switch_producers()
                .iter()
                .any(|row| row.active && row.authored_index == index && row.stage_tag == group)
        );
    }
    visit(&mut session, "following source reclaim", 0);
    assert!(session.source_arena().resolve(handle).is_none());
    assert!(groups(&session, 2).is_none());

    place_at(&mut session, 585);
    for turn in 0..8 {
        visit(&mut session, "real group2 corridor-control successor", turn);
        if let Some((_, rows)) = groups(&session, 41) {
            assert_eq!(rows.as_ref(), &[586]);
            let command = *session
                .economy()
                .terrain_strip_close_triggers()
                .iter()
                .find(|row| row.authored_index == 586)
                .expect("ordinary Cave corridor close command");
            assert_eq!((command.class, command.model, command.action), (10, 63, 68));
            let visited = |sim: &crate::native_world::Mc2Simulation| {
                sim.cf10_production_schedule_receipts().iter().any(|receipt| matches!(receipt,
                    ScheduleReceipt::CallbackCompleted { slot, .. } if *slot == command.source_entity_index))
            };
            if !visited(&session) {
                visit(
                    &mut session,
                    "scheduled Cave corridor close command",
                    turn + 1,
                );
                assert!(visited(&session));
            }
            return;
        }
    }
    panic!("actual switch585 did not publish its class10/63 corridor command");
}
