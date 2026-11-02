use super::*;

#[test]
fn alliance_follows_retained_parent_and_expires_when_dead() {
    let (mut arena, mut runtime, mut terrain, particles, player, feature) = mission_four();
    let position = feature.world_position(terrain.ground_height(feature.world_position(0)));
    let slot = mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain, &particles,
        position, RULES.max_life, None, feature.index).unwrap().unwrap();
    let terminal = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
    let effect = &mut arena.entities[usize::from(terminal)];
    effect.source_id = player;
    effect.record[74] = 1;
    effect.subspell_index = 40;
    effect.record[154..156].copy_from_slice(&slot.to_le_bytes());
    let impact = crate::native_world::alliance_spell::step_impact(&mut arena, terminal, &[slot]).unwrap();
    assert_eq!(impact.affected_entities, [slot]);
    let hostile = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    let mut enemy = crate::world::terrain::cave_features::new_event(hostile, 17, Position {
        x: position.x.wrapping_add(512), ..position
    });
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
    let families = std::array::from_fn(|model| if model == usize::from(SOURCE_MODEL) { vec![slot] } else { Vec::new() });
    for life in [100, 0] {
        let mut trial = arena.clone();
        let mut trial_runtime = runtime.clone();
        let mut trial_terrain = terrain.clone();
        trial.entities[usize::from(player)].set_life(life);
        let effects = mc2::step_row(&mut trial, &mut trial_runtime, &mut trial_terrain, &particles,
            slot, &[], &families, None).unwrap();
        let actor = &trial.entities[usize::from(slot)];
        assert_eq!(actor.record[77], if life > 0 { 14 } else { 10 });
        assert_eq!(u16::from_le_bytes([actor.record[44], actor.record[45]]),
            if life > 0 { player } else { 0 });
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
    mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player],
        &families,
        Some((player, 0)),
    )
    .unwrap();
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
    let (_, _, terrain, _, player, _) = mission_four();
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
    actor.action = 162;
    actor.target = player;
    actor.phase = 1; // No ranged callback at this cadence: status still advances phase0.
    actor.control_flags |= 0x800;
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
        local_player_status: Some((player, 1)),
    };
    let effects = step(&mut actor, &mut context).unwrap();
    assert!(effects.web_target.is_none());
    assert_eq!(actor.attack_phase, 1);
    actor.control_flags |= 0x800;
    actor.phase = 35;
    let effects = step(&mut actor, &mut context).unwrap();
    assert_eq!(effects.melee, Some((player, 100, 701)));
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
    actor.yaw = heading_for_positions(peer.position, actor.position) as i16;
    actor.control_flags |= 0x800;
    actor.proximity = 0;
    let mut context = SpiderContext {
        terrain: &terrain,
        targets: &[peer],
        class3: &[],
        model20: &[702],
        class5: &[702],
        stage_runtime: None,
        local_player_status: None,
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
        let result = mc2::step_row(
            &mut arena,
            &mut runtime,
            &mut terrain,
            &particles,
            slot,
            &[player],
            &families,
            Some((player, 0)),
        )
        .unwrap();
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
    let result = mc2::step_row(
        &mut arena,
        &mut runtime,
        &mut terrain,
        &particles,
        slot,
        &[player],
        &families,
        Some((player, 0)),
    )
    .unwrap();
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
