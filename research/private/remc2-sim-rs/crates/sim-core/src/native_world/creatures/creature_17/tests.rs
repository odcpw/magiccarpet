use super::*;
use crate::campaign::source_arena::SourceArena;

fn put(row: &mut crate::native_world::EntitySnapshot, offset: usize, value: u16) {
    row.record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn add_creature(
    arena: &mut SourceArena,
    terrain: &mut SessionTerrain,
    particles: &[crate::native_world::ParticleParameter],
    position: Position,
    action: u8,
) -> u16 {
    let slot = arena
        .allocate_source_slot_with_terrain(terrain)
        .expect("fixture has a source slot");
    let seed = arena.new_event_rng(slot);
    let mut counter = 0;
    let mut row = mc2::construct_row(
        slot,
        arena.slot_generation(slot),
        seed,
        position,
        &mut counter,
        particles,
    )
    .expect("model17 constructor assets are retained");
    row.source_id = slot;
    put(&mut row, 30, slot);
    row.set_source_action(action);
    arena.economy.source_rng.insert(slot, row.entity_rng);
    arena.entities[usize::from(slot)] = row;
    crate::world::effects::sync_position(arena, terrain, slot, position, true, false)
        .expect("publish source row position");
    slot
}

fn class3_slots(arena: &SourceArena) -> Vec<u16> {
    arena
        .entities
        .iter()
        .filter(|row| row.active && row.class == 3 && row.life >= 0)
        .map(|row| row.index)
        .collect()
}

fn model_slots(arena: &SourceArena) -> [Vec<u16>; 29] {
    std::array::from_fn(|model| {
        arena
            .entities
            .iter()
            .filter(|row| row.active && row.class == 5 && usize::from(row.model) == model)
            .map(|row| row.index)
            .collect()
    })
}

#[test]
fn constructor_binds_source17_properties_and_particle285() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = &simulation.session.definition.presentation.particles;
    let mut counter = 17;
    let row = mc2::construct_row(
        41,
        9,
        0x1234,
        Position {
            x: 20_000,
            y: 20_000,
            z: 256,
        },
        &mut counter,
        particles,
    )
    .expect("model17 binds retained source assets");

    assert_eq!((row.class, row.model, row.action), (5, SOURCE_MODEL, 137));
    assert_eq!(
        (row.max_life, row.life, row.mana),
        (RULES.max_life, RULES.max_life, RULES.initial_mana)
    );
    assert_eq!(
        (
            row.actual_speed,
            row.minimum_speed(),
            u16::from_le_bytes([row.record[138], row.record[139]]) as i16,
        ),
        (
            RULES.minimum_speed,
            RULES.minimum_speed,
            RULES.maximum_speed,
        )
    );
    assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
    assert_eq!(row.particle_parameter_index, Some(285));
    assert_eq!(counter, 18);
}

#[test]
fn unsupported_action142_does_not_claim_a_callback() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let runtime = simulation.session.state.live.core.runtime.clone();
    let mut counter = 0;
    let mut actor = construct(
        41,
        9,
        0x1234,
        Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        },
        &mut counter,
    );
    actor.action = 142;
    let context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &[],
        class3: &[],
        model17: &[],
    };

    assert!(step(&mut actor, &context).is_none());
}

#[test]
fn ordinary_acquisition_uses_frozen_class3_order_and_ignores_nearer_scenery() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let runtime = simulation.session.state.live.core.runtime.clone();
    let actor_position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let scenery = Target {
        slot: 9,
        source_id: 9,
        class: 4,
        model: 0,
        life: 1000,
        position: Position {
            y: actor_position.y.wrapping_sub(100),
            ..actor_position
        },
        ..Target::default()
    };
    let first_player = Target {
        slot: 10,
        source_id: 10,
        class: 3,
        model: 0,
        life: 1000,
        position: Position {
            y: actor_position.y.wrapping_sub(300),
            ..actor_position
        },
        ..Target::default()
    };
    let tied_player = Target {
        slot: 11,
        source_id: 11,
        ..first_player
    };
    let targets = [scenery, first_player, tied_player];
    let classes = [10, 11];
    let families = [];
    let mut counter = 0;
    let mut actor = construct(41, 9, 0x1234, actor_position, &mut counter);
    actor.action = 137;
    actor.phase = RULES.source_scan_period;
    actor.yaw = 0;
    actor.roll = 0;
    actor.proximity = 64;
    actor.control_flags |= 0x800;
    let context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &targets,
        class3: &classes,
        model17: &families,
    };

    step(&mut actor, &context).expect("ordinary acquisition callback is supported");

    assert_eq!(actor.action, 138);
    assert_eq!(
        actor.target, 10,
        "nearest scenery is ignored and class3 ties keep list order"
    );

    let mut scenery_only = construct(41, 9, 0x1234, actor_position, &mut counter);
    scenery_only.action = 137;
    scenery_only.phase = RULES.source_scan_period;
    scenery_only.yaw = 0;
    scenery_only.roll = 0;
    scenery_only.proximity = 64;
    scenery_only.control_flags |= 0x800;
    let scenery_context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &targets[..1],
        class3: &[],
        model17: &families,
    };
    step(&mut scenery_only, &scenery_context).expect("scenery-only callback is supported");
    assert_eq!(scenery_only.action, 137);
    assert_eq!(scenery_only.target, 0);
}

#[test]
fn ordinary_136_and_137_wrappers_reset_attack_phase_and_reject_nonplayer_targets() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let runtime = simulation.session.state.live.core.runtime.clone();
    let actor_position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let attacker = Target {
        slot: 9,
        source_id: 9,
        class: 4,
        life: 1000,
        position: Position {
            y: actor_position.y.wrapping_sub(100),
            ..actor_position
        },
        ..Target::default()
    };
    let nonplayer_class3 = Target {
        slot: 10,
        source_id: 10,
        class: 3,
        model: 2,
        life: 1000,
        position: Position {
            y: actor_position.y.wrapping_sub(200),
            ..actor_position
        },
        ..Target::default()
    };

    let mut counter = 0;
    let mut stationary = construct(41, 9, 0x1234, actor_position, &mut counter);
    stationary.action = 136;
    stationary.attack_phase = 5;
    stationary.pending_damage = 1;
    stationary.pending_attacker = attacker.slot;
    let stationary_targets = [attacker];
    let stationary_context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &stationary_targets,
        class3: &[],
        model17: &[],
    };
    step(&mut stationary, &stationary_context).expect("stationary callback is supported");
    assert_eq!(stationary.action, 138);
    assert_eq!(stationary.target, 0);
    assert_eq!(stationary.attack_phase, 0);

    let mut roaming = construct(41, 9, 0x1234, actor_position, &mut counter);
    roaming.action = 137;
    roaming.phase = RULES.source_scan_period;
    roaming.attack_phase = 6;
    roaming.yaw = 0;
    roaming.roll = 0;
    roaming.proximity = 64;
    roaming.control_flags |= 0x800;
    let roaming_targets = [nonplayer_class3];
    let classes = [nonplayer_class3.slot];
    let roaming_context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &roaming_targets,
        class3: &classes,
        model17: &[],
    };
    step(&mut roaming, &roaming_context).expect("roaming callback is supported");
    assert_eq!(roaming.action, 138);
    assert_eq!(roaming.target, 0);
    assert_eq!(roaming.attack_phase, 0);
}

#[test]
fn peer_cones_differ_for_stationary_and_roaming_and_zero_proximity_skips_both_scans() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let runtime = simulation.session.state.live.core.runtime.clone();
    let actor_position = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let mut counter = 0;
    let mut stationary = construct(41, 9, 0x1234, actor_position, &mut counter);
    stationary.action = 136;
    stationary.phase = RULES.source_scan_period;
    stationary.yaw = 0;
    let peer = Target {
        slot: 42,
        source_id: stationary.source_id,
        class: 5,
        model: SOURCE_MODEL,
        life: 1000,
        position: Position {
            y: actor_position.y.wrapping_sub(200),
            ..actor_position
        },
        ..Target::default()
    };
    let targets = [peer];
    let peers = [peer.slot];
    let context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &targets,
        class3: &[],
        model17: &peers,
    };

    step(&mut stationary, &context).expect("stationary peer callback is supported");
    assert_eq!(
        stationary.action, 136,
        "stationary scans face peer-to-actor"
    );
    assert_eq!(stationary.linked_peer, 0);

    let mut roaming = construct(41, 9, 0x1234, actor_position, &mut counter);
    roaming.action = 137;
    roaming.phase = RULES.source_scan_period;
    roaming.yaw = 0;
    roaming.roll = 0;
    roaming.proximity = 64;
    roaming.control_flags |= 0x800;
    step(&mut roaming, &context).expect("roaming peer callback is supported");
    assert_eq!(roaming.action, 139, "roaming scans face actor-to-peer");
    assert_eq!(roaming.linked_peer, peer.slot);

    let mut no_proximity = construct(41, 9, 0x1234, actor_position, &mut counter);
    no_proximity.action = 137;
    no_proximity.phase = RULES.source_scan_period;
    no_proximity.yaw = 0;
    no_proximity.roll = 0;
    no_proximity.proximity = 0;
    no_proximity.control_flags |= 0x800;
    step(&mut no_proximity, &context).expect("zero-proximity callback is supported");
    assert_eq!(no_proximity.action, 137);
    assert_eq!(no_proximity.linked_peer, 0);
}

#[test]
fn attack_creates_guided_model20_slows_target_then_reclaims_projectile_next_pass() {
    let package = std::env::var_os("MC_CAMPAIGN_PACKAGE").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-cf02-campaign-package-v2-b/qualified-level2")
        },
        std::path::PathBuf::from,
    );
    let cache = std::env::var_os("MC_LEVEL2_GAMEPLAY_CACHE_V5").map_or_else(
        || {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/level2-day-owner-cache/components/gameplay")
        },
        std::path::PathBuf::from,
    );
    let mut simulation = crate::campaign::CampaignSession::open_with_owner_cache(
        package,
        cache,
        crate::campaign::spec::CampaignSelection::LEVEL2,
    )
    .expect("qualified Level 2 campaign and matching owner cache")
    .simulation()
    .clone();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let target_slot = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0 && row.life >= 0)
        .expect("fixture retains a live player")
        .index;
    let target_position = live.economy.entities[usize::from(target_slot)].position;
    let (actor_x, actor_yaw) = if target_position.x >= 2_000 {
        (target_position.x - 2_000, 0)
    } else {
        (target_position.x + 2_000, 1_024)
    };
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let actor_slot = add_creature(
        arena,
        terrain,
        &particles,
        Position {
            x: actor_x,
            y: target_position.y,
            z: target_position.z,
        },
        138,
    );
    {
        let actor = &mut arena.entities[usize::from(actor_slot)];
        actor.set_yaw(actor_yaw);
        actor.set_source_roll(actor_yaw);
        actor.set_phase(RULES.source_scan_period);
        actor.set_control_flags(actor.control_flags | 0x800);
        put(actor, 154, target_slot);
    }
    let live = &mut simulation.session.state.live;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let classes = class3_slots(arena);
    let families = model_slots(arena);
    let effects = mc2::step_row(
        arena, runtime, terrain, &particles, actor_slot, &classes, &families,
    )
    .expect("model17 attack callback succeeds");
    assert_eq!(effects.sound, Some(58));
    let projectile_slot = effects
        .projectile_slot
        .expect("source-range attack materializes its projectile");
    let projectile = &arena.entities[usize::from(projectile_slot)];
    assert_eq!(
        (projectile.class, projectile.model, projectile.action),
        (9, 20, 21)
    );
    assert_eq!(projectile.particle_parameter_index, Some(196));
    assert_eq!(projectile.parameter_relation, Some(65));
    assert_eq!(put_word(projectile, 154), target_slot);
    assert_eq!(projectile.subspell_index, RULES.projectile_damage as u16);

    // Put the actual launched row half a tick before the target, in the same
    // source map cell, so the real guided callback exercises collision.
    let target_position = arena.entities[usize::from(target_slot)].position;
    let target_x_in_cell = target_position.x & 0xff;
    let start_x = if target_x_in_cell >= 106 {
        target_position.x.wrapping_sub(500)
    } else {
        target_position.x.wrapping_add(500)
    };
    let mut start = target_position;
    start.x = start_x;
    let yaw = heading_for_positions(start, target_position) as i16;
    {
        let projectile = &mut arena.entities[usize::from(projectile_slot)];
        projectile.set_position(start);
        projectile.set_yaw(yaw);
        projectile.set_pitch(0);
    }
    crate::world::effects::sync_position(arena, terrain, projectile_slot, start, true, false)
        .expect("publish guided projectile position");
    let impact = mc2::step_projectile(
        arena,
        terrain,
        &particles,
        projectile_slot,
        crate::native_world::source_target::Context::default(),
    )
    .expect("guided model20 callback advances")
    .impact_slot
    .expect("collision constructs the source Slowing impact");
    assert_eq!(
        (
            arena.entities[usize::from(impact)].class,
            arena.entities[usize::from(impact)].model,
            arena.entities[usize::from(impact)].action,
        ),
        (10, 65, 70)
    );
    assert_eq!(
        put_word(&arena.entities[usize::from(impact)], 154),
        target_slot
    );
    assert!(arena.entities[usize::from(projectile_slot)].drawing_disabled());

    let player = arena.entities[usize::from(target_slot)].clone();
    let mut player_state = crate::FlightState {
        entity_index: target_slot,
        position: player.position,
        ..crate::FlightState::default()
    };
    let definition = sim_definition::adapter::mc2_field_definition(
        crate::level_definition::FieldKind::Slowing,
        crate::level_definition::TilePosition { x: 0, y: 0 },
    );
    let local_color = player_state.player_color_index;
    crate::world::fields::status::impact(
        &mut arena.entities[usize::from(impact)],
        definition,
        Some((&player, &mut player_state)),
        local_color,
    )
    .expect("typed Slowing consumer applies the actual impact");
    assert_eq!(player_state.speed_modifier, 1);

    arena
        .rebuild_source_arena_with_terrain(terrain, &[])
        .expect("following source boundary reclaims the hidden projectile");
    assert!(!arena.entities[usize::from(projectile_slot)].active);
    assert!(!arena.entities[usize::from(impact)].active);

    // Level 17 contains actual reset-imported model17 actors and their package
    // semantic bindings. Position and aim are controlled for this collision
    // check; the actor, attack, scheduler and player state are real carriers.
    let level17 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level17");
    let mut scheduled = crate::campaign::CampaignSession::open(
        level17,
        crate::campaign::spec::CampaignSelection::ordinary(17).unwrap(),
    )
    .expect("retained Level 17 campaign")
    .simulation()
    .clone();
    let (actor_slot, actor_source_id, target_slot) = {
        let live = &mut scheduled.session.state.live;
        let target_slot = live.core.state.entity_index;
        let target_position = live.core.state.position;
        let (actor_x, actor_yaw) = if target_position.x >= 2_000 {
            (target_position.x - 2_000, 0)
        } else {
            (target_position.x + 2_000, 1_024)
        };
        let actor = live
            .economy
            .entities
            .iter_mut()
            .find(|row| row.active && (row.class, row.model) == (5, SOURCE_MODEL))
            .expect("retained Level 17 model17 actor");
        let position = Position {
            x: actor_x,
            y: target_position.y,
            z: target_position.z,
        };
        actor.set_position(position);
        actor.set_source_action(138);
        actor.set_yaw(actor_yaw);
        actor.set_source_roll(actor_yaw);
        actor.set_phase(RULES.source_scan_period);
        actor.set_control_flags(actor.control_flags | 0x800);
        put(actor, 154, target_slot);
        let (slot, source_id) = (actor.index, actor.source_id);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            slot,
            position,
            true,
            false,
        )
        .expect("publish the retained model17 position");
        (slot, source_id, target_slot)
    };
    scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !scheduled
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "model17 scheduled attack failed: {:?}",
        scheduled.dispatch_events()
    );
    let projectile_handle = {
        let live = &mut scheduled.session.state.live;
        let projectile = live
            .economy
            .entities
            .iter()
            .find(|row| {
                row.active
                    && (row.class, row.model, row.action) == (9, 20, 21)
                    && row.source_id == actor_source_id
            })
            .expect("scheduled model17 attack creates the actual guided projectile")
            .index;
        let target = live.core.state.position;
        let mut start = target;
        start.x = if target.x & 0xff >= 106 {
            target.x.wrapping_sub(500)
        } else {
            target.x.wrapping_add(500)
        };
        let yaw = heading_for_positions(start, target) as i16;
        let row = &mut live.economy.entities[usize::from(projectile)];
        row.set_position(start);
        row.set_yaw(yaw);
        row.set_pitch(0);
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            projectile,
            start,
            true,
            false,
        )
        .expect("publish the actual projectile map position");
        live.economy.entities[usize::from(actor_slot)].set_source_action(136);
        (projectile, live.economy.economy.slot_generation(projectile))
    };
    let mut impact_slot = None;
    for _ in 0..4 {
        scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !scheduled
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "model17 scheduled impact failed: {:?}",
            scheduled.dispatch_events()
        );
        let live = &scheduled.session.state.live;
        if let Some(row) = live.economy.entities.iter().find(|row| {
            row.active
                && (row.class, row.model, row.action) == (10, 65, 70)
                && put_word(row, 154) == target_slot
        }) {
            impact_slot = Some((row.index, live.economy.economy.slot_generation(row.index)));
        }
        if live.core.state.speed_modifier != 0 {
            break;
        }
    }
    assert!(
        impact_slot.is_some(),
        "ordinary collision produces the scheduled status65 row"
    );
    assert_eq!(scheduled.session.state.live.core.state.speed_modifier, 1);
    let impact_slot = impact_slot.unwrap();
    for _ in 0..12 {
        scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !scheduled
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. }))
        );
    }
    let live = &scheduled.session.state.live;
    assert_eq!(
        (
            live.core.state.speed_modifier,
            live.core.state.speed_modifier_counter
        ),
        (0, 0)
    );
    for (slot, generation) in [projectile_handle, impact_slot] {
        let row = &live.economy.entities[usize::from(slot)];
        assert!(
            !row.active || live.economy.economy.slot_generation(slot) != generation,
            "source projectile and impact are reclaimed or their addresses reused"
        );
    }
}

#[test]
fn prekill_inherits_owned_child_credit_without_running_death_early() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation
        .session
        .definition
        .presentation
        .particles
        .to_vec();
    let live = &mut simulation.session.state.live;
    let player_slot = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && row.class == 3 && row.model == 0)
        .expect("fixture retains the source player")
        .index;
    let (arena, runtime, terrain) = (&mut live.economy, &mut live.core.runtime, &mut live.terrain);
    let root = add_creature(
        arena,
        terrain,
        &particles,
        Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        },
        140,
    );
    let child = add_creature(
        arena,
        terrain,
        &particles,
        Position {
            x: 20_128,
            y: 20_000,
            z: 0,
        },
        136,
    );
    put(&mut arena.entities[usize::from(root)], 56, child);
    put(&mut arena.entities[usize::from(child)], 40, player_slot);
    let classes = class3_slots(arena);
    let families = model_slots(arena);

    let effects = mc2::step_row(
        arena, runtime, terrain, &particles, root, &classes, &families,
    )
    .expect("PreKill callback succeeds");

    assert_eq!(effects.kill_credit, Some(player_slot));
    assert_eq!(arena.entities[usize::from(root)].action, 141);
    assert_eq!(arena.entities[usize::from(child)].action, 141);
    assert_eq!(
        put_word(&arena.entities[usize::from(root)], 40),
        player_slot
    );
    assert!(arena.entities[usize::from(root)].active);
    assert!(
        !effects.kill,
        "PreKill does not run the phase-gated death callback"
    );

    // The retained Level 17 reset contains actual model17 roots. Drive a
    // nonlethal and then lethal contact through their normal scheduler and
    // inspect the same class-5 list used by absence objectives.
    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level17");
    let mut scheduled = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(17).unwrap(),
    )
    .expect("retained Level 17 campaign")
    .simulation()
    .clone();
    let (victim, generation, player, initial_life) = {
        let live = &mut scheduled.session.state.live;
        let slots = live.economy.rebuilt_class5_model_slots();
        assert!(
            slots[17].len() > 1,
            "real package retains other model17 roots"
        );
        let victim = slots[17][0];
        let player = live.core.state.entity_index;
        let row = &mut live.economy.entities[usize::from(victim)];
        let initial_life = row.life;
        row.set_source_action(136);
        row.set_phase(0);
        let generation = live.economy.economy.slot_generation(victim);
        crate::native_world::area_damage::contact(&mut live.economy, None, victim, 1, player)
            .expect("real model17 damage mailbox");
        (victim, generation, player, initial_life)
    };
    scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !scheduled
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "model17 nonlethal source pass must not roll back: {:?}",
        scheduled.dispatch_events()
    );
    {
        let live = &mut scheduled.session.state.live;
        let row = &live.economy.entities[usize::from(victim)];
        assert_eq!(row.life, initial_life - 1);
        assert!(live.economy.rebuilt_class5_model_slots()[17].contains(&victim));
        let lethal = u16::try_from(row.life + 1).expect("retained model17 life fits damage word");
        crate::native_world::area_damage::contact(&mut live.economy, None, victim, lethal, player)
            .expect("real model17 lethal mailbox");
    }
    scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !scheduled
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "model17 lethal source pass must not roll back: {:?}",
        scheduled.dispatch_events()
    );
    {
        let live = &scheduled.session.state.live;
        let row = &live.economy.entities[usize::from(victim)];
        assert!(row.active && row.life < 0 && row.action == 140);
        let frozen = live.economy.rebuilt_class5_model_slots();
        assert!(
            !frozen[17].contains(&victim),
            "negative-life root leaves the next frozen list"
        );
        assert!(
            !frozen[17].is_empty(),
            "other living roots still prevent family absence"
        );
    }
    let mut saw_hidden = false;
    for _ in 0..16 {
        scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            !scheduled
                .dispatch_events()
                .iter()
                .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "model17 death lifecycle must not roll back: {:?}",
            scheduled.dispatch_events()
        );
        let live = &scheduled.session.state.live;
        let row = &live.economy.entities[usize::from(victim)];
        if live.economy.economy.slot_generation(victim) != generation || !row.active {
            assert!(
                saw_hidden,
                "death remains hidden until the following source boundary"
            );
            break;
        }
        saw_hidden |= row.drawing_disabled();
    }
    assert!(
        saw_hidden,
        "phase-gated model17 death hides the actual source actor"
    );
    let live = &scheduled.session.state.live;
    assert!(
        !live.economy.entities[usize::from(victim)].active
            || live.economy.economy.slot_generation(victim) != generation,
        "the next source boundary reclaims the hidden actor"
    );

    // Keep one real package root alive as the absence negative, then remove
    // the last root through the same source contact and scheduled callback.
    // This controls a bounded family lifecycle, not an earned mission result.
    let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/mc-bhq6-ordinary-20261024-b/level17");
    let mut absence = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(17).unwrap(),
    )
    .expect("retained Level 17 campaign")
    .simulation()
    .clone();
    let survivor = {
        let live = &mut absence.session.state.live;
        let victims = live.economy.rebuilt_class5_model_slots()[17].clone();
        assert!(victims.len() > 1);
        let survivor = *victims.last().unwrap();
        let player = live.core.state.entity_index;
        for slot in victims.into_iter().filter(|slot| *slot != survivor) {
            let row = &mut live.economy.entities[usize::from(slot)];
            row.set_source_action(136);
            let lethal = u16::try_from(row.life + 1).unwrap();
            crate::native_world::area_damage::contact(
                &mut live.economy,
                None,
                slot,
                lethal,
                player,
            )
            .expect("real model17 lethal contact");
        }
        survivor
    };
    absence.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !absence
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "model17 family damage must not roll back: {:?}",
        absence.dispatch_events()
    );
    {
        let live = &mut absence.session.state.live;
        let remaining = &live.economy.rebuilt_class5_model_slots()[17];
        assert_eq!(
            remaining.as_slice(),
            &[survivor],
            "one living source root prevents absence"
        );
        let player = live.core.state.entity_index;
        let row = &mut live.economy.entities[usize::from(survivor)];
        row.set_source_action(136);
        let lethal = u16::try_from(row.life + 1).unwrap();
        crate::native_world::area_damage::contact(
            &mut live.economy,
            None,
            survivor,
            lethal,
            player,
        )
        .expect("last model17 lethal contact");
    }
    absence.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert!(
        !absence
            .dispatch_events()
            .iter()
            .any(|event| matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
        "last model17 source pass must not roll back: {:?}",
        absence.dispatch_events()
    );
    assert!(
        absence
            .session
            .state
            .live
            .economy
            .rebuilt_class5_model_slots()[17]
            .is_empty(),
        "the following source-frozen family list admits absence"
    );
}

#[test]
fn action138_emits_sound_before_lethal_damage_redirects_to_prekill() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.session.state.live.terrain.clone();
    let runtime = simulation.session.state.live.core.runtime.clone();
    let mut counter = 0;
    let mut actor = construct(
        41,
        9,
        0x1234,
        Position {
            x: 20_000,
            y: 20_000,
            z: 0,
        },
        &mut counter,
    );
    actor.action = 138;
    actor.life = 1;
    actor.pending_damage = 2;
    actor.pending_attacker = 7;
    let context = Creature17Context {
        terrain: &terrain,
        runtime: &runtime,
        targets: &[],
        class3: &[],
        model17: &[],
    };

    let effects = step(&mut actor, &context).expect("action138 is supported");

    assert_eq!(effects.sound, Some(58));
    assert_eq!(actor.action, 140, "lethal damage redirects to PreKill");
}

fn put_word(row: &crate::native_world::EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}
