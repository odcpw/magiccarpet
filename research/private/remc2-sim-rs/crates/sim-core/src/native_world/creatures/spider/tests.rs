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

#[test]
fn mission_four_actual_spider_web_contact_recovery_and_paid_meteor_cleanup() {
    use crate::campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign};
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::game_session::{GameCommand, GameSession, SpellAssignment, SpellAssignmentHand};
    use crate::level_definition::{CreatureKind, METEOR_PROPERTIES};
    use crate::native_world::{Level2EconomyEvent, Mc2DispatchEvent, area_damage, lightning};
    use crate::{FlightWorld, PlayerCommand};

    const GROUP6: &[u16] = &[
        8, 9, 10, 11, 12, 137, 138, 139, 141, 250, 251, 255, 601, 602, 724, 725, 726,
    ];
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
    let switch = simulation(&game).source_arena().handle(6).unwrap();
    let switch_row = &simulation(&game).entities()[6];
    assert_eq!(
        (
            switch_row.class,
            switch_row.model,
            switch_row.action,
            switch_row.source_id
        ),
        (11, 32, 32, 6)
    );
    assert!(
        !simulation(&game)
            .creature_runtime_census()
            .roots_of_kind(CreatureKind::Spider)
            .any(|row| row.living)
    );
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
        assert!(simulation(&game).source_arena().resolve(switch).is_some());
    }
    assert_eq!(
        simulation(&game).session.state.live.player_protection_ticks,
        0
    );

    // Declared prior carry/progression, player resources and a real constructed
    // Castle are prerequisites for the paid public tierII activation below.
    // These controls do not assert earned mission progression.
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
            .find(|position| {
                !sim.economy().castle_projectile_site_blocked(
                    *position,
                    sim.entities(),
                    sim.canonical_terrain(),
                )
            })
            .expect("source-valid Castle site");
        let castle = sim.economy_mut().build_castle(site, player).unwrap();
        assert!(
            sim.economy_mut()
                .set_castle_mana_prerequisite_for_test(castle, 200_000)
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
        let color = live.core.state.player_color_index;
        live.core
            .runtime
            .set_level2_objective_stage_selector_for(usize::from(color), 0, 2);
    }
    let mut published = false;
    // This is the actual source model32 gate. No group is activated directly.
    for turn in 0..8 {
        assert_eq!(
            simulation(&game).source_arena().handle(switch.source_slot),
            Some(switch),
            "activation must retain the original reset6 generation"
        );
        let command = if turn == 0 {
            GameCommand {
                assign_spell: Some(SpellAssignment {
                    spell_id: crate::game_definition::SemanticId::parse("spell.meteor").unwrap(),
                    tier: 1,
                    hand: SpellAssignmentHand::Left,
                    holder_actor: None,
                }),
                ..Default::default()
            }
        } else {
            GameCommand::default()
        };
        public_step(&mut game, command, "actual group6 switch", turn);
        if simulation(&game).dispatch_events().iter().any(|event|
            matches!(event, Mc2DispatchEvent::AuthoredGroupActivated { source_slot: 6, group: 6, authored_indices, .. }
                if authored_indices.as_ref() == GROUP6)) { published = true; break; }
    }
    assert!(
        published,
        "actual reset6 must publish the entire17-member group6"
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
    // Its real holder is materialised after the wave so the already-pinned
    // Spider IDs and seeded headings remain the same source inputs as C3.
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
    area_damage::start_effect_damage_trace(root.source_slot);
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
                && visit.mailbox == (780, original.source_id)
                && visit.life_after == visit.life_before.wrapping_sub(780)
                && visit.writer.is_some_and(|writer| {
                    writer.target == player_handle
                        && writer.mailbox == visit.mailbox
                        && traced_impacts.iter().any(|impact| {
                            writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
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
            position.z = row.position.z.wrapping_add(256);
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
    let provocation_effects = area_damage::take_effect_damage_trace();
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
    let (web_launches, web_impacts) = web::take_trace();
    let web_hit = player_consumptions.iter().any(|visit| {
        visit.target == player_handle
            && visit.mailbox == (780, original.source_id)
            && visit.life_after == visit.life_before.wrapping_sub(780)
            && visit.writer.is_some_and(|writer| {
                writer.target == player_handle
                    && writer.mailbox == visit.mailbox
                    && web_impacts.iter().any(|impact| {
                        writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
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

    // One declared player-only fixed firing placement.1024 behind the current
    // actor leaves the256 hand offset within the source113 acquisition cone.
    // Aim is computed once; no subsequent pose/life read changes the command.
    // TierII supplies five1600 pulses (four suffice for the actual5300 life)
    // with stationary half-widths0/384/768/1152/1536. C7's tierIII also killed
    // original138; the smaller tierII area must still pass the unchanged
    // five-survivor and +1 credit checks below.
    let target = simulation(&game).entities()[usize::from(root.source_slot)].clone();
    assert_eq!(
        (target.life, target.max_life),
        (5300, 5500),
        "only the actual200 provocation changed life"
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
}

#[test]
fn configured_mission_four_all_six_spider_deaths_drive_scheduled_absence_and_reclaim() {
    use crate::PlayerCommand;
    use crate::campaign::CampaignSession;
    use crate::campaign::source_arena::CreatureRootOrigin;
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{Mc2DispatchEvent, area_damage};

    const GROUP6: &[u16] = &[
        8, 9, 10, 11, 12, 137, 138, 139, 141, 250, 251, 255, 601, 602, 724, 725, 726,
    ];
    let campaign = CampaignSession::open(
        mission_four_root(),
        crate::campaign::spec::CampaignSelection::ordinary(4).unwrap(),
    )
    .unwrap();
    let mut session = campaign.simulation().clone();
    let stage = 1;
    assert_eq!(
        campaign.definition().level_definition().objectives()[stage].condition(),
        ObjectiveCondition::NoLivingCreatures(CreatureKind::Spider)
    );
    assert_eq!(session.runtime().stages[stage].kind, 7);
    let owner = usize::from(session.state.player_color_index);
    let player = session.state.entity_index;
    let switch = session.source_arena().handle(6).unwrap();
    // Only prior objective progress is declared here. Original placement,
    // six constructors, source phases, RNG and lifetimes are retained.
    session
        .session
        .state
        .live
        .core
        .runtime
        .set_level2_objective_stage_selector_for(owner, 0, 2);
    let mut published = false;
    for turn in 0..8 {
        assert_eq!(session.source_arena().handle(6), Some(switch));
        session.step_outer_turn(PlayerCommand::default(), 0);
        no_dispatch_errors(&session, "configured absence real producer", turn);
        if session.dispatch_events().iter().any(|event| {
            matches!(event, Mc2DispatchEvent::AuthoredGroupActivated {
                source_slot: 6, group: 6, authored_indices, ..
            } if authored_indices.as_ref() == GROUP6)
        }) {
            published = true;
            break;
        }
    }
    assert!(published);
    // The declared prior selector0=2 bypasses its real completion transition,
    // which alone would advance ObjectiveText. Bind the package's existing
    // task1 as current only AFTER all six actual roots have been published.
    session
        .session
        .state
        .live
        .core
        .runtime
        .set_level2_objective_text_selector_for(owner, stage);
    // Consume any real objective-done pause and require a living blocked pass.
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
}
