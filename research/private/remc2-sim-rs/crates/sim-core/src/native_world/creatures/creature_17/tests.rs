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
fn alliance_impact_drives_model17_follow_selected_attack_and_expiry() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let mut runtime = session.session.state.live.core.runtime.clone();
    let particles = &session.session.definition.presentation.particles;
    let parent = session.state.entity_index;
    let position = session.state.position;
    let actor = add_creature(&mut arena, &mut terrain, particles, position, 137);
    arena.entities[usize::from(actor)].record[76] = 3;
    let enemy = arena.allocate_source_slot_with_terrain(&mut terrain).unwrap();
    let mut target = crate::native_world::cave_features::new_event(enemy, 5, position);
    target.class = 5; target.model = 21; target.record[67] = 5; target.record[68] = 21;
    target.set_life(1_000);
    arena.entities[usize::from(enemy)] = target;
    put(&mut arena.entities[usize::from(parent)], 154, enemy);
    let impact = crate::native_world::alliance_spell::construct_impact(&mut arena, &mut terrain).unwrap();
    let effect = &mut arena.entities[usize::from(impact)];
    effect.source_id = parent; put(effect, 30, parent); effect.record[74] = 32;
    effect.subspell_index = 40; put(effect, 154, actor);
    assert_eq!(crate::native_world::alliance_spell::step_impact(&mut arena, impact, &[actor]).unwrap().affected_entities, vec![actor]);
    assert_eq!((arena.entities[usize::from(actor)].action, arena.entities[usize::from(actor)].record[77]), (143, 14));
    let families = model_slots(&arena);
    let outcome = mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor, &[parent], &families, Default::default()).unwrap();
    assert_eq!(outcome.feedback, vec![(parent, 24)]);
    let row = &arena.entities[usize::from(actor)];
    assert_eq!((row.action, u16::from_le_bytes([row.record[154], row.record[155]])), (138, enemy));
    let before = u16::from_le_bytes([row.record[50], row.record[51]]);
    mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor, &[parent], &families, Default::default()).unwrap();
    let row = &arena.entities[usize::from(actor)];
    assert_eq!(u16::from_le_bytes([row.record[50], row.record[51]]), before - 1);
    assert_eq!(row.action, 138, "a selected class5 enemy remains a valid target");

    let row = &mut arena.entities[usize::from(actor)];
    row.set_source_action(143); put(row, 50, 1);
    mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor, &[parent], &families, Default::default()).unwrap();
    let row = &arena.entities[usize::from(actor)];
    assert_eq!((row.record[77], row.record[76], u16::from_le_bytes([row.record[44], row.record[45]])), (10, 3, 0));
    assert!(!row.drawing_disabled());
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
        (RULES.max_life, RULES.max_life, RULES.max_life >> 1)
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
        models: None,
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
        models: None,
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
        models: None,
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
        models: None,
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
        models: None,
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
        models: None,
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
fn attack_stance_and_recovery_follow_each_source_visit() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let runtime = simulation.runtime();
    let mut counter = 0;
    let point = Position {
        x: 20_000,
        y: 20_000,
        z: 8_000,
    };
    let mut actor = construct(41, 9, 0x1234, point, &mut counter);
    let mut ordinary = actor;
    ordinary.position.z = terrain.ground_height(point).wrapping_add(500);
    ordinary.speed = 0;
    ordinary.yaw = 0;
    ordinary.roll = 512;
    let mut strike = ordinary;
    strike.descriptor = STRIKE_DESCRIPTOR;
    let above_floor = ordinary.position.z;
    move_actor(&mut ordinary, terrain, &mut Creature17Effects::default());
    move_actor(&mut strike, terrain, &mut Creature17Effects::default());
    assert_eq!(
        (ordinary.position.z, strike.position.z),
        (
            above_floor + RULES.descent,
            above_floor + RULES.stance_descent
        )
    );
    assert_eq!(
        (ordinary.yaw, strike.yaw),
        (RULES.turn_limit, 512),
        "actual current descriptor87 owns its1024 turn bound and zero relaxation"
    );
    ordinary.position.z = above_floor;
    strike.position.z = above_floor;
    settle(&mut ordinary, terrain);
    settle(&mut strike, terrain);
    assert_eq!(
        (ordinary.position.z, strike.position.z),
        (
            above_floor + RULES.descent,
            above_floor + RULES.stance_descent
        )
    );
    actor.action = 138;
    actor.target = 10;
    actor.phase = RULES.source_scan_period;
    let mut target = Target {
        slot: 10,
        source_id: 10,
        class: 3,
        model: 0,
        life: 1000,
        position: Position {
            x: point.x + 500,
            ..point
        },
        ..Target::default()
    };
    let visit = |actor: &mut Creature17State, target: Target| {
        actor.control_flags |= 0x800;
        step(
            actor,
            &Creature17Context {
                terrain,
                runtime,
                targets: &[target],
                class3: &[10],
                model17: &[],
                models: None,
            },
        )
        .unwrap()
    };
    visit(&mut actor, target);
    assert_eq!(
        (actor.attack_phase, actor.descriptor, actor.timer),
        (1, DESCRIPTOR, 0),
        "cadenced phase0 selects phase1 without executing its next callback"
    );
    actor.phase = actor.phase.wrapping_add(1);
    visit(&mut actor, target);
    assert_eq!(
        (
            actor.attack_phase,
            actor.descriptor,
            actor.timer,
            actor.speed
        ),
        (2, STRIKE_DESCRIPTOR, 0, RULES.minimum_speed * 3)
    );
    let roll = actor.roll;
    target.position.x = point.x + 4000;
    actor.phase = actor.phase.wrapping_add(1);
    let z = actor.position.z;
    visit(&mut actor, target);
    assert_eq!(
        (actor.action, actor.attack_phase, actor.timer, actor.roll),
        (138, 2, 1, roll),
        "airborne phases neither wait for cadence nor refresh their heading/range choice"
    );
    assert_eq!(actor.position.z, z + RULES.strike_vertical_step);
    actor.phase = actor.phase.wrapping_add(1);
    target.position = actor.position;
    let strike = visit(&mut actor, target);
    assert_eq!(
        strike.melee,
        Some((10, i32::from(RULES.constructor_subspell), actor.source_id))
    );
    assert_eq!((actor.attack_phase, actor.timer), (3, 2));
    actor.attack_phase = 4;
    actor.timer = 1;
    visit(&mut actor, target);
    assert_eq!(
        (actor.attack_phase, actor.timer),
        (4, 0),
        "recovery tests the previous counter"
    );
    visit(&mut actor, target);
    assert_eq!(
        (actor.attack_phase, actor.timer, actor.speed),
        (0, -1, actor.minimum_speed)
    );
}
#[test]
fn configured_stage13_and16_preserve_countdown_target_damage_and_expiry_products() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain().clone();
    let runtime = simulation.runtime().clone();
    let point = Position {
        x: 20_000,
        y: 20_000,
        z: 0,
    };
    let parent = Target {
        slot: 10,
        source_id: 10,
        class: 3,
        model: 0,
        life: 100,
        position: point,
        ..Target::default()
    };
    let hostile = Target {
        slot: 11,
        source_id: 11,
        position: Position { x: 22_000, ..point },
        ..parent
    };
    for mode in [13, 16] {
        let mut count = 0;
        let mut actor = construct(41, 9, 0x1234, point, &mut count);
        actor.action = 143;
        actor.route_mode = mode;
        actor.parent = parent.slot;
        actor.route_remaining = 10;
        actor.phase = 1;
        actor.target = hostile.slot;
        actor.control_flags |= 0x800;
        let effects = step(
            &mut actor,
            &Creature17Context {
                terrain: &terrain,
                runtime: &runtime,
                targets: &[parent, hostile],
                class3: &[10, 11],
                model17: &[],
                models: None,
            },
        )
        .unwrap();
        assert_eq!(actor.route_remaining, if mode == 13 { 9 } else { 10 });
        assert_eq!(effects.stage_expiry, None);
        actor.target = 0;
        actor.control_flags |= 0x800;
        step(
            &mut actor,
            &Creature17Context {
                terrain: &terrain,
                runtime: &runtime,
                targets: &[parent],
                class3: &[10],
                model17: &[],
                models: None,
            },
        )
        .unwrap();
        assert_eq!(
            actor.route_remaining,
            if mode == 13 { 4 } else { 6 },
            "parent fallback costs4 after mode13's own tick"
        );
        assert_eq!(actor.target, 0);
        actor.target = hostile.slot;
        actor.pending_damage = 1;
        actor.pending_attacker = hostile.slot;
        actor.control_flags |= 0x800;
        let effects = step(
            &mut actor,
            &Creature17Context {
                terrain: &terrain,
                runtime: &runtime,
                targets: &[parent, hostile],
                class3: &[10, 11],
                model17: &[],
                models: None,
            },
        )
        .unwrap();
        assert_eq!(
            (actor.life, actor.action, actor.attack_phase),
            (RULES.max_life - 1, 138, 0)
        );
        assert_eq!(effects.feedback, [(parent.slot, 19)]);
    }
    // These are explicitly configured helper modes, not a claim that the
    // ordinary Army table creates model17. Use real constructors/allocators
    // and the shared expiry publisher for their advertised consequences.
    for mode in [13, 16] {
        let particles = simulation.session.definition.presentation.particles.clone();
        let live = &mut simulation.session.state.live;
        let slot = add_creature(&mut live.economy, &mut live.terrain, &particles, point, 143);
        live.economy.entities[usize::from(slot)].record[77] = mode;
        let before = live
            .economy
            .entities
            .iter()
            .filter(|row| row.active)
            .map(|row| (row.index, live.economy.slot_generation(row.index)))
            .collect::<Vec<_>>();
        let classes = class3_slots(&live.economy);
        let families = model_slots(&live.economy);
        mc2::step_row(
            &mut live.economy,
            &mut live.core.runtime,
            &mut live.terrain,
            &particles,
            slot,
            &classes,
            &families,
            Default::default(),
        )
        .unwrap();
        assert!(live.economy.entities[usize::from(slot)].drawing_disabled());
        let children = live
            .economy
            .entities
            .iter()
            .filter(|row| {
                row.active
                    && !before.contains(&(row.index, live.economy.slot_generation(row.index)))
            })
            .map(|row| (row.class, row.model))
            .collect::<Vec<_>>();
        assert!(children.contains(&(10, 0)));
        assert_eq!(children.contains(&(10, 73)), mode == 13);
    }
}

#[test]
fn summoned_fixed_addresses_read_retained_parent_target_and_current_reuse() {
    let mut simulation = crate::native_world::tests::level_two_campaign_session();
    let particles = simulation.session.definition.presentation.particles.clone();
    let live = &mut simulation.session.state.live;
    let parent = live.core.state.entity_index;
    let target = live
        .economy
        .entities
        .iter()
        .find(|row| row.active && (row.class, row.model) == (3, 1))
        .unwrap()
        .index;
    let point = live.economy.entities[usize::from(parent)].position;
    let slot = add_creature(&mut live.economy, &mut live.terrain, &particles, point, 143);
    {
        let actor = &mut live.economy.entities[usize::from(slot)];
        actor.record[77] = 16;
        put(actor, 44, parent);
        put(actor, 154, target);
        put(actor, 50, 10);
        actor.set_phase(1);
        actor.set_control_flags(actor.control_flags | 0x800);
    }
    // Explicitly controlled source release exercises the fixed-address read
    // contract, not an assertion that ordinary player death frees this row.
    // Real imported packets retain their life/pose/model after class clears.
    for (address, life) in [(target, 100), (parent, 0)] {
        let position = live.economy.entities[usize::from(address)].position;
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            address,
            position,
            false,
            false,
        )
        .unwrap();
        live.economy.entities[usize::from(address)].set_life(life);
        live.economy.entities[usize::from(address)].reclaim_source_row();
        live.economy.economy.release_slot(address, None);
    }
    let families = model_slots(&live.economy);
    let original_generation = live.economy.slot_generation(parent);
    mc2::step_row(
        &mut live.economy,
        &mut live.core.runtime,
        &mut live.terrain,
        &particles,
        slot,
        &[],
        &families,
        Default::default(),
    )
    .unwrap();
    let actor = &live.economy.entities[usize::from(slot)];
    assert!(
        !actor.drawing_disabled(),
        "source1E580 admits zero-life current parent packets"
    );
    assert_eq!(
        put_word(actor, 154),
        target,
        "positive-life retained selected target stays readable"
    );
    assert_eq!(
        put_word(actor, 50),
        10,
        "mode16 retains its timer while a selected target exists"
    );

    let replacement = crate::world::effects::allocate(
        &mut live.economy,
        &mut live.terrain,
        sim_definition::adapter::mc2_effect_definition(crate::level_definition::EffectKind::Impact),
        point,
        &particles,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        replacement, parent,
        "the actual NewEvent consumes the last released source address"
    );
    assert!(live.economy.slot_generation(parent) > original_generation);
    let flags = live.economy.entities[usize::from(slot)].control_flags;
    live.economy.entities[usize::from(slot)].set_control_flags(flags | 0x800);
    mc2::step_row(
        &mut live.economy,
        &mut live.core.runtime,
        &mut live.terrain,
        &particles,
        slot,
        &[],
        &families,
        Default::default(),
    )
    .unwrap();
    assert!(
        !live.economy.entities[usize::from(slot)].drawing_disabled(),
        "the fixed parent pointer follows the new live impact rather than an old generation"
    );
    live.economy.entities[usize::from(parent)].set_life(-1);
    mc2::step_row(
        &mut live.economy,
        &mut live.core.runtime,
        &mut live.terrain,
        &particles,
        slot,
        &[],
        &families,
        Default::default(),
    )
    .unwrap();
    assert!(
        live.economy.entities[usize::from(slot)].drawing_disabled(),
        "the current replacement parent's negative life triggers real expiry"
    );
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
        arena, runtime, terrain, &particles, actor_slot, &classes, &families, Default::default(),
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

    // Level 9 contains actual reset-imported model17 actors and their package
    // semantic bindings. Position and aim are controlled for this collision
    // check; the actor, attack, scheduler and player state are real carriers.
    let level9 = std::env::var_os("MC_LEVEL9_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/group-zero-owner-package-v4/level9"),
        std::path::PathBuf::from,
    );
    let mut scheduled = crate::campaign::CampaignSession::open(
        level9,
        crate::campaign::spec::CampaignSelection::ordinary(9).unwrap(),
    )
    .expect("qualified Level 9 campaign")
    .simulation()
    .clone();
    let mut melee = scheduled.clone();
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
            .expect("retained Level 9 model17 actor");
        let position = Position {
            x: actor_x,
            y: target_position.y,
            z: target_position.z,
        };
        actor.set_position(position);
        assert_eq!(actor.action, 143, "retain the real authored StageVar birth");
        actor.set_yaw(actor_yaw);
        actor.set_source_roll(actor_yaw);
        actor.set_phase(RULES.source_scan_period.wrapping_sub(1));
        actor.set_control_flags(actor.control_flags | 0x800);
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
        crate::native_world::area_damage::contact(&mut live.economy, None, slot, 1, target_slot)
            .expect("ordinary incoming damage selects the actual player attacker");
        (slot, source_id, target_slot)
    };
    scheduled.step_outer_turn(crate::PlayerCommand::default(), 0);
    assert_eq!(
        scheduled.entities()[usize::from(actor_slot)].action,
        138,
        "the real143 damage callback selects pursue without a forced target/action"
    );
    assert_eq!(
        put_word(&scheduled.entities()[usize::from(actor_slot)], 154),
        target_slot
    );
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

    // A second genuine imported actor takes the close branch from the same
    // StageVar incoming-damage path. Only combat positions and initial source
    // cadence are controls; stance, strike, hit and recovery all schedule.
    // A fresh player has100 source protection visits. Let those callbacks
    // expire naturally before combat; the protected prefix clears incoming
    // contact mailboxes instead of applying them to life.
    let initial_protection = melee.session.state.live.player_protection_ticks;
    assert!(initial_protection > 0);
    for _ in 0..initial_protection {
        let live = &melee.session.state.live;
        let player = &live.economy.entities[usize::from(live.core.state.entity_index)];
        assert!(
            !live.economy.player_castle_shift_aabb_overlap(player),
            "the declared combat observer must not refresh Castle protection"
        );
        if live.player_protection_ticks == 0 {
            break;
        }
        melee.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            melee
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "natural player protection prelude: {:?}",
            melee.dispatch_events()
        );
    }
    assert_eq!(
        melee.session.state.live.player_protection_ticks, 0,
        "ordinary scheduled player visits expire the source protection counter"
    );
    let player = melee.state.entity_index;
    let player_life = melee.canonical_player_life_for_test();
    let actor = {
        let live = &mut melee.session.state.live;
        let player_pose = live.core.state.position;
        let actor = live
            .economy
            .entities
            .iter_mut()
            .find(|row| row.active && (row.class, row.model) == (5, SOURCE_MODEL))
            .unwrap();
        assert_eq!(actor.action, 143);
        let pose = Position {
            x: player_pose.x.wrapping_add(384),
            ..player_pose
        };
        actor.set_position(pose);
        let yaw = heading_for_positions(pose, player_pose) as i16;
        actor.set_yaw(yaw);
        actor.set_source_roll(yaw);
        actor.set_phase(RULES.source_scan_period.wrapping_sub(2));
        actor.set_control_flags(actor.control_flags | 0x800);
        let slot = actor.index;
        crate::world::effects::sync_position(
            &mut live.economy,
            &mut live.terrain,
            slot,
            pose,
            true,
            false,
        )
        .unwrap();
        crate::native_world::area_damage::contact(&mut live.economy, None, slot, 1, player)
            .unwrap();
        slot
    };
    let mut phases = Vec::new();
    let mut contact_receipts = Vec::new();
    let mut recovered = false;
    for _ in 0..80 {
        melee.step_outer_turn(crate::PlayerCommand::default(), 0);
        assert!(
            melee
                .dispatch_events()
                .iter()
                .all(|event| !matches!(event, crate::native_world::Mc2DispatchEvent::Error { .. })),
            "actual model17 strike: {:?}",
            melee.dispatch_events()
        );
        let row = &melee.entities()[usize::from(actor)];
        let phase = row.record[74];
        if phases.last() != Some(&phase) {
            phases.push(phase);
        }
        if phase == 2 {
            assert_eq!(row.parameter_relation, Some(STRIKE_DESCRIPTOR));
        }
        // This actor precedes the player numerically, so the player's later
        // visit consumes the owner mailbox before the outer-turn observation.
        // The existing event retains the applied payload and its attribution.
        for event in melee.dispatch_events() {
            if let crate::native_world::Mc2DispatchEvent::Economy {
                event:
                    crate::native_world::Level2EconomyEvent::DamageApplied {
                        target_entity_index,
                        attacker_entity_index,
                        damage,
                        lethal,
                    },
                ..
            } = event
                && *target_entity_index == player
                && *attacker_entity_index == row.source_id
            {
                contact_receipts.push((*damage, *lethal));
            }
        }
        if phases.contains(&4) && phase == 0 {
            assert_eq!(row.parameter_relation, Some(DESCRIPTOR));
            recovered = true;
            break;
        }
    }
    assert!(
        phases.windows(4).any(|phases| phases == [1, 2, 3, 4]),
        "ordinary source strike phases must remain distinct: {phases:?}"
    );
    assert!(
        contact_receipts.contains(&(i32::from(RULES.constructor_subspell), false))
            && melee.canonical_player_life_for_test() < player_life,
        "the actual actor's350 contact is consumed by the live player: actor={actor}, \
         player={player}, receipts={contact_receipts:?}, life={player_life}->{current_life}",
        current_life = melee.canonical_player_life_for_test()
    );
    assert!(
        recovered,
        "the full18-count recovery restores descriptor85 and phase0"
    );
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
        arena, runtime, terrain, &particles, root, &classes, &families, Default::default(),
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

    // The qualified Level 9 reset contains actual model17 roots. Drive a
    // nonlethal and then lethal contact through their normal scheduler and
    // inspect the same class-5 list used by absence objectives.
    let package = std::env::var_os("MC_LEVEL9_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/group-zero-owner-package-v4/level9"),
        std::path::PathBuf::from,
    );
    let mut scheduled = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(9).unwrap(),
    )
    .expect("qualified Level 9 campaign")
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
    let package = std::env::var_os("MC_LEVEL9_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/group-zero-owner-package-v4/level9"),
        std::path::PathBuf::from,
    );
    let mut absence = crate::campaign::CampaignSession::open(
        package,
        crate::campaign::spec::CampaignSelection::ordinary(9).unwrap(),
    )
    .expect("qualified Level 9 campaign")
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
        models: None,
    };

    let effects = step(&mut actor, &context).expect("action138 is supported");

    assert_eq!(effects.sound, Some(58));
    assert_eq!(actor.action, 140, "lethal damage redirects to PreKill");
}

fn put_word(row: &crate::native_world::EntitySnapshot, offset: usize) -> u16 {
    u16::from_le_bytes([row.record[offset], row.record[offset + 1]])
}

#[test]
fn configured_stage_callbacks_keep_source_hit_tail_and_wander_order() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let mut runtime = simulation.runtime().clone();
    let mut position = simulation.state.position;
    position.z = terrain.ground_height(position);
    let mut counter = 0;
    let base = construct(41, 1, 1234, position, &mut counter);
    let friend = Target { slot: 42, class: 5, model: SOURCE_MODEL, life: 1000,
        position, ..Target::default() };
    let player_position = Position { x: position.x.wrapping_add(100), ..position };
    let player = Target { slot: 10, class: 3, model: 0, life: 1000,
        position: player_position, ..Target::default() };
    let followed = Target { slot: 43, class: 5, model: 1, life: -1,
        position, target: player.slot, ..Target::default() };
    runtime.stage_variables[1].flags = 0;
    runtime.stage_variables[1].relation = crate::native_world::RuntimeRelation::LiveEntity(followed.slot);
    let targets = [friend, player, followed];
    let models: [Vec<u16>; 29] = std::array::from_fn(|_| Vec::new());
    let context = Creature17Context { terrain, runtime: &runtime, targets: &targets,
        class3: &[player.slot], model17: &[], models: Some(&models) };
    let mut point_hit = base;
    point_hit.action = 143;
    point_hit.route_mode = 1;
    point_hit.pending_damage = 1;
    point_hit.pending_attacker = friend.slot;
    point_hit.target = 777;
    step(&mut point_hit, &context).unwrap();
    assert_eq!((point_hit.action, point_hit.route_mode, point_hit.target), (143, 1, 777),
        "same-family point damage settles without retaliating");
    assert_eq!(point_hit.life, RULES.max_life - 1);

    let mut guard_hit = base;
    guard_hit.action = 143;
    guard_hit.route_mode = 2;
    guard_hit.route_index = 1;
    guard_hit.phase = RULES.source_scan_period;
    guard_hit.yaw = heading_for_positions(position, player_position) as i16;
    guard_hit.pending_damage = 1;
    guard_hit.pending_attacker = friend.slot;
    step(&mut guard_hit, &context).unwrap();
    assert_eq!((guard_hit.action, guard_hit.route_mode, guard_hit.target), (138, 10, player.slot),
        "guard scans the real class3 list after same-family damage");

    for mode in 3..=5 {
        let mut actor = base;
        actor.action = 143;
        actor.route_index = 1;
        actor.route_mode = mode;
        actor.phase = 8;
        actor.life = 10;
        actor.pending_damage = 20;
        actor.pending_attacker = player.slot;
        step(&mut actor, &context).unwrap();
        assert!(actor.life < 0);
        match mode {
            3 => assert_eq!((actor.action, actor.target), (138, followed.slot)),
            4 => assert_eq!((actor.action, actor.target), (138, player.slot)),
            5 => assert_eq!(actor.action, 140),
            _ => unreachable!(),
        }
    }
    for mode in 6..=9 {
        let mut actor = base;
        actor.action = 143;
        actor.route_index = 0;
        actor.route_mode = mode;
        actor.phase = 16;
        actor.speed = 0;
        actor.rng = 1234;
        step(&mut actor, &context).unwrap();
        assert_eq!((actor.action, actor.route_index, actor.route_mode), (143, 0, mode));
        if actor.control_flags & 0x40000 == 0 {
            assert_eq!(actor.rng, crate::native_world::level2_next_rng16(1234));
        }
    }
    let mut expired = base;
    expired.action = 143;
    expired.route_mode = 10;
    expired.target = 0;
    step(&mut expired, &context).unwrap();
    assert_eq!((expired.action, expired.target), (138, 0));

    let parent = Target { slot: 77, life: 1, flags: 0x400, position: player_position,
        yaw: 101, roll: 202, ..Target::default() };
    let sentinel = Target { slot: 0, life: -1, position: Position { z: -7, ..position },
        yaw: 303, roll: 404, ..Target::default() };
    let parent_targets = [parent, sentinel];
    let parent_context = Creature17Context { terrain, runtime: &runtime,
        targets: &parent_targets, class3: &[], model17: &[], models: Some(&models) };
    let mut follower = base;
    follower.action = 143;
    follower.route_mode = 12;
    follower.parent = parent.slot;
    let effects = step(&mut follower, &parent_context).unwrap();
    assert!(!effects.route_hide, "hidden living parent remains a source12 parent");
    assert_eq!((follower.position, follower.yaw, follower.roll), (parent.position, 101, 202));
    follower.parent = 0;
    let effects = step(&mut follower, &parent_context).unwrap();
    assert!(effects.route_hide);
    assert_eq!((follower.position.z, follower.yaw, follower.roll), (0, 303, 404),
        "source12 still copies the current sentinel after hiding");
}

#[test]
fn configured_linked_hit_preserves_source_parent_and_actor_order() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let runtime = simulation.runtime();
    let mut counter = 0;
    let base = construct(41, 1, 1234, simulation.state.position, &mut counter);
    for self_link in [false, true] {
        let linked = if self_link { base.slot } else { 42 };
        let peer = Target { slot: linked, class: 5, model: SOURCE_MODEL, life: 1000,
            position: base.position, ..Target::default() };
        let attacker = Target { slot: 10, class: 3, model: 0, life: 1000,
            position: base.position, ..Target::default() };
        let targets = [peer, attacker];
        let context = Creature17Context { terrain, runtime, targets: &targets,
            class3: &[10], model17: &[], models: None };
        let mut actor = base;
        actor.action = 139;
        actor.life = 10;
        actor.linked_peer = linked;
        actor.target = 777;
        actor.pending_damage = 20;
        actor.pending_attacker = attacker.slot;
        let effects = step(&mut actor, &context).unwrap();
        assert_eq!((actor.action, actor.linked_peer, actor.killer), (140, 0, attacker.slot));
        assert_eq!(actor.target, if self_link { attacker.slot } else { 777 });
        if !self_link {
            assert_eq!(effects.linked_updates, vec![LinkedUpdate {
                slot: linked, action: 138, target: attacker.slot, linked_peer: 0,
            }]);
        }
    }
}

#[test]
fn configured_linked_next_reads_sentinel_and_signed_peer_coordinates() {
    let simulation = crate::native_world::tests::level_two_campaign_session();
    let terrain = simulation.canonical_terrain();
    let mut counter = 0;
    let mut position = simulation.state.position;
    position.x = 32760;
    position.z = terrain.ground_height(position);
    let mut actor = construct(41, 1, 1234, position, &mut counter);
    actor.action = 139;
    actor.linked_peer = 42;
    actor.phase = RULES.source_scan_period;
    actor.speed = 0;
    let parent = Target { slot: 42, source_id: 42, class: 5, model: SOURCE_MODEL,
        life: 1000, action: 139, linked_peer: 0, maximum_speed: 30, speed: 7,
        position: Position { x: 30000, ..position }, ..Target::default() };
    let sentinel = Target { slot: 0, life: -1,
        position: Position { y: position.y.wrapping_add(3000), ..position }, ..Target::default() };
    // These are close as unsigned coordinates but far apart as signed i16.
    let other = Target { slot: 43, source_id: 43, class: 5, model: SOURCE_MODEL,
        life: -1, position: Position { x: 32775, ..position }, ..Target::default() };
    let rows = [parent, sentinel, other];
    let context = Creature17Context { terrain, runtime: simulation.runtime(), targets: &rows,
        class3: &[], model17: &[43], models: None };
    step(&mut actor, &context).unwrap();
    assert_eq!((actor.action, actor.linked_peer, actor.speed), (139, 0, 37));
    assert_eq!(actor.roll, heading_for_positions(actor.position, sentinel.position) as i16,
        "source follows sentinel and rejects unsigned-only peer proximity");
}

#[test]
fn configured_death_and_failed_allocation_keep_source_effect_order() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut base = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let mut runtime = session.runtime().clone();
    let particles = &session.session.definition.presentation.particles;
    let player = session.state.entity_index;
    let actor = add_creature(&mut base, &mut terrain, particles, session.state.position, 141);
    let original = base.handle(actor).unwrap();
    let source_id = actor.wrapping_add(1000);
    {
        let row = &mut base.entities[usize::from(actor)];
        row.source_id = source_id;
        put(row, 30, source_id);
        row.set_mana(700);
        row.record[152..154].copy_from_slice(&player.to_le_bytes());
        row.set_phase(0);
    }
    let families = model_slots(&base);
    let classes = class3_slots(&base);
    let earlier_spheres: Vec<_> = base.economy.spheres().iter().map(|sphere| sphere.source_entity_index).collect();
    let mut success = base.clone();
    let mut success_terrain = terrain.clone();
    mc2::step_row(&mut success, &mut runtime.clone(), &mut success_terrain, particles,
        actor, &classes, &families, Default::default()).unwrap();
    let (&burst, &(generation, parent)) = success.effects.death_burst_origins().iter()
        .find(|(_, (_, parent))| *parent == actor).expect("actual source10/1 burst birth");
    let burst_row = &success.entities[usize::from(burst)];
    assert_eq!((parent, burst_row.class, burst_row.model, burst_row.source_id),
        (actor, 10, 1, source_id));
    assert_eq!(success.handle(burst).unwrap().generation, generation);
    assert!(success.economy.spheres().iter().any(|sphere| sphere.active && sphere.mana == 700
        && sphere.owner_entity_index == player && !earlier_spheres.contains(&sphere.source_entity_index)),
        "the actual death conversion carries the whole source mana");
    assert!(success.entities[usize::from(actor)].drawing_disabled());
    assert_eq!(success.entities[usize::from(actor)].source_player_entity_index(), 0);

    // Explicit capacity-failure control: neither effect can allocate; source
    // still consumes its positive-mana RNG draw, clears owner and hides.
    let mut exhausted = base.clone();
    for row in &mut exhausted.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    exhausted.economy.exhaust_new_event_capacity_for_test();
    exhausted.rebuild_source_free_arena();
    let rng = exhausted.entities[usize::from(actor)].entity_rng;
    mc2::step_row(&mut exhausted, &mut runtime, &mut terrain, particles,
        actor, &classes, &families, Default::default()).unwrap();
    assert_eq!(exhausted.handle(actor), Some(original));
    let row = &exhausted.entities[usize::from(actor)];
    assert!(row.drawing_disabled());
    assert_eq!(row.entity_rng, crate::native_world::level2_next_rng16(rng));
    assert_eq!(row.source_player_entity_index(), 0);
    assert!(!exhausted.effects.death_burst_origins().values().any(|(_, parent)| *parent == actor));
}

#[test]
fn configured_constructor_and_ranged_allocation_failure_preserve_source_prelude() {
    let session = crate::native_world::tests::level_two_campaign_session();
    let mut arena = session.source_arena().clone();
    let mut terrain = session.canonical_terrain().clone();
    let mut runtime = session.runtime().clone();
    let particles = &session.session.definition.presentation.particles;
    let player = session.state.entity_index;
    let target_position = arena.entities[usize::from(player)].position;
    let mut position = target_position;
    position.x = position.x.wrapping_add(2048);
    let actor = add_creature(&mut arena, &mut terrain, particles, position, 138);
    {
        let row = &mut arena.entities[usize::from(actor)];
        row.set_phase(RULES.source_scan_period);
        row.set_actual_speed(0);
        row.record[74] = 0;
        put(row, 154, player);
    }
    let mut available = arena.clone();
    let mut available_terrain = terrain.clone();
    let mut available_runtime = runtime.clone();
    let available_families = model_slots(&available);
    let available_classes = class3_slots(&available);
    let shot = mc2::step_row(&mut available, &mut available_runtime, &mut available_terrain,
        particles, actor, &available_classes, &available_families, Default::default()).unwrap();
    let projectile = shot.projectile_slot.expect("same source callback allocates with capacity");
    assert_eq!((available.entities[usize::from(projectile)].class,
        available.entities[usize::from(projectile)].model), (9, 20));
    // Configured replacement pressure makes NewEvent reuse the caller's own
    // fixed address. The current projectile, not the old actor packet, owns
    // the map chain after that allocation.
    let mut reused = arena.clone();
    let mut reused_terrain = terrain.clone();
    let mut reused_runtime = runtime.clone();
    for row in &mut reused.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    let flags = reused.entities[usize::from(actor)].control_flags;
    reused.entities[usize::from(actor)].set_control_flags(
        flags | crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    reused.economy.exhaust_new_event_capacity_for_test();
    reused.rebuild_source_free_arena();
    let reused_families = model_slots(&reused);
    let reused_classes = class3_slots(&reused);
    let original = reused.handle(actor).unwrap();
    let replacement = mc2::step_row(&mut reused, &mut reused_runtime, &mut reused_terrain,
        particles, actor, &reused_classes, &reused_families, Default::default()).unwrap();
    assert_eq!(replacement.projectile_slot, Some(actor));
    assert!(reused.resolve(original).is_none());
    let current = &reused.entities[usize::from(actor)];
    assert_eq!((current.class, current.model, current.position.x, current.position.y), (9, 20, 0, 0));
    assert_eq!(reused.economy.map_chains.tile_of(actor), 0,
        "the prior actor's position must not republish over the replacement");
    for row in &mut arena.entities {
        row.set_control_flags(row.control_flags & !crate::level1_mission::SOURCE_REPLACEABLE_FLAG);
    }
    arena.economy.exhaust_new_event_capacity_for_test();
    arena.rebuild_source_free_arena();
    let counter_before = runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)];
    assert!(mc2::allocate_authored(&mut arena, &mut runtime, &mut terrain, particles,
        position, RULES.max_life, None, 500).unwrap().is_none());
    assert_eq!(runtime.constructor_counters().unwrap()[usize::from(SOURCE_MODEL)], counter_before,
        "failed NewEvent leaves the source constructor counter unchanged");

    let families = model_slots(&arena);
    let classes = class3_slots(&arena);
    let events_before = arena.economy.events().len();
    assert_eq!(mc2::prepare_callback_sound(&mut arena, actor), Some(58));
    let outcome = mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles,
        actor, &classes, &families, Default::default()).unwrap();
    assert!(outcome.projectile_slot.is_none(), "exhausted source cannot publish a 9/20");
    assert!(matches!(arena.economy.events()[events_before],
        crate::native_world::Level2EconomyEvent::PreparedSound {
            source_entity_index, sound_id: 58, ..
        } if source_entity_index == actor), "action138 sound precedes the failed allocation");
    assert!(arena.entities[usize::from(actor)].active);
}


#[test]
fn level_nine_public_meteor_reclaims_original_manticores_and_completes_absence() {
    use crate::campaign::{CampaignSession, spec::CampaignSelection};
    use crate::level_definition::{CreatureKind, ObjectiveCondition};
    use crate::native_world::{area_damage, Mc2DispatchEvent};
    use crate::PlayerCommand;

    let package = std::env::var_os("MC_LEVEL9_PACKAGE").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/ribbon-20261104-integration/group-zero-owner-package-v4/level9"),
        std::path::PathBuf::from,
    );
    let mut session = CampaignSession::open(package, CampaignSelection::ordinary(9).unwrap())
        .expect("whole Level9 owner package").simulation().clone();
    let player = session.state.entity_index;
    let owner = usize::from(session.state.player_color_index);
    let sensor = session.session.definition.semantic_level().unwrap().objectives().iter()
        .position(|objective| objective.id().as_str() == "objective.campaign.task_5")
        .expect("actual Level9 Manticore absence objective");
    assert_eq!(session.session.definition.semantic_level().unwrap().objectives()[sensor].condition(),
        ObjectiveCondition::NoLivingCreatures(CreatureKind::Creature17));
    let originals = [67_u16, 68].map(|slot| {
        let row = &session.entities()[usize::from(slot)];
        assert_eq!((row.class, row.model, row.life), (5, 17, 10_000));
        session.source_arena().handle(slot).expect("actual reset root")
    });
    // Declared entry controls: carried Meteor, the normal Castle producer and
    // its mana prerequisite, spell XP/tier, player resources and objective.
    session.apply_carried_spell_families(&[9], Some(9)).unwrap();
    for (stage, footprint) in crate::native_world::campaign_castle_stage_footprints().unwrap() {
        session.economy_mut().set_castle_stage_footprint(stage, footprint);
    }
    let site = (1_u16..32).flat_map(|y| (1_u16..32).map(move |x| Position {
        x: x * 2048, y: y * 2048, z: 0,
    })).find(|position| !session.economy().castle_projectile_site_blocked(
        *position, session.entities(), session.canonical_terrain())).expect("normal Castle site");
    let castle = session.economy_mut().build_castle(site, player).expect("normal Castle constructor");
    assert!(session.economy_mut().set_castle_mana_prerequisite_for_test(castle, 200_000));
    session.shared_spells_mut().award_spell_experience(player, 9, 5_000);
    {
        let live = &mut session.session.state.live;
        let parent = &mut live.economy.entities[usize::from(player)];
        parent.set_max_life(200_000);
        parent.set_life(200_000);
        parent.max_mana = 200_000;
        parent.record[144..148].copy_from_slice(&parent.max_mana.to_le_bytes());
        parent.set_mana(200_000);
        parent.set_mana_regen(0);
        let parent = parent.clone();
        live.player_life = parent.life;
        live.shared_spells.sync_player_entity_from_authority(&parent);
        live.shared_spells.player_mana = parent.mana;
        live.shared_spells.caster.mana = parent.mana;
        live.shared_spells.sync_typed_spell_parent_caches_from_player_entity();
        if let Some(fire) = live.core.context.spell_fire.as_mut() { fire.caster.mana = parent.mana; }
        let runtime = &mut live.core.runtime;
        for stage in 0..usize::from(runtime.stage_index) {
            runtime.set_level2_objective_stage_selector_for(owner, stage, u8::from(stage == sensor));
        }
        runtime.set_level2_objective_text_selector_for(owner, sensor);
    }
    let ui = crate::SOURCE_SPELL_INDEX_MAP.iter().position(|family| *family == 9).unwrap() as i8;
    session.step_outer_turn(PlayerCommand { opcode: 31, argument_1: ui, argument_2: 2,
        ..Default::default() }, 0);
    let holder = session.shared_spells().local_player().enabled_spell_holders[9];
    assert_ne!(holder, 0);
    assert_eq!(session.shared_spells().pickup(holder).unwrap().level_gate, 2);
    let holder_handle = session.source_arena().handle(holder).expect("current Meteor holder");
    let caster_handle = session.source_arena().handle(player).expect("current player caster");
    let credit_before = session.economy().authored_bee_kill_census(player);
    let slots = originals.map(|handle| handle.source_slot);
    area_damage::start_effect_damage_trace_for_targets(&slots);
    area_damage::start_actor_damage_trace(&originals);
    let mut callbacks = Vec::new();
    let mut launches = Vec::new();
    let mut impacts = Vec::new();
    let mut observed_death = [false; 2];
    let mut hidden = [false; 2];
    // Exactly one public cast per surviving original; no repeated aiming,
    // forecast, target control or NPC state writes. A miss remains a failure.
    for target in originals {
        if session.source_arena().resolve(target).is_none() { continue; }
        let target_position = session.entities()[usize::from(target.source_slot)].position;
        let mut origin = target_position;
        crate::move_entity(&mut origin, 1024, 0, 2048);
        origin.z = session.canonical_terrain().ground_height(target_position).wrapping_add(768);
        session.set_level2_entity_position(player, origin).unwrap();
        session.state.actual_speed = 0;
        session.state.target_speed = 0;
        let (_, pitch) = crate::level1_mission::projectile_guidance_angles(origin, target_position);
        session.set_level2_player_aim_for_test(0, pitch);
        for turn in 0..100 {
            session.start_creature17_trace();
            session.step_outer_turn(if turn == 0 { PlayerCommand { flags: 0x10, ..Default::default() } }
                else { PlayerCommand::default() }, 0);
            let trace = session.take_creature17_trace();
            launches.extend(trace.meteor_launches);
            impacts.extend(trace.meteor_impacts);
            let turn_callbacks = trace.callbacks;
            for callback in &turn_callbacks {
                if let Some(index) = originals.iter().position(|target| *target == callback.source)
                    && let Some(after) = callback.after
                {
                    observed_death[index] |= after.life < 0 && after.killer == player;
                    hidden[index] |= after.control_flags & 0x400 != 0;
                }
            }
            callbacks.extend(turn_callbacks);
            assert!(session.dispatch_events().iter().all(|event| !matches!(event, Mc2DispatchEvent::Error { .. })),
                "Meteor callback: {:?}", session.dispatch_events());
            for (i, original) in originals.iter().enumerate() {
                if session.source_arena().resolve(*original).is_some() {
                    let row = &session.entities()[usize::from(original.source_slot)];
                    if row.life < 0 {
                        observed_death[i] |= put_word(row, 40) == player;
                        hidden[i] |= row.drawing_disabled();
                    }
                }
            }
            if session.source_arena().resolve(target).is_none() { break; }
        }
    }
    let receipts = area_damage::take_effect_damage_trace();
    for (i, target) in originals.iter().enumerate() {
        assert!(receipts.iter().any(|r| r.target_slot == target.source_slot
            && r.target_generation == target.generation && r.pulse.owner == player
            && r.pulse.payload == 1600 && r.hit_count.is_some_and(|n| n > 0)
            && r.mailbox_after.is_some_and(|(damage, attacker)| damage > 0 && attacker == player)),
            "actual Meteor impact must write this original mailbox: {target:?} {receipts:?}");
        assert!(callbacks.iter().any(|callback| {
            let Some(after) = callback.after else { return false; };
            let Some(writer) = callback.entry_writer else { return false; };
            callback.source == *target && writer.target == *target
                && callback.before.linked_child == 0
                && callback.before.pending_attacker == player
                && callback.before.pending_damage > 0
                && callback.before.life >= 0
                && after.life == callback.before.life.wrapping_sub(callback.before.pending_damage)
                && after.life < 0 && after.action == 140 && after.killer == player
                && after.pending_attacker == 0
                && writer.mailbox == (callback.before.pending_damage, player)
                && receipts.iter().any(|receipt| {
                    receipt.target_slot == target.source_slot
                        && receipt.target_generation == target.generation
                        && receipt.pulse.owner == player && receipt.pulse.payload == 1600
                        && receipt.source_scoped
                        && receipt.writer_sequence == Some(writer.sequence)
                        && impacts.iter().any(|impact| {
                            receipt.source_slot == Some(impact.impact.source_slot)
                                && writer.source == area_damage::PlayerMailboxSource::Effect(impact.impact)
                                && (impact.state.source_class, impact.state.model, impact.state.subspell_index)
                                    == (10, 17, 16_000)
                                && launches.iter().any(|launch| {
                                    launch.holder == holder_handle && launch.caster == caster_handle
                                        && launch.projectile == impact.projectile
                                        && (launch.packet.class, launch.packet.model, launch.packet.subspell_index)
                                            == (9, 3, 16_000)
                                })
                        })
                })
        }), "the latest actual Meteor writer must be consumed as this original's fatal mailbox: {target:?} callbacks={callbacks:?} receipts={receipts:?}");
        assert!(observed_death[i] && hidden[i] && session.source_arena().resolve(*target).is_none(),
            "original must die with player credit, hide and reclaim: {target:?}");
    }
    assert!(session.economy().authored_bee_kill_census(player) >= credit_before + 2);
    for _ in 0..8 {
        if session.runtime().objectives[owner].stage_selectors[sensor] == 2 { break; }
        session.step_outer_turn(PlayerCommand::default(), 0);
    }
    assert_eq!(session.runtime().objectives[owner].stage_selectors[sensor], 2,
        "actual NoLiving17 objective follows both original retirements");
}
