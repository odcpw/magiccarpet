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
    let outcome = mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor, &[parent], &families).unwrap();
    assert_eq!(outcome.feedback, vec![(parent, 24)]);
    let row = &arena.entities[usize::from(actor)];
    assert_eq!((row.action, u16::from_le_bytes([row.record[154], row.record[155]])), (138, enemy));
    let before = u16::from_le_bytes([row.record[50], row.record[51]]);
    mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor, &[parent], &families).unwrap();
    let row = &arena.entities[usize::from(actor)];
    assert_eq!(u16::from_le_bytes([row.record[50], row.record[51]]), before - 1);
    assert_eq!(row.action, 138, "a selected class5 enemy remains a valid target");

    let row = &mut arena.entities[usize::from(actor)];
    row.set_source_action(143); put(row, 50, 1);
    mc2::step_row(&mut arena, &mut runtime, &mut terrain, particles, actor, &[parent], &families).unwrap();
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
        Some((10, i32::from(RULES.melee_damage), actor.source_id))
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
            .expect("retained Level 17 model17 actor");
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
        contact_receipts.contains(&(i32::from(RULES.melee_damage), false))
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
